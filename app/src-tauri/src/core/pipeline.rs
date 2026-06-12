use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

#[cfg(test)]
mod pipeline_test;

use super::breadth::BreadthEngine;
use super::db::Database;
use super::events::{MarketEvent, SystemEvent};
use super::indicators::IndicatorEngine;
use super::metadata::MetadataManager;
use super::rest::BinanceRestClient;
use super::risk::RiskManager;
use super::websocket::BinanceWsClient;
use crate::core::models::NormalizedCandleData;
use crate::engine::scanner::{ScannerEngine, ScannerPayload};

const STARTUP_UNIVERSE_CACHE_TTL_MS: i64 = 60 * 60 * 1000;

pub struct DataPipeline {
    market_event_rx: mpsc::Receiver<MarketEvent>,
    system_event_rx: mpsc::Receiver<SystemEvent>,
    ws_client: Arc<BinanceWsClient>,
    /// Live market client: scan, real-time REST calls. Budget cao (65%).
    rest_client: BinanceRestClient,
    /// Bootstrap client: warmup, metadata, breadth. Budget thấp (40%) để nhường live.
    rest_client_bootstrap: BinanceRestClient,
    db: Arc<Database>,
    global_event_tx: broadcast::Sender<MarketEvent>,
    indicator_engine: Arc<Mutex<IndicatorEngine>>,
    risk_manager: Arc<Mutex<RiskManager>>,
    metadata_manager: Arc<MetadataManager>,
    breadth_engine: Arc<BreadthEngine>,
    scanner_engine: ScannerEngine,
    symbols: Vec<String>,
    app_handle: AppHandle,
    last_scan_timestamp: Arc<Mutex<i64>>,
}

impl DataPipeline {
    /// Khởi tạo DataPipeline Orchestrator: Thiết lập hạ tầng truyền tin (IPC), kết nối Exchange Feed và các Engine phân tích định lượng (Quantitative Engines).
    pub fn new(
        symbols: Vec<String>,
        db: Arc<Database>,
        global_event_tx: broadcast::Sender<MarketEvent>,
        app_handle: AppHandle,
    ) -> Self {
        let (market_tx, market_rx) = mpsc::channel(1000);
        let (system_tx, system_rx) = mpsc::channel(100);

        // Hai rate-limit budget tách biệt:
        // - live: scan + real-time REST (65% budget, concurrency 8)
        // - bootstrap: warmup + metadata + breadth (40% budget, concurrency 4)
        // Đảm bảo bootstrap không bao giờ chèn live market events.
        let rest_client = BinanceRestClient::new();
        let rest_client_bootstrap = BinanceRestClient::new_bootstrap();

        let ws_client = BinanceWsClient::new(market_tx, system_tx);
        let indicator_engine = Arc::new(Mutex::new(IndicatorEngine::new()));

        Self {
            market_event_rx: market_rx,
            system_event_rx: system_rx,
            ws_client: Arc::new(ws_client),
            rest_client: rest_client.clone(),
            rest_client_bootstrap: rest_client_bootstrap.clone(),
            db: db.clone(),
            global_event_tx,
            indicator_engine: indicator_engine.clone(),
            risk_manager: Arc::new(Mutex::new(RiskManager::new())),
            // MetadataManager dùng bootstrap client — fetch nặng, không time-critical.
            metadata_manager: Arc::new(MetadataManager::new(rest_client_bootstrap.clone())),
            // BreadthEngine dùng bootstrap client — tính breadth là batch job 1h/lần.
            breadth_engine: Arc::new(BreadthEngine::new(
                rest_client_bootstrap.clone(),
                db.clone(),
                app_handle.clone(),
            )),
            // ScannerEngine dùng live client — cần latency thấp khi scan.
            scanner_engine: ScannerEngine::new(rest_client, indicator_engine),
            symbols,
            app_handle,
            last_scan_timestamp: Arc::new(Mutex::new(0)),
        }
    }

    /// Kích hoạt chu kỳ sống của Pipeline: Đồng bộ Scanning Universe, nạp dữ liệu lịch sử (Indicator Priming) và thực thi vòng lặp điều phối chính.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Data Pipeline (Phase 0)...");

        let _ = self.app_handle.emit(
            "market-event",
            &MarketEvent::SyncProgress {
                step: "METADATA".to_string(),
                progress: 5.0,
                message: "System engine starting...".to_string(),
            },
        );

        // 1. Fast-start từ cache nội bộ để user không phải chờ full metadata scan.
        self.bootstrap_symbols_from_cache().await;

        // 2. Worker Risk & News: Theo dõi lịch kinh tế và các biến cố Macro (30p/lần).
        let risk_manager_clone = Arc::clone(&self.risk_manager);
        tokio::spawn(async move {
            loop {
                {
                    let risk = risk_manager_clone.lock().await;
                    if let Err(e) = risk.update_economic_calendar().await {
                        error!("Failed to update economic calendar: {}", e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1800)).await;
            }
        });

        // 3. Worker Market Universe: Cập nhật universe và Market Breadth định kỳ (30min/lần).
        // Lần đầu chạy sau 5 phút (không sleep 1h trước như cũ) để đảm bảo universe fresh
        // nếu background bootstrap đã dùng cache cũ mà bỏ qua REST refresh.
        let breadth_engine_clone = Arc::clone(&self.breadth_engine);
        let metadata_manager_clone = Arc::clone(&self.metadata_manager);
        let db_clone_sync = Arc::clone(&self.db);
        let risk_manager_for_total3 = Arc::clone(&self.risk_manager);
        let app_handle_clone = self.app_handle.clone();

        tokio::spawn(async move {
            // Delay ngắn lần đầu để system ổn định trước khi refresh
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            loop {
                info!("Scheduled sync: Updating Universe and Market Breadth (30min interval)...");
                if let Ok(candidates) = metadata_manager_clone.get_top_altcoins(None).await {
                    let _ = app_handle_clone.emit(
                        "market-event",
                        &MarketEvent::UniverseUpdated(candidates.clone()),
                    );
                    if let Err(e) = db_clone_sync.save_universe_candidates(&candidates).await {
                        warn!("Failed to save universe candidates: {}", e);
                    }
                    let top_universe: Vec<String> =
                        candidates.into_iter().map(|c| c.symbol).collect();

                    if let Ok((ema50, ema200)) =
                        breadth_engine_clone.calculate_breadth(&top_universe).await
                    {
                        breadth_engine_clone.apply_results(ema50, ema200).await;

                        let mut risk = risk_manager_for_total3.lock().await;
                        use crate::core::models::TrendDirection;
                        risk.total3_trend = if ema50 > 55.0 {
                            TrendDirection::Up
                        } else if ema50 < 45.0 {
                            TrendDirection::Down
                        } else {
                            TrendDirection::Sideway
                        };
                    }
                }
                // 30min thay vì 1h — crypto market thay đổi nhanh
                tokio::time::sleep(tokio::time::Duration::from_secs(1800)).await;
            }
        });

        // 4. Duy trì kết nối WebSocket thời gian thực càng sớm càng tốt.
        let ws_client_clone = Arc::clone(&self.ws_client);
        tokio::spawn(async move {
            if let Err(e) = ws_client_clone.run().await {
                error!("WebSocket client stopped with error: {}", e);
            }
        });

        // 5. Prime BTC context (từ DB cache) và background bootstrap chạy SONG SONG.
        // prime_cached_btc_context() chỉ là DB I/O (~50-100ms) — không cần chờ trước bootstrap.
        // Background bootstrap cần bắt đầu sớm nhất có thể để warmup indicators.
        tokio::join!(
            self.prime_cached_btc_context(),
            std::future::ready(self.spawn_background_bootstrap())
        );

        // 6. Vòng lặp điều phối chính: Phân phối sự kiện và kích hoạt Scanner.
        let mut regime_rx = self.global_event_tx.subscribe();
        loop {
            tokio::select! {
                Some(market_event) = self.market_event_rx.recv() => {
                    self.handle_market_event(market_event).await;
                }
                Some(system_event) = self.system_event_rx.recv() => {
                    self.handle_system_event(system_event).await;
                }
                Ok(global_event) = regime_rx.recv() => {
                    if let MarketEvent::RegimeUpdated(context) = global_event {
                        let _ = self.app_handle.emit("market-event", &MarketEvent::RegimeUpdated(context.clone()));

                        if context.allow_alt_scan {
                            let now = chrono::Utc::now().timestamp();
                            let mut last_scan = self.last_scan_timestamp.lock().await;

                            if now - *last_scan >= 900 {
                                info!("Phase 2: Triggering real Altcoin Scan...");
                                *last_scan = now;

                                let mut candidates = self.db.get_stored_universe_candidates().await.unwrap_or_default();
                                if candidates.is_empty() {
                                    match self.metadata_manager.get_top_altcoins(None).await {
                                        Ok(fresh_candidates) => {
                                            if let Err(e) = self.db.save_universe_candidates(&fresh_candidates).await {
                                                warn!("Failed to save universe candidates: {}", e);
                                            }
                                            candidates = fresh_candidates;
                                        }
                                        Err(e) => {
                                            warn!("Failed to load universe candidates for scan: {}", e);
                                        }
                                    }
                                }

                                if !candidates.is_empty() {
                                    let _ = self.app_handle.emit("market-event", &MarketEvent::UniverseUpdated(candidates.clone()));
                                    let top_altcoins: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();

                                    if let Ok(tickers_24h) = self.rest_client.fetch_24h_tickers().await {
                                        let snapshots = self.scanner_engine.fetch_real_snapshots(&top_altcoins, &tickers_24h, self.db.clone(), Arc::clone(&self.risk_manager)).await;

                                        let btc_change_1d = tickers_24h.iter()
                                            .find(|t| t["symbol"].as_str() == Some("BTCUSDT"))
                                            .and_then(|t| t["priceChangePercent"].as_str())
                                            .and_then(|s| s.parse::<f64>().ok())
                                            .unwrap_or(0.0);

                                        // Lấy 1 nến 4H gần nhất — tính % change trong nến đó (open→close).
                                        // Trước đây lấy 2 nến và tính từ nến[0].open → nến[1].close = 8H span, sai tên.
                                        let btc_4h = self.db.get_candles("BTCUSDT", "4h", 1).await.unwrap_or_default();
                                        let btc_change_4h = if let Some(c) = btc_4h.first() {
                                            if c.open > 0.0 { (c.close - c.open) / c.open * 100.0 } else { 0.0 }
                                        } else { 0.0 };

                                        let shortlist = self.scanner_engine.scan(&context, btc_change_1d, btc_change_4h, &snapshots);

                                        // Save shortlist entries for entry validation
                                        let alt_tf = crate::core::config::AppConfig::load().altcoin_analysis_timeframe;
                                        for candidate in &shortlist {
                                            if let Some(snap) = snapshots.iter().find(|s| s.symbol == candidate.symbol) {
                                                let db_data = NormalizedCandleData {
                                                    timestamp: now,
                                                    candle: crate::core::models::Candle {
                                                        symbol: snap.symbol.clone(),
                                                        timeframe: alt_tf.clone(),
                                                        close: snap.price,
                                                        is_closed: true,
                                                        ..Default::default()
                                                    },
                                                    indicators: crate::core::models::Indicators {
                                                        ema50: Some(snap.ema50_4h),
                                                        ema200: Some(snap.ema200_4h),
                                                        ..Default::default()
                                                    },
                                                    microstructure: crate::core::models::Microstructure {
                                                        oi_change_4h_pct: snap.oi_growth_4h_pct,
                                                        ..Default::default()
                                                    },
                                                    ..Default::default()
                                                };
                                                let _ = self.db.insert_closed_candle(&db_data).await;
                                            }
                                        }

                                        let payload = ScannerPayload { scan_timestamp: now, shortlist };
                                        let _ = self.app_handle.emit("market-event", &MarketEvent::ScannerUpdated(payload.clone()));
                                        let _ = self.global_event_tx.send(MarketEvent::ScannerUpdated(payload));
                                    }
                                }
                            }
                        } else {
                            let payload = ScannerPayload { scan_timestamp: chrono::Utc::now().timestamp(), shortlist: vec![] };
                            let _ = self.app_handle.emit("market-event", &MarketEvent::ScannerUpdated(payload.clone()));
                            let _ = self.global_event_tx.send(MarketEvent::ScannerUpdated(payload));
                        }
                    }
                }
                else => {
                    info!("Pipeline channels closed. Exiting...");
                    break;
                }
            }
        }
        Ok(())
    }

    async fn bootstrap_symbols_from_cache(&mut self) {
        let cached = self
            .db
            .get_stored_universe_candidates()
            .await
            .unwrap_or_default();
        if !cached.is_empty() {
            let _ = self.app_handle.emit(
                "market-event",
                &MarketEvent::UniverseUpdated(cached.clone()),
            );
        }

        let mut symbols = vec!["BTCUSDT".to_string()];
        symbols.extend(cached.into_iter().map(|c| c.symbol));
        symbols.sort();
        symbols.dedup();

        self.symbols = symbols.clone();
        self.ws_client.update_symbols(symbols).await;

        let _ = self.app_handle.emit(
            "market-event",
            &MarketEvent::SyncProgress {
                step: "WEBSOCKET".to_string(),
                progress: 70.0,
                message: "Starting live market feed from cached universe...".to_string(),
            },
        );
    }

    async fn prime_cached_btc_context(&self) {
        // Thứ tự: 1D → 4H (indicator context) → 15m (risk snapshot seed).
        // 15m được broadcast cuối để set latest_risk_snapshot trước khi live 15m feed đến.
        let tfs = ["1d", "4h", "15m"];
        for tf in tfs {
            if let Ok(btc_data) = self.db.get_candles_with_indicators("BTCUSDT", tf, 1).await {
                if let Some(mut data) = btc_data.into_iter().next() {
                    let risk = self.risk_manager.lock().await;
                    let (e50, e200) = {
                        let r50 = self.breadth_engine.market_breadth_ema50.read().await;
                        let r200 = self.breadth_engine.market_breadth_ema200.read().await;
                        (*r50, *r200)
                    };
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure =
                        risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    data.macro_events = risk.get_macro_events().await;
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = e50;
                    indices.market_breadth_pct_above_ema200 = e200;
                    data.market_indices = indices;

                    // Enrich range và ATR surge để Volatility Regime hoạt động đúng từ khởi động.
                    // Không có range → luôn là Compression; không có atr_surge_ratio → không phát VolatilityAlert.
                    if data.candle.open > 0.0 {
                        data.range_24h_pct =
                            (data.candle.high - data.candle.low) / data.candle.open;
                    }
                    data.range_p40_90d = self
                        .db
                        .get_p40_range_90d_for_tf("BTCUSDT", tf)
                        .await
                        .unwrap_or(0.0);
                    if let Some(current_atr) = data.indicators.atr14 {
                        let avg_atr = self
                            .db
                            .get_avg_atr_20("BTCUSDT", tf)
                            .await
                            .unwrap_or(current_atr);
                        if avg_atr > 0.0 {
                            data.atr_surge_ratio = current_atr / avg_atr;
                        }
                    }

                    let _ = self.global_event_tx.send(MarketEvent::CandleClosed(data));
                }
            }
        }
    }

    fn spawn_background_bootstrap(&self) {
        let metadata_manager = Arc::clone(&self.metadata_manager);
        let breadth_engine = Arc::clone(&self.breadth_engine);
        let db = Arc::clone(&self.db);
        let risk_manager = Arc::clone(&self.risk_manager);
        // Bootstrap dùng rest_client_bootstrap (budget thấp) để không chiếm bandwidth live.
        let rest_client = self.rest_client_bootstrap.clone();
        let ws_client = Arc::clone(&self.ws_client);
        let indicator_engine = Arc::clone(&self.indicator_engine);
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            info!("[PIPELINE] Background bootstrap started.");
            let _ = app_handle.emit(
                "market-event",
                &MarketEvent::SyncProgress {
                    step: "BACKGROUND_SYNC".to_string(),
                    progress: 72.0,
                    message: "Checking local universe cache before Binance refresh...".to_string(),
                },
            );

            let cached_candidates = db
                .get_stored_universe_candidates()
                .await
                .unwrap_or_default();
            let cache_age_ms =
                db.get_universe_updated_at()
                    .await
                    .ok()
                    .flatten()
                    .map(|updated_at| {
                        chrono::Utc::now()
                            .timestamp_millis()
                            .saturating_sub(updated_at)
                    });

            if !cached_candidates.is_empty()
                && cache_age_ms
                    .map(|age| age <= STARTUP_UNIVERSE_CACHE_TTL_MS)
                    .unwrap_or(false)
            {
                let age_minutes = cache_age_ms.unwrap_or_default() / 60_000;
                info!(
                    "[PIPELINE] Using fresh universe cache (age={}m); skipping startup REST metadata refresh.",
                    age_minutes
                );

                let _ = app_handle.emit(
                    "market-event",
                    &MarketEvent::UniverseUpdated(cached_candidates.clone()),
                );

                let top_alts: Vec<String> =
                    cached_candidates.iter().map(|c| c.symbol.clone()).collect();
                {
                    let mut risk = risk_manager.lock().await;
                    for candidate in &cached_candidates {
                        risk.symbol_funding
                            .insert(candidate.symbol.clone(), candidate.funding_rate);
                    }
                }

                let mut ws_symbols = vec!["BTCUSDT".to_string()];
                ws_symbols.extend(top_alts);
                ws_symbols.sort();
                ws_symbols.dedup();
                ws_client.update_symbols(ws_symbols).await;

                let _ = app_handle.emit(
                    "market-event",
                    &MarketEvent::SyncProgress {
                        step: "WARMUP_DONE".to_string(),
                        progress: 100.0,
                        message: format!(
                            "Live feed ready from cached universe; skipped startup REST refresh (cache age {}m).",
                            age_minutes
                        ),
                    },
                );
                return;
            }

            match metadata_manager.get_top_altcoins(Some(&app_handle)).await {
                Ok(candidates) => {
                    let _ = app_handle.emit(
                        "market-event",
                        &MarketEvent::UniverseUpdated(candidates.clone()),
                    );
                    if let Err(e) = db.save_universe_candidates(&candidates).await {
                        warn!("Failed to save universe candidates: {}", e);
                    }

                    let top_alts: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
                    let mut ws_symbols = vec!["BTCUSDT".to_string()];
                    ws_symbols.extend(top_alts.clone());
                    ws_symbols.sort();
                    ws_symbols.dedup();
                    ws_client.update_symbols(ws_symbols.clone()).await;

                    let funding_task = {
                        let rest = rest_client.clone();
                        let risk_m = Arc::clone(&risk_manager);
                        let universe_set: std::collections::HashSet<String> =
                            top_alts.iter().cloned().collect();
                        async move {
                            match rest.fetch_premium_index().await {
                                Ok(premiums) => {
                                    let mut risk = risk_m.lock().await;
                                    for p in premiums {
                                        let sym = p["symbol"].as_str().unwrap_or("").to_string();
                                        if universe_set.contains(&sym) || sym == "BTCUSDT" {
                                            let fr = p["lastFundingRate"]
                                                .as_str()
                                                .unwrap_or("0")
                                                .parse::<f64>()
                                                .unwrap_or(0.0);
                                            risk.symbol_funding.insert(sym, fr);
                                        }
                                    }
                                }
                                Err(e) => warn!("[PIPELINE] Background funding sync failed: {}", e),
                            }
                        }
                    };

                    let breadth_task = {
                        let breadth_e = Arc::clone(&breadth_engine);
                        let alts = top_alts.clone();
                        async move {
                            if let Ok((e50, e200)) = breadth_e.calculate_breadth(&alts).await {
                                breadth_e.apply_results(e50, e200).await;
                                e50
                            } else {
                                0.0
                            }
                        }
                    };

                    let _ = app_handle.emit(
                        "market-event",
                        &MarketEvent::SyncProgress {
                            step: "CONTEXT".to_string(),
                            progress: 82.0,
                            message: "Synchronizing funding and market breadth...".to_string(),
                        },
                    );

                    let context_result =
                        tokio::time::timeout(tokio::time::Duration::from_secs(120), async {
                            tokio::join!(funding_task, breadth_task)
                        })
                        .await;

                    let ema50_val = match context_result {
                        Ok((_, ema50_val)) => ema50_val,
                        Err(_) => {
                            warn!("[PIPELINE] Funding/breadth sync timed out; continuing with cached context.");
                            let _ = app_handle.emit(
                                "market-event",
                                &MarketEvent::SyncProgress {
                                    step: "CONTEXT".to_string(),
                                    progress: 88.0,
                                    message: "Context sync is delayed by rate limits; continuing with cached data.".to_string(),
                                },
                            );
                            0.0
                        }
                    };
                    {
                        let mut risk = risk_manager.lock().await;
                        use crate::core::models::TrendDirection;
                        risk.total3_trend = if ema50_val > 55.0 {
                            TrendDirection::Up
                        } else if ema50_val < 45.0 {
                            TrendDirection::Down
                        } else {
                            TrendDirection::Sideway
                        };
                    }

                    if let Err(e) = Self::warmup_symbols(
                        ws_symbols,
                        db,
                        rest_client,
                        indicator_engine,
                        Some(app_handle.clone()),
                    )
                    .await
                    {
                        warn!("[PIPELINE] Background warmup failed: {}", e);
                    }

                    let _ = app_handle.emit(
                        "market-event",
                        &MarketEvent::SyncProgress {
                            step: "WARMUP_DONE".to_string(),
                            progress: 100.0,
                            message: "Universe, funding, breadth, and warmup synchronized."
                                .to_string(),
                        },
                    );
                }
                Err(e) => {
                    warn!("[PIPELINE] Background universe refresh failed: {}", e);
                    let _ = app_handle.emit(
                        "market-event",
                        &MarketEvent::SyncProgress {
                            step: "WARMUP_DONE".to_string(),
                            progress: 100.0,
                            message:
                                "Live feed ready with cached context; background refresh failed."
                                    .to_string(),
                        },
                    );
                }
            }
        });
    }

    #[allow(dead_code)]
    /// Lọc danh sách Top 100 và xác lập baseline bối cảnh thị trường.
    async fn sync_metadata_and_breadth(&mut self) -> Result<()> {
        info!("[PIPELINE] Synchronizing Universe Metadata & Breadth...");

        // 1. Metadata (25%)
        let candidates = self
            .metadata_manager
            .get_top_altcoins(Some(&self.app_handle))
            .await?;
        let _ = self.app_handle.emit(
            "market-event",
            &MarketEvent::UniverseUpdated(candidates.clone()),
        );
        if let Err(e) = self.db.save_universe_candidates(&candidates).await {
            warn!("Failed to save universe candidates: {}", e);
        }
        let top_alts: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
        let universe_set: std::collections::HashSet<String> = top_alts.iter().cloned().collect();

        // 2. WebSocket (30%)
        let mut all_symbols = vec!["BTCUSDT".to_string()];
        all_symbols.extend(top_alts.clone());
        self.symbols = all_symbols.clone();
        self.ws_client.update_symbols(all_symbols).await;

        let _ = self.app_handle.emit(
            "market-event",
            &MarketEvent::SyncProgress {
                step: "CONTEXT".to_string(),
                progress: 40.0,
                message: "Syncing Funding & Breadth in parallel...".to_string(),
            },
        );

        // 3 & 4. Parallel Sync of Funding and Breadth (Fixes Weaknesses 1-4)
        let rest = self.rest_client.clone();
        let risk_m = Arc::clone(&self.risk_manager);
        let breadth_e = Arc::clone(&self.breadth_engine);
        let alts = top_alts.clone();

        let funding_task = async move {
            match rest.fetch_premium_index().await {
                Ok(premiums) => {
                    let mut risk = risk_m.lock().await;
                    let mut count = 0;
                    for p in premiums {
                        let sym = p["symbol"].as_str().unwrap_or("").to_string();
                        if universe_set.contains(&sym) || sym == "BTCUSDT" {
                            let fr = p["lastFundingRate"]
                                .as_str()
                                .unwrap_or("0")
                                .parse::<f64>()
                                .unwrap_or(0.0);
                            risk.symbol_funding.insert(sym, fr);
                            count += 1;
                        }
                    }
                    info!(
                        "[PIPELINE] Funding rates synchronized for {} symbols.",
                        count
                    );
                }
                Err(e) => warn!("[PIPELINE] Funding sync failed: {}", e),
            }
        };

        let breadth_task = async move {
            if let Ok((e50, e200)) = breadth_e.calculate_breadth(&alts).await {
                breadth_e.apply_results(e50, e200).await;
                e50
            } else {
                0.0
            }
        };

        let (_, ema50_val) = tokio::join!(funding_task, breadth_task);

        // Update TOTAL3 trend
        {
            let mut risk = self.risk_manager.lock().await;
            use crate::core::models::TrendDirection;
            risk.total3_trend = if ema50_val > 55.0 {
                TrendDirection::Up
            } else if ema50_val < 45.0 {
                TrendDirection::Down
            } else {
                TrendDirection::Sideway
            };
        }

        let _ = self.app_handle.emit(
            "market-event",
            &MarketEvent::SyncProgress {
                step: "PHASE0_DONE".to_string(),
                progress: 70.0,
                message: "Context sync complete.".to_string(),
            },
        );

        Ok(())
    }

    #[allow(dead_code)]
    /// Backfilling dữ liệu nến lịch sử để khởi tạo Indicators.
    async fn perform_warmup(&mut self) -> Result<()> {
        info!("Performing warm-up...");
        Self::warmup_symbols(
            self.symbols.clone(),
            Arc::clone(&self.db),
            self.rest_client.clone(),
            Arc::clone(&self.indicator_engine),
            Some(self.app_handle.clone()),
        )
        .await?;
        self.prime_cached_btc_context().await;
        Ok(())
    }

    async fn warmup_symbols(
        symbols: Vec<String>,
        db: Arc<Database>,
        rest_client: BinanceRestClient,
        indicator_engine: Arc<Mutex<IndicatorEngine>>,
        progress_handle: Option<AppHandle>,
    ) -> Result<()> {
        let timeframes = crate::core::config::AppConfig::load().timeframes;
        // Concurrency 8 — phù hợp với rate_limiter bootstrap (max_concurrency=4 thực tế do rate limiter).
        // Outer semaphore 8 giới hạn số task trong queue, rate limiter kiểm soát actual HTTP calls.
        let semaphore = Arc::new(tokio::sync::Semaphore::new(8));
        let total_steps = timeframes.len() * symbols.len();
        if total_steps == 0 {
            return Ok(());
        }
        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // BTC-first: warmup BTC đồng bộ trước tất cả altcoins.
        // Regime engine cần BTC 1D/4H indicators để phát context — không để BTC tranh queue với 100 alts.
        let (btc_symbols, alt_symbols): (Vec<String>, Vec<String>) =
            symbols.into_iter().partition(|s| s == "BTCUSDT");

        for tf in &timeframes {
            for symbol in &btc_symbols {
                let last = db.get_last_update_time(symbol, tf).await.unwrap_or(0);
                let interval = timeframe_to_ms(tf);
                if (chrono::Utc::now().timestamp_millis() - last) > interval {
                    let limit = if last == 0 {
                        200
                    } else {
                        (((chrono::Utc::now().timestamp_millis() - last) / interval) as u32)
                            .min(200)
                            + 2
                    };
                    if let Ok(klines) = rest_client.fetch_klines(symbol, tf, limit).await {
                        let rows = {
                            let mut engine = indicator_engine.lock().await;
                            klines
                                .into_iter()
                                .map(|k| {
                                    let inds = engine.process(&k);
                                    NormalizedCandleData {
                                        candle: k,
                                        indicators: inds,
                                        metadata: crate::core::models::CandleMetadata {
                                            is_warmup: true,
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    }
                                })
                                .collect::<Vec<_>>()
                        };
                        if let Err(e) = db.insert_closed_candles_bulk(&rows).await {
                            warn!("Failed to bulk insert BTC warmup candles {} {}: {}", symbol, tf, e);
                        }
                    }
                } else {
                    let candles = db.get_candles(symbol, tf, 200).await.unwrap_or_default();
                    let mut engine = indicator_engine.lock().await;
                    for k in candles {
                        let _ = engine.process(&k);
                    }
                }
                let done = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if let Some(handle) = &progress_handle {
                    let prog = 70.0 + (done as f64 / total_steps as f64) * 30.0;
                    let _ = handle.emit(
                        "market-event",
                        &MarketEvent::SyncProgress {
                            step: "WARMUP".to_string(),
                            progress: prog,
                            message: format!("Warmup BTC {}: {}/{}", tf, done, total_steps),
                        },
                    );
                }
            }
        }

        // Altcoins: concurrent với semaphore, dùng bootstrap budget
        let mut join_handles = Vec::new();
        for tf in timeframes {
            for symbol in &alt_symbols {
                let (s, t, d, r, i, c, h) = (
                    symbol.clone(),
                    tf.clone(),
                    db.clone(),
                    rest_client.clone(),
                    indicator_engine.clone(),
                    completed.clone(),
                    progress_handle.clone(),
                );
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                join_handles.push(tokio::spawn(async move {
                    let _permit = permit;
                    let last = d.get_last_update_time(&s, &t).await.unwrap_or(0);
                    let interval = timeframe_to_ms(&t);
                    if (chrono::Utc::now().timestamp_millis() - last) > interval {
                        let limit = if last == 0 {
                            200
                        } else {
                            (((chrono::Utc::now().timestamp_millis() - last) / interval) as u32)
                                .min(200)
                                + 2
                        };
                        if let Ok(klines) = r.fetch_klines(&s, &t, limit).await {
                            let rows = {
                                let mut engine = i.lock().await;
                                klines
                                    .into_iter()
                                    .map(|k| {
                                        let inds = engine.process(&k);
                                        NormalizedCandleData {
                                            candle: k,
                                            indicators: inds,
                                            metadata: crate::core::models::CandleMetadata {
                                                is_warmup: true,
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            };
                            if let Err(e) = d.insert_closed_candles_bulk(&rows).await {
                                warn!(
                                    "Failed to bulk insert warmup candles for {} {}: {}",
                                    s, t, e
                                );
                            }
                        }
                    } else {
                        let candles = d.get_candles(&s, &t, 200).await.unwrap_or_default();
                        let mut engine = i.lock().await;
                        for k in candles {
                            let _ = engine.process(&k);
                        }
                    }
                    let done = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if let Some(handle) = h {
                        let prog = 70.0 + (done as f64 / total_steps as f64) * 30.0;
                        let _ = handle.emit(
                            "market-event",
                            &MarketEvent::SyncProgress {
                                step: "WARMUP".to_string(),
                                progress: prog,
                                message: format!("Warmup {} {}: {}/{}", s, t, done, total_steps),
                            },
                        );
                    }
                }));
            }
        }
        for handle in join_handles {
            let _ = handle.await;
        }
        Ok(())
    }

    pub async fn handle_market_event(&mut self, event: MarketEvent) {
        match event {
            MarketEvent::CandleClosed(mut data) => {
                let _ = self
                    .fill_gaps(
                        &data.candle.symbol,
                        &data.candle.timeframe,
                        data.candle.open_time,
                    )
                    .await;
                let mut engine = self.indicator_engine.lock().await;
                data.indicators = engine.process(&data.candle);

                let alt_tf = crate::core::config::AppConfig::load().altcoin_analysis_timeframe;
                let tf = data.candle.timeframe.as_str();
                let is_btc = data.candle.symbol == "BTCUSDT";

                // range_24h_pct: phép tính đơn giản, luôn compute cho mọi nến đóng.
                // Dùng bởi scanner (altcoin 1D) và regime engine (BTC 4H + 1D).
                if data.candle.open > 0.0 {
                    data.range_24h_pct =
                        (data.candle.high - data.candle.low) / data.candle.open;
                }

                // range_p40_90d và atr_surge_ratio yêu cầu truy vấn DB — chỉ compute khi cần:
                // - Scanner cần cho mọi symbol ở altcoin_analysis_timeframe
                // - Regime engine cần cho BTCUSDT 4H (operational) và 1D (macro)
                let is_regime_tf = is_btc && (tf == "4h" || tf == "1d");
                if tf == alt_tf || is_regime_tf {
                    data.range_p40_90d = self
                        .db
                        .get_p40_range_90d_for_tf(&data.candle.symbol, tf)
                        .await
                        .unwrap_or(0.0);

                    // atr_surge_ratio = current_atr14 / avg_atr14_20_nến
                    // > 2.5 → Volatility Expansion; > 3.0 → VolatilityAlert
                    if let Some(current_atr) = data.indicators.atr14 {
                        let avg_atr = self
                            .db
                            .get_avg_atr_20(&data.candle.symbol, tf)
                            .await
                            .unwrap_or(current_atr);
                        if avg_atr > 0.0 {
                            data.atr_surge_ratio = current_atr / avg_atr;
                        }
                    }
                }

                {
                    let mut risk = self.risk_manager.lock().await;
                    let (e50, e200) = {
                        let r50 = self.breadth_engine.market_breadth_ema50.read().await;
                        let r200 = self.breadth_engine.market_breadth_ema200.read().await;
                        (*r50, *r200)
                    };
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure =
                        risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    data.macro_events = risk.get_macro_events().await;
                    if data.candle.timeframe == "4h" {
                        risk.snapshot_4h_oi(&data.candle.symbol);
                    }
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = e50;
                    indices.market_breadth_pct_above_ema200 = e200;
                    data.market_indices = indices;
                }

                let _ = self.db.insert_closed_candle(&data).await;
                let _ = self
                    .app_handle
                    .emit("market-event", &MarketEvent::CandleClosed(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleClosed(data));
            }
            MarketEvent::CandleUpdated(mut data) => {
                {
                    let mut risk = self.risk_manager.lock().await;
                    let (e50, e200) = {
                        let r50 = self.breadth_engine.market_breadth_ema50.read().await;
                        let r200 = self.breadth_engine.market_breadth_ema200.read().await;
                        (*r50, *r200)
                    };
                    let mut engine = self.indicator_engine.lock().await;
                    let cvd = data.candle.taker_buy_volume * 2.0 - data.candle.volume;
                    if data.candle.timeframe == "4h" {
                        risk.symbol_cvd_4h.insert(data.candle.symbol.clone(), cvd);
                    } else if data.candle.timeframe == "1d" {
                        risk.symbol_cvd_1d.insert(data.candle.symbol.clone(), cvd);
                    }

                    data.indicators = engine.process_unclosed(&data.candle);
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure =
                        risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = e50;
                    indices.market_breadth_pct_above_ema200 = e200;
                    data.market_indices = indices;
                }
                let _ = self
                    .app_handle
                    .emit("market-event", &MarketEvent::CandleUpdated(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleUpdated(data));
            }
            MarketEvent::DepthUpdated {
                symbol,
                is_liquidation,
                price,
                value_usd,
                ..
            } => {
                let mut risk = self.risk_manager.lock().await;
                if !is_liquidation {
                    risk.update_oi(symbol, value_usd);
                } else {
                    risk.recent_liquidations_usd += value_usd;
                    if value_usd > 100_000.0 {
                        let cur = *risk.symbol_liq_upper.get(&symbol).unwrap_or(&0.0);
                        if cur == 0.0 || price > cur {
                            risk.symbol_liq_upper.insert(symbol, price);
                        } else {
                            risk.symbol_liq_lower.insert(symbol, price);
                        }
                    }
                }
            }
            MarketEvent::FundingUpdated {
                symbol,
                funding_rate,
                ..
            } => {
                let mut risk = self.risk_manager.lock().await;
                if symbol == "BTCDOMUSDT" {
                    risk.btc_dominance = funding_rate;
                } else {
                    risk.symbol_funding.insert(symbol, funding_rate);
                }
            }
            MarketEvent::LiveFeedReady { .. } => {
                let _ = self.app_handle.emit("market-event", &event);
                let _ = self.global_event_tx.send(event);
            }
            _ => {
                let _ = self.global_event_tx.send(event);
            }
        }
    }

    async fn handle_system_event(&self, event: SystemEvent) {
        if let SystemEvent::HealthChanged {
            previous,
            current,
            timestamp,
        } = event
        {
            info!(
                "[SYSTEM HEALTH] State changed from {:?} to {:?} at {}",
                previous, current, timestamp
            );
        }
    }

    async fn fill_gaps(
        &mut self,
        symbol: &str,
        timeframe: &str,
        current_open_time: i64,
    ) -> Result<()> {
        let last = self.db.get_last_update_time(symbol, timeframe).await?;
        if last == 0 {
            return Ok(());
        }
        let interval = timeframe_to_ms(timeframe);
        if current_open_time - last > (interval as f64 * 1.5) as i64 {
            let missing_count = ((current_open_time - last) / interval) as u32;
            if let Ok(missing) = self
                .rest_client
                .fetch_klines(symbol, timeframe, missing_count + 1)
                .await
            {
                for c in missing {
                    if c.open_time > last && c.open_time < current_open_time {
                        let mut engine = self.indicator_engine.lock().await;
                        let d = NormalizedCandleData {
                            candle: c.clone(),
                            indicators: engine.process(&c),
                            ..Default::default()
                        };
                        let _ = self.db.insert_closed_candle(&d).await;
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn timeframe_to_ms(tf: &str) -> i64 {
    let num = tf
        .chars()
        .take_while(|c| c.is_numeric())
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or(1);
    let unit = tf
        .chars()
        .skip_while(|c| c.is_numeric())
        .collect::<String>();
    match unit.as_str() {
        "m" => num * 60000,
        "h" => num * 3600000,
        "d" => num * 86400000,
        "w" => num * 604800000,
        _ => 3600000,
    }
}
