use anyhow::Result;
use tokio::sync::{mpsc, broadcast};
use tracing::{info, error, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[cfg(test)]
mod pipeline_test;

use super::events::{MarketEvent, SystemEvent, HealthState};
use super::websocket::BinanceWsClient;
use super::db::Database;
use super::rest::BinanceRestClient;
use super::indicators::IndicatorEngine;
use super::risk::RiskManager;
use super::metadata::MetadataManager;
use super::breadth::BreadthEngine;
use crate::core::models::{NormalizedCandleData, AltcoinSnapshot};
use crate::engine::scanner::{ScannerEngine, ScannerPayload};

pub struct DataPipeline {
    market_event_rx: mpsc::Receiver<MarketEvent>,
    system_event_rx: mpsc::Receiver<SystemEvent>,
    ws_client: Arc<BinanceWsClient>,
    rest_client: BinanceRestClient,
    db: Arc<Database>,
    global_event_tx: broadcast::Sender<MarketEvent>,
    indicator_engine: Arc<Mutex<IndicatorEngine>>, 
    risk_manager: Arc<Mutex<RiskManager>>,
    metadata_manager: Arc<MetadataManager>,
    breadth_engine: Arc<Mutex<BreadthEngine>>,
    scanner_engine: ScannerEngine,
    symbols: Vec<String>,
    app_handle: AppHandle,
    last_scan_timestamp: Arc<Mutex<i64>>, // Thêm bộ đếm thời gian để throttle
}

impl DataPipeline {
    /// Khởi tạo DataPipeline Orchestrator: Thiết lập hạ tầng truyền tin (IPC), kết nối Exchange Feed và các Engine phân tích định lượng (Quantitative Engines).
    pub fn new(
        symbols: Vec<String>,
        db: Arc<Database>,
        global_event_tx: broadcast::Sender<MarketEvent>,
        app_handle: AppHandle,
    ) -> Self {
        // Phân tách luồng dữ liệu: Market Channel (Dữ liệu giá/OI/Vol) và System Channel (Trạng thái kết nối và độ trễ Feed)
        let (market_tx, market_rx) = mpsc::channel(1000);
        let (system_tx, system_rx) = mpsc::channel(100);

        let rest_client = BinanceRestClient::new();
        let ws_client = BinanceWsClient::new(market_tx, system_tx);
        // Initial symbols update handled below

        let indicator_engine = Arc::new(Mutex::new(IndicatorEngine::new()));

        Self {
            market_event_rx: market_rx,
            system_event_rx: system_rx,
            ws_client: Arc::new(ws_client),
            rest_client: rest_client.clone(),
            db: db.clone(),
            global_event_tx,
            indicator_engine: indicator_engine.clone(),
            risk_manager: Arc::new(Mutex::new(RiskManager::new())),
            metadata_manager: Arc::new(MetadataManager::new(rest_client.clone())),
            breadth_engine: Arc::new(Mutex::new(BreadthEngine::new(rest_client.clone(), db.clone(), app_handle.clone()))),
            scanner_engine: ScannerEngine::new(rest_client, indicator_engine),
            symbols,
            app_handle,
            last_scan_timestamp: Arc::new(Mutex::new(0)),
        }
    }

    /// Kích hoạt chu kỳ sống của Pipeline: Đồng bộ Scanning Universe, nạp dữ liệu lịch sử (Indicator Priming) và thực thi vòng lặp điều phối chính.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Data Pipeline (Phase 0)...");
        
        // Phát tín hiệu khởi tạo ngay lập tức để UI chuyển trạng thái
        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "METADATA".to_string(),
            progress: 5.0,
            message: "System engine starting...".to_string(),
        });

        // 1. [SPEC 2.2] Thiết lập Scanning Universe (Top 100 Altcoins) và xác lập baseline Độ rộng thị trường (Market Breadth) để định danh Market Regime.
        self.sync_metadata_and_breadth().await?;

        // 2. Hâm nóng các Engine Momentum (Warm-up): Tải nến lịch sử để khởi tạo trạng thái cho các chỉ báo Trend (EMA) và Volatility (ATR).
        self.perform_warmup().await?;

        // 3. Worker Risk & News: Theo dõi lịch kinh tế và các biến cố Macro có khả năng gây đột biến biến động (30p/lần).
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

        // 4. Worker Market Universe: Tự động cập nhật danh sách Altcoin dẫn dắt dòng tiền (Top 100) và tái cấu trúc Market Breadth (1h/lần).
        let breadth_engine_clone = Arc::clone(&self.breadth_engine);
        let metadata_manager_clone = Arc::clone(&self.metadata_manager);
        let db_clone_sync = Arc::clone(&self.db);
        let risk_manager_for_total3 = Arc::clone(&self.risk_manager);
        let app_handle_clone = self.app_handle.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                info!("Scheduled sync: Updating Top 100 Symbols and Market Breadth...");
                if let Ok(candidates) = metadata_manager_clone.get_top_altcoins(None).await {
                    let _ = app_handle_clone.emit("market-event", &MarketEvent::UniverseUpdated(candidates.clone()));
                    let _ = db_clone_sync.save_universe_candidates(&candidates).await;
                    let top_100: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
                    let breadth_res = {
                        let engine = breadth_engine_clone.lock().await;
                        engine.calculate_breadth(&top_100).await
                    };
                    
                    let breadth_ema50 = if let Ok((ema50, ema200)) = breadth_res {
                        let mut engine = breadth_engine_clone.lock().await;
                        engine.apply_results(ema50, ema200);
                        ema50
                    } else {
                        0.0
                    };

                    // [SPEC 2.3] Cập nhật xu hướng TOTAL3 (Dòng tiền Altcoin) dựa trên sức mạnh nội tại của thị trường (Breadth).
                    let mut risk = risk_manager_for_total3.lock().await;
                    use crate::core::models::TrendDirection;
                    risk.total3_trend = if breadth_ema50 > 55.0 {
                        TrendDirection::Up
                    } else if breadth_ema50 < 45.0 {
                        TrendDirection::Down
                    } else {
                        TrendDirection::Sideway
                    };
                }
            }
        });

        // 5. Duy trì kết nối WebSocket thời gian thực (TCP stream) để nhận biến động Orderflow và Price Action.
        let ws_client_clone = Arc::clone(&self.ws_client);
        tokio::spawn(async move {
            if let Err(e) = ws_client_clone.run().await {
                error!("WebSocket client stopped with error: {}", e);
            }
        });

        // 6. Vòng lặp điều phối chính: Phân phối sự kiện và kích hoạt Phase 2 (Scanner) khi điều kiện Regime hội tụ.
        let mut regime_rx = self.global_event_tx.subscribe();

        loop {
            tokio::select! {
                // Tiếp nhận dữ liệu OHLCV (Price Action) và Volume từ sàn.
                Some(market_event) = self.market_event_rx.recv() => {
                    self.handle_market_event(market_event).await;
                }
                // Giám sát Telemetry hệ thống: Tình trạng kết nối sàn (Connectivity) và độ trễ luồng dữ liệu (Latency).
                Some(system_event) = self.system_event_rx.recv() => {
                    self.handle_system_event(system_event).await;
                }
                // Lắng nghe tín hiệu Regime từ Phase 1: Mở/Khóa bộ lọc Altcoin dựa trên điều kiện thị trường chung.
                Ok(global_event) = regime_rx.recv() => {
                    if let MarketEvent::RegimeUpdated(context) = global_event {
                        // Đẩy trạng thái Regime (Bullish/Bearish/Sideaway) lên UI Dashboard layer.
                        let _ = self.app_handle.emit("market-event", &MarketEvent::RegimeUpdated(context.clone()));

                        if context.allow_alt_scan {
                            let now = chrono::Utc::now().timestamp();
                            let mut last_scan = self.last_scan_timestamp.lock().await;

                            // [THROTTLE] Giới hạn tần suất quét (900s) để tối ưu hiệu suất và tránh Over-trading do tín hiệu nhiễu.
                            if now - *last_scan >= 900 {
                                info!("Phase 2: Gateway Open & Cooldown finished. Triggering real Altcoin Scan...");
                                *last_scan = now;

                                if let Ok(candidates) = self.metadata_manager.get_top_altcoins(None).await {
                                    let _ = self.app_handle.emit("market-event", &MarketEvent::UniverseUpdated(candidates.clone()));
                                    let _ = self.db.save_universe_candidates(&candidates).await;
                                    let top_altcoins: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
                                    // Fetch bulk 24h tickers để tính toán biến động rolling của toàn bộ Universe.
                                    let tickers_24h = match self.rest_client.fetch_24h_tickers().await {
                                        Ok(t) => t,
                                        Err(e) => {
                                            warn!("Failed to fetch 24h tickers: {}. Skipping scan.", e);
                                            continue;
                                        }
                                    };

                                    // Lấy Snapshot đa chiều: Price, Open Interest (Dòng tiền), EMA (Trend) cho Universe.
                                    let snapshots = self.scanner_engine.fetch_real_snapshots(&top_altcoins, &tickers_24h, self.db.clone(), Arc::clone(&self.risk_manager)).await;

                                    // Chuẩn hóa biến động BTCUSDT làm mốc tham chiếu cho Relative Strength.
                                    let btc_change_1d = tickers_24h.iter()
                                        .find(|t| t["symbol"].as_str() == Some("BTCUSDT"))
                                        .and_then(|t| t["priceChangePercent"].as_str())
                                        .and_then(|s| s.parse::<f64>().ok())
                                        .unwrap_or(0.0);

                                    let btc_4h = self.db.get_candles("BTCUSDT", "4h", 2).await.unwrap_or_default();
                                    let btc_change_4h = if btc_4h.len() >= 2 {
                                        let prev = &btc_4h[btc_4h.len() - 2];
                                        let curr = btc_4h.last().unwrap();
                                        (curr.close - prev.open) / prev.open * 100.0
                                    } else {
                                        btc_4h.last().map(|c| (c.close - c.open) / c.open * 100.0).unwrap_or(0.0)
                                    };

                                    // Thực hiện tính toán ma trận Relative Strength (Z-Score weighting) để tìm Alpha.
                                    let shortlist = self.scanner_engine.scan(&context, btc_change_1d, btc_change_4h, &snapshots);

                                    // Lưu trữ ứng viên (Shortlist) vào Database phục vụ kiểm chứng Entry (Phase 3).
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

                                    let payload = ScannerPayload {
                                        scan_timestamp: now,
                                        shortlist,
                                    };
                                    // Phát tín hiệu kết quả quét lên Dashboard UI và các Module hạ nguồn.
                                    let _ = self.app_handle.emit("market-event", &MarketEvent::ScannerUpdated(payload.clone()));
                                    let _ = self.global_event_tx.send(MarketEvent::ScannerUpdated(payload));
                                }
                            }
                        } else {
                            // Khi Regime không thuận lợi, reset shortlist trên UI để tránh tín hiệu sai lệch.
                            let payload = ScannerPayload {
                                scan_timestamp: chrono::Utc::now().timestamp(),
                                shortlist: vec![],
                            };
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

    /// Lọc danh sách Top 100 Altcoin có thanh khoản cao nhất v    /// Lọc danh sách Top 100 Altcoin có thanh khoản cao nhất và xác lập baseline Độ rộng thị trường (Bullish/Bearish Regime).
    async fn sync_metadata_and_breadth(&mut self) -> Result<()> {
        info!("[PIPELINE] Starting sync_metadata_and_breadth...");

        // Bước 1: Xác định Scanning Universe (25%)
        let candidates = self.metadata_manager.get_top_altcoins(Some(&self.app_handle)).await?;
        let _ = self.app_handle.emit("market-event", &MarketEvent::UniverseUpdated(candidates.clone()));
        let _ = self.db.save_universe_candidates(&candidates).await;
        let top_alts: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
        let universe_set: std::collections::HashSet<String> = top_alts.iter().cloned().collect();
        info!("[PIPELINE] METADATA sync complete. Selected {} coins.", top_alts.len());

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WEBSOCKET".to_string(), progress: 30.0,
            message: "Initializing live stream connections...".to_string(),
        });

        // Bước 2: Khởi động WebSocket Worker (30%)
        let mut all_symbols = vec!["BTCUSDT".to_string()];
        all_symbols.extend(top_alts.clone());
        self.symbols = all_symbols.clone();
        self.ws_client.update_symbols(all_symbols).await;

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "CONTEXT".to_string(), progress: 40.0,
            message: "Fetching Funding & Breadth in parallel...".to_string(),
        });

        // Bước 3 & 4: Song song hóa Funding và Market Breadth (Tiết kiệm thời gian startup)
        let rest = self.rest_client.clone();
        let risk_manager = Arc::clone(&self.risk_manager);
        let breadth_engine = Arc::clone(&self.breadth_engine);
        let top_alts_clone = top_alts.clone();

        let funding_task = async move {
            match rest.fetch_premium_index().await {
                Ok(premiums) => {
                    let mut risk = risk_manager.lock().await;
                    let mut count = 0;
                    for p in premiums {
                        let sym = p["symbol"].as_str().unwrap_or("").to_string();
                        // Tối ưu RAM: Chỉ lưu funding của những coin trong Universe
                        if universe_set.contains(&sym) || sym == "BTCUSDT" {
                            let fr = p["lastFundingRate"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            risk.symbol_funding.insert(sym, fr);
                            count += 1;
                        }
                    }
                    info!("[PIPELINE] Funding rates synchronized for {} symbols.", count);
                }
                Err(e) => warn!("[PIPELINE] Funding sync failed: {}. Continuing...", e),
            }
        };

        let breadth_task = async move {
            let calculation = {
                let engine = breadth_engine.lock().await;
                engine.calculate_breadth(&top_alts_clone).await
            };
            
            if let Ok((ema50, ema200)) = calculation {
                let mut engine = breadth_engine.lock().await;
                engine.apply_results(ema50, ema200);
                ema50
            } else {
                0.0
            }
        };

        // Thực thi song song
        let (_, ema50_val) = tokio::join!(funding_task, breadth_task);

        // Cập nhật TOTAL3 Trend dựa trên kết quả Breadth vừa có
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

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "PHASE0_DONE".to_string(), progress: 70.0,
            message: "Initial Context synchronization complete.".to_string(),
        });

        Ok(())
    }

    /// Backfilling dữ liệu nến (OHLCV) để hâm nóng trạng thái (Priming) cho các Engine tính toán Momentum và Trend.
    async fn perform_warmup(&mut self) -> Result<()> {
        info!("Performing high-performance incremental warm-up...");
        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WARMUP_START".to_string(), progress: 70.0,
            message: "Starting high-speed data warm-up...".to_string(),
        });

        let timeframes = load_timeframes_from_config();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let total_steps = timeframes.len() * self.symbols.len();
        let completed_steps = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Sử dụng Semaphore để quản lý concurrency (limit: 20 parallel requests) tránh bị sàn Rate Limit IP.
        let semaphore = Arc::new(tokio::sync::Semaphore::new(20));
        let mut join_handles = Vec::new();

        for tf in timeframes {
            let tf_str = tf.to_string();
            for symbol in &self.symbols {
                let symbol = symbol.clone();
                let tf = tf_str.clone();
                let db = self.db.clone();
                let rest_client = self.rest_client.clone();
                let indicator_engine = self.indicator_engine.clone();
                let app_handle = self.app_handle.clone();
                let completed_steps = completed_steps.clone();
                let total_steps = total_steps;
                let permit = semaphore.clone().acquire_owned().await.unwrap();

                join_handles.push(tokio::spawn(async move {
                    let _permit = permit; // Giữ permit cho đến khi task backfill hoàn tất.
                    let last_update = db.get_last_update_time(&symbol, &tf).await.unwrap_or(0);
                    let interval_ms = timeframe_to_ms(&tf);

                    // Phát hiện lỗ hổng dữ liệu (Data Gaps) trong DB local.
                    if (now_ms - last_update) > interval_ms {
                        // Tính toán offset nến thiếu và thực thi bù dữ liệu để đảm bảo tính liên tục của Indicator.
                        let missing_candles = ((now_ms - last_update) / interval_ms).min(200) as u32;
                        let fetch_limit = if last_update == 0 { 200 } else { missing_candles + 2 };

                        if fetch_limit > 0 {
                            if let Ok(data) = rest_client.fetch_klines(&symbol, &tf, fetch_limit).await {
                                let mut engine = indicator_engine.lock().await;
                                for c in &data {
                                    if c.close_time < now_ms {
                                        // Feed dữ liệu vào engine để update internal state của EMA/ATR/RSI.
                                        let inds = engine.process(c);
                                        let normalized_data = NormalizedCandleData {
                                            candle: c.clone(),
                                            indicators: inds,
                                            ..Default::default()
                                        };
                                        let _ = db.insert_closed_candle(&normalized_data).await;
                                    }
                                }
                            }
                        }
                    } else {
                        // Nếu dữ liệu up-to-date, nạp nến gần nhất để đồng bộ lại trạng thái Indicator.
                        let candles = db.get_candles(&symbol, &tf, 200).await.unwrap_or_default();
                        let mut engine = indicator_engine.lock().await;
                        for candle in candles {
                            let _ = engine.process(&candle);
                        }
                    }

                    // Report tiến độ khởi tạo hệ thống lên UI.
                    let done = completed_steps.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    let progress = 70.0 + (done as f64 / total_steps as f64) * 30.0;
                    let _ = app_handle.emit("market-event", &MarketEvent::SyncProgress {
                        step: "WARMUP".to_string(),
                        progress,
                        message: format!("Syncing {} {}: {}/{}", symbol, tf, done, total_steps),
                    });
                }));
            }
        }

                // Đợi ranh giới đồng bộ (Sync boundary) hoàn tất cho toàn bộ Universe.
        for handle in join_handles {
            let _ = handle.await;
        }

        // [QUAN TRỌNG] Kích hoạt phân tích Market Regime ban đầu ngay sau khi Warmup hoàn tất
        // Điều này đảm bảo FLOW SCORE và các chỉ số bối cảnh được tính toán XONG trước khi tắt Loading Overlay.
                // [QUAN TRỌNG] Kích hoạt phân tích Market Regime đa khung thời gian ngay sau khi Warmup hoàn tất
        // Điều này đảm bảo FLOW SCORE và các chỉ số bối cảnh được tính toán CHUẨN XÁC trước khi tắt Loading Overlay.
        info!("[PIPELINE] Warmup complete. Priming Market Regime Engine with BTC Multi-timeframe context...");
        
        let tfs = ["1d", "4h", "15m"];
        for tf in tfs {
            if let Ok(btc_data) = self.db.get_candles_with_indicators("BTCUSDT", tf, 1).await {
                if let Some(mut data) = btc_data.into_iter().next() {
                    let mut risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;
                    
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    data.macro_events = risk.get_macro_events().await;
                    
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                    
                    // Phát sự kiện giả lập để cập nhật trạng thái nội tại của RegimeEngine
                    let _ = self.global_event_tx.send(MarketEvent::CandleClosed(data));
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
        
        // Đợi thêm một chút để đảm bảo UI nhận được tín hiệu RegimeUpdated cuối cùng từ nến 15m
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WARMUP_DONE".to_string(),
            progress: 100.0,
            message: "System fully synchronized.".to_string(),
        });

        Ok(())
    }

    /// Dispatcher: Phân phối và xử lý đa luồng dữ liệu (Price Action, Liquidation, Funding, Open Interest).
    pub async fn handle_market_event(&mut self, event: MarketEvent) {
        match event {
            MarketEvent::CandleClosed(mut data) => {
                // [SPEC 2.2] Gap Reconciliation: Tự động bù nến hụt để bảo đảm tính đúng đắn của chỉ báo kỹ thuật.
                if let Err(e) = self.fill_gaps(&data.candle.symbol, &data.candle.timeframe, data.candle.open_time).await {
                    error!("Gap filling error: {}", e);
                }

                // Chốt chỉ báo kỹ thuật tại biên đóng nến (Closed boundary).
                let mut engine = self.indicator_engine.lock().await;
                data.indicators = engine.process(&data.candle);

                let alt_tf = crate::core::config::AppConfig::load().altcoin_analysis_timeframe;
                if data.candle.timeframe == alt_tf {
                    tracing::info!("[PIPELINE] Altcoin analysis timeframe candle closed: {} [{}]", data.candle.symbol, data.candle.timeframe);
                    data.range_24h_pct = (data.candle.high - data.candle.low) / data.candle.open;
                    data.range_p40_90d = self.db.get_p40_range_90d(&data.candle.symbol).await.unwrap_or(0.0);
                }

                {
                    let mut risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;

                    // Phân tích rủi ro vi mô (Microstructure): Biến động dòng tiền (OI) so với biến động giá (ATR).
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    data.macro_events = risk.get_macro_events().await;

                    if data.candle.timeframe == "4h" {
                        risk.snapshot_4h_oi(&data.candle.symbol);
                    }

                    // Tích hợp chỉ số thị trường chung (Breadth) vào payload dữ liệu.
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                }

                info!("[CONFIRMED] {} - {}: C: {} OI Change: {:.2}%",
                    data.candle.symbol, data.candle.timeframe, data.candle.close, data.microstructure.oi_change_4h_pct);

                // Commit dữ liệu xác nhận đóng vào DB.
                if let Err(e) = self.db.insert_closed_candle(&data).await {
                    error!("Failed to save closed candle to DB: {}", e);
                }

                // Broadcast sự kiện nến đóng lên Dashboard UI và system bus.
                let _ = self.app_handle.emit("market-event", &MarketEvent::CandleClosed(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleClosed(data));
            }

            MarketEvent::CandleUpdated(mut data) => {
                {
                    let mut risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;
                    let mut engine = self.indicator_engine.lock().await;

                    // Tính toán chênh lệch lực mua/bán chủ động (CVD - Cumulative Volume Delta).
                    let cvd = data.candle.taker_buy_volume * 2.0 - data.candle.volume;
                    if data.candle.timeframe == "4h" {
                        risk.symbol_cvd_4h.insert(data.candle.symbol.clone(), cvd);
                    } else if data.candle.timeframe == "1d" {
                        risk.symbol_cvd_1d.insert(data.candle.symbol.clone(), cvd);
                    }

                    // Tính toán chỉ báo tạm thời cho nến đang chạy (Pending boundary).
                    data.indicators = engine.process_unclosed(&data.candle);
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);

                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                }

                // Push cập nhật real-time phục vụ Dashboard responsiveness.
                let _ = self.app_handle.emit("market-event", &MarketEvent::CandleUpdated(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleUpdated(data));
            }
            MarketEvent::DepthUpdated { symbol, is_liquidation, price, value_usd, timestamp: _ } => {
                let mut risk = self.risk_manager.lock().await;
                if !is_liquidation {
                    // Cập nhật biến động Open Interest (Dòng tiền thực).
                    risk.update_oi(symbol, value_usd);
                } else {
                    // Theo dõi thanh lý lớn (Large Liquidations) để xác định các vùng hỗ trợ/kháng cự động.
                    risk.recent_liquidations_usd += value_usd;
                    if value_usd > 100_000.0 {
                        let current_upper = *risk.symbol_liq_upper.get(&symbol).unwrap_or(&0.0);
                        if current_upper == 0.0 || price > current_upper {
                            risk.symbol_liq_upper.insert(symbol.clone(), price);
                        } else {
                            risk.symbol_liq_lower.insert(symbol.clone(), price);
                        }
                    }
                }
            }
            MarketEvent::FundingUpdated { symbol, funding_rate, timestamp: _ } => {
                let mut risk = self.risk_manager.lock().await;
                if symbol == "BTCDOMUSDT" {
                    risk.btc_dominance = funding_rate;
                } else {
                    risk.symbol_funding.insert(symbol, funding_rate);
                }
            }
            _ => {
                let _ = self.global_event_tx.send(event);
            }
        }
    }

    /// Tiếp nhận Telemetry về trạng thái kết nối sàn (Connectivity) và các tín hiệu Health từ background workers.
    async fn handle_system_event(&self, event: SystemEvent) {
        match event {
            SystemEvent::HealthChanged { previous, current, timestamp } => {
                info!("[SYSTEM HEALTH] State changed from {:?} to {:?} at {}", previous, current, timestamp);
            }
            _ => {}
        }
    }

    /// [SPEC 2.2] Gap Reconciliation: Tự động phát hiện và bù dữ liệu OHLCV bị thiếu để bảo toàn tính toán của Indicator Engine.
    async fn fill_gaps(&mut self, symbol: &str, timeframe: &str, current_open_time: i64) -> Result<()> {
        let last_stored = self.db.get_last_update_time(symbol, timeframe).await?;
        if last_stored == 0 { return Ok(()); }

        let interval_ms = timeframe_to_ms(timeframe);

        // Phát hiện gap hụt dữ liệu > 1.5 chu kỳ nến.
        let gap = current_open_time - last_stored;
        if gap > (interval_ms as f64 * 1.5) as i64 {
            let missing_count = (gap / interval_ms) as u32;
            warn!("Gap filling for {} {}: {} candles", symbol, timeframe, missing_count);
            // Thực thi backfill qua REST API để đảm bảo tính liên tục của chuỗi thời gian (Time-series integrity).
            if let Ok(missing) = self.rest_client.fetch_klines(symbol, timeframe, missing_count + 1).await {
                for c in missing {
                    if c.open_time > last_stored && c.open_time < current_open_time {
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

fn load_timeframes_from_config() -> Vec<String> {
    crate::core::config::AppConfig::load().timeframes
}

pub fn timeframe_to_ms(tf: &str) -> i64 {
    let num_str: String = tf.chars().take_while(|c| c.is_numeric()).collect();
    let unit: String = tf.chars().skip_while(|c| c.is_numeric()).collect();
    let num = num_str.parse::<i64>().unwrap_or(1);
    match unit.as_str() {
        "m" => num * 60 * 1000,
        "h" => num * 60 * 60 * 1000,
        "d" => num * 24 * 60 * 60 * 1000,
        "w" => num * 7 * 24 * 60 * 60 * 1000,
        _ => 3600_000,
    }
}
