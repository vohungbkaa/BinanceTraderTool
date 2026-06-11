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
    ws_client: Arc<Mutex<BinanceWsClient>>,
    rest_client: BinanceRestClient,
    db: Arc<Database>,
    global_event_tx: broadcast::Sender<MarketEvent>,
    indicator_engine: Arc<Mutex<IndicatorEngine>>, 
    risk_manager: Arc<Mutex<RiskManager>>,
    metadata_manager: MetadataManager,
    breadth_engine: Arc<Mutex<BreadthEngine>>,
    scanner_engine: ScannerEngine,
    symbols: Vec<String>,
    app_handle: AppHandle,
    last_scan_timestamp: Arc<Mutex<i64>>, // Thêm bộ đếm thời gian để throttle
}

impl DataPipeline {
    /// [CONSTRUCTOR] Khởi tạo bộ máy DataPipeline.
    /// Tương đương với việc tạo một 'class' DataPipeline trong Java/Kotlin.
    pub fn new(
        symbols: Vec<String>, 
        db: Arc<Database>, 
        global_event_tx: broadcast::Sender<MarketEvent>,
        app_handle: AppHandle,
    ) -> Self {
        // Tạo các kênh truyền tin nội bộ (MPSC) để nhận dữ liệu từ WebSocket Thread.
        let (market_tx, market_rx) = mpsc::channel(1000);
        let (system_tx, system_rx) = mpsc::channel(100);

        let rest_client = BinanceRestClient::new();
        let mut ws_client = BinanceWsClient::new(market_tx, system_tx);
        ws_client.update_symbols(symbols.clone());
        
        let indicator_engine = Arc::new(Mutex::new(IndicatorEngine::new()));

        // Trả về một thực thể (instance) hoàn chỉnh.
        Self {
            market_event_rx: market_rx,
            system_event_rx: system_rx,
            ws_client: Arc::new(Mutex::new(ws_client)),
            rest_client: rest_client.clone(),
            db: db.clone(),
            global_event_tx,
            indicator_engine: indicator_engine.clone(),
            risk_manager: Arc::new(Mutex::new(RiskManager::new())),
            metadata_manager: MetadataManager::new(rest_client.clone()),
            breadth_engine: Arc::new(Mutex::new(BreadthEngine::new(rest_client.clone(), db.clone(), app_handle.clone()))),
            scanner_engine: ScannerEngine::new(rest_client, indicator_engine),
            symbols,
            app_handle,
            last_scan_timestamp: Arc::new(Mutex::new(0)),
        }
    }

    /// [PHASE 0 ENTRY POINT] Khởi động hệ thống nạp và điều phối dữ liệu.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Data Pipeline (Phase 0)...");

        // 1. [INITIALIZATION] Đồng bộ danh sách Coin và tính Market Breadth ban đầu.
        // Đây là bước "Kiểm tra thời tiết" trước khi cất cánh.
        self.sync_metadata_and_breadth().await?;

        // 2. [WARMUP] Tải nến cũ từ Database hoặc Binance.
        // Đảm bảo các chỉ báo kỹ thuật (EMA, ADX...) có đủ dữ liệu lịch sử để tính toán chính xác.
        self.perform_warmup().await?;

        // 3. [NEWS LOOP] Khởi chạy vòng lặp cập nhật tin tức kinh tế (Mỗi 30 phút).
        let risk_manager_clone = Arc::clone(&self.risk_manager);
        tokio::spawn(async move {
            loop {
                {
                    let risk = risk_manager_clone.lock().await;
                    let _ = risk.update_economic_calendar().await;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1800)).await;
            }
        });

        // 4. [SCHEDULED SYNC] Cập nhật định kỳ danh sách Top 100 và Market Breadth (Mỗi 1 tiếng).
        let breadth_engine_clone = Arc::clone(&self.breadth_engine);
        let metadata_manager_clone = MetadataManager::new(self.rest_client.clone());
        let risk_manager_for_total3 = Arc::clone(&self.risk_manager);
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                info!("Scheduled sync: Updating Top 100 Symbols and Market Breadth...");
                if let Ok(top_100) = metadata_manager_clone.get_top_altcoins().await {
                    let breadth_ema50 = {
                        let mut engine = breadth_engine_clone.lock().await;
                        let _ = engine.update_breadth(&top_100).await;
                        engine.market_breadth_ema50
                    };
                    
                    let mut risk = risk_manager_for_total3.lock().await;
                    use crate::core::models::TrendDirection;
                    risk.total3_trend = if breadth_ema50 > 55.0 { TrendDirection::Up } else if breadth_ema50 < 45.0 { TrendDirection::Down } else { TrendDirection::Sideway };
                }
            }
        });

        // 5. [WEBSOCKET] Mở "vòi nước" dữ liệu thời gian thực từ sàn Binance.
        let ws_client_clone = Arc::clone(&self.ws_client);
        tokio::spawn(async move {
            let mut client = ws_client_clone.lock().await;
            if let Err(e) = client.run().await {
                error!("WebSocket client stopped: {}", e);
            }
        });

        // 6. [MAIN EVENT LOOP] Vòng lặp sự kiện chính (Trạm trực chiến).
        // Lắng nghe dữ liệu từ sàn và lệnh từ Phase 1 để kích hoạt Phase 2.
        let mut regime_rx = self.global_event_tx.subscribe();
        
        loop {
            tokio::select! {
                // A. Nhận dữ liệu nến/giá từ WebSocket
                Some(market_event) = self.market_event_rx.recv() => {
                    self.handle_market_event(market_event).await;
                }
                // B. Nhận tin nhắn cập nhật trạng thái thị trường từ Phase 1 (Regime Engine)
                Ok(global_event) = regime_rx.recv() => {
                    if let MarketEvent::RegimeUpdated(context) = global_event {
                        // Forward trạng thái lên UI để người dùng thấy BLOCKED/ENABLED
                        let _ = self.app_handle.emit("market-event", &MarketEvent::RegimeUpdated(context.clone()));

                        // Nếu Phase 1 cho phép -> Kích hoạt PHASE 2: Altcoin Scanner
                        if context.allow_alt_scan {
                            let now = chrono::Utc::now().timestamp();
                            let mut last_scan = self.last_scan_timestamp.lock().await;
                            
                            // [THROTTLE] Giới hạn tần suất quét (Ví dụ: 15 phút/lần) để tránh nhiễu.
                            if now - *last_scan >= 900 {
                                *last_scan = now;
                                self.trigger_altcoin_scan(context).await;
                            }
                        }
                    }
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn sync_metadata_and_breadth(&mut self) -> Result<()> {
        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "METADATA".to_string(),
            progress: 10.0,
            message: "Filtering top 100 high-quality altcoins...".to_string(),
        });
        
        let top_alts = self.metadata_manager.get_top_altcoins().await?;
        
        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WEBSOCKET".to_string(),
            progress: 30.0,
            message: "Initializing live stream connections...".to_string(),
        });

        // Cập nhật danh sách symbol cho Pipeline (bao gồm BTCUSDT + các Altcoin cấu hình)
        let mut all_symbols = vec!["BTCUSDT".to_string()];
        all_symbols.extend(top_alts.clone());
        self.symbols = all_symbols.clone();
        
        // Cập nhật WS Client để nhận live data cho TẤT CẢ timeframes của TẤT CẢ altcoins
        {
            let mut ws = self.ws_client.lock().await;
            ws.update_symbols(all_symbols);
        }

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "FUNDING".to_string(),
            progress: 50.0,
            message: "Fetching initial funding rates and open interest...".to_string(),
        });

        // Lấy Funding Rates ban đầu
        if let Ok(premiums) = self.rest_client.fetch_premium_index().await {
            let mut risk = self.risk_manager.lock().await;
            for p in premiums {
                let sym = p["symbol"].as_str().unwrap_or("").to_string();
                let fr = p["lastFundingRate"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                risk.symbol_funding.insert(sym, fr);
            }
        }

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "BREADTH".to_string(),
            progress: 70.0,
            message: "Calculating Market Breadth for top 100 altcoins...".to_string(),
        });

        let mut breadth = self.breadth_engine.lock().await;
        let _ = breadth.update_breadth(&top_alts).await;
        let ema50_val = breadth.market_breadth_ema50;
        
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
            step: "BREADTH_DONE".to_string(),
            progress: 100.0,
            message: "Market Breadth sync complete.".to_string(),
        });

        Ok(())
    }

    async fn perform_warmup(&mut self) -> Result<()> {
        info!("Performing intelligent warm-up (DB Cache First)...");
        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WARMUP_START".to_string(),
            progress: 0.0,
            message: "Starting intelligent data warm-up...".to_string(),
        });

        let timeframes = load_timeframes_from_config();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let total_steps = timeframes.len() * self.symbols.len();
        let mut completed_steps = 0;

        for tf in &timeframes {
            let tf_str = tf.to_string();
            // Chia lô 5 symbols tải song song để tối ưu tốc độ và Rate Limit
            for chunk in self.symbols.chunks(5) {
                let mut tasks = Vec::new();
                for symbol in chunk {
                    let symbol = symbol.clone();
                    let tf = tf_str.clone();
                    let db = self.db.clone();
                    let rest_client = self.rest_client.clone();
                    let indicator_engine = self.indicator_engine.clone();

                    tasks.push(tokio::spawn(async move {
                        let last_update = db.get_last_update_time(&symbol, &tf).await.unwrap_or(0);
                        
                        let interval_ms = timeframe_to_ms(&tf);
                        
                        let is_fresh = (now_ms - last_update) < interval_ms;
                        let candles = db.get_candles(&symbol, &tf, 200).await.unwrap_or_default();
                        let has_enough = candles.len() >= 200;

                        if is_fresh && has_enough {
                            for candle in candles {
                                let mut engine = indicator_engine.lock().await;
                                let inds = engine.process(&candle);
                                let data = NormalizedCandleData {
                                    candle: candle.clone(),
                                    indicators: inds,
                                    ..Default::default()
                                };
                                let _ = db.insert_closed_candle(&data).await;
                            }
                            info!("Warm-up complete for {} {} (Used DB Cache & Updated Indicators)", symbol, tf);
                        } else {
                            info!("Fetching fresh data for {} {}: is_fresh={}, count={}", symbol, tf, is_fresh, candles.len());
                            match rest_client.fetch_klines(&symbol, &tf, 200).await {
                                Ok(data) => {
                                    let now_ms = chrono::Utc::now().timestamp_millis();
                                    for c in &data {
                                        // CHỈ lưu vào DB nếu nến ĐÃ THỰC SỰ KẾT THÚC (close_time < now)
                                        if c.close_time < now_ms {
                                            let mut engine = indicator_engine.lock().await;
                                            let inds = engine.process(c);
                                            let normalized_data = NormalizedCandleData {
                                                candle: c.clone(),
                                                indicators: inds,
                                                ..Default::default()
                                            };
                                            let _ = db.insert_closed_candle(&normalized_data).await;
                                        }
                                    }
                                    info!("Warm-up complete for {} {} (Fetched from Binance & Saved)", symbol, tf);
                                }
                                Err(e) => {
                                    warn!("Failed to fetch {} {}: {}", symbol, tf, e);
                                }
                            }
                        }
                    }));
                }

                // Đợi tất cả tasks trong lô chạy xong
                for task in tasks {
                    let _ = task.await;
                    completed_steps += 1;
                    
                    let progress = (completed_steps as f64 / total_steps as f64) * 100.0;
                    let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
                        step: "WARMUP".to_string(),
                        progress,
                        message: format!("Warming up: {} steps/{}", completed_steps, total_steps),
                    });
                }
                
                // Sleep nhẹ để reset Rate Limit (rất an toàn cho Binance Futures)
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }

        let _ = self.app_handle.emit("market-event", &MarketEvent::SyncProgress {
            step: "WARMUP_DONE".to_string(),
            progress: 100.0,
            message: "All symbols warmed up and ready.".to_string(),
        });

        Ok(())
    }

    pub async fn handle_market_event(&mut self, event: MarketEvent) {
        match event {
            MarketEvent::CandleClosed(mut data) => {
                // [SPEC 2.2] Gap Filling: Kiểm tra xem nến mới nhận có bị hụt so với DB không
                if let Err(e) = self.fill_gaps(&data.candle.symbol, &data.candle.timeframe, data.candle.open_time).await {
                    error!("Gap filling error: {}", e);
                }

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
                    
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    data.macro_events = risk.get_macro_events().await;
                    
                    if data.candle.timeframe == "4h" {
                        risk.snapshot_4h_oi(&data.candle.symbol);
                    }
                    
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                    
                    // Tính toán Price Change 4h pct cho microstructure
                    // data.microstructure.price_change_4h_pct = ... (logic tính toán từ cache)
                }

                info!("[CONFIRMED] {} - {}: C: {} OI Change: {:.2}%", 
                    data.candle.symbol, data.candle.timeframe, data.candle.close, data.microstructure.oi_change_4h_pct);
                
                if let Err(e) = self.db.insert_closed_candle(&data).await {
                    error!("Failed to save closed candle to DB: {}", e);
                }

                let _ = self.app_handle.emit("market-event", &MarketEvent::CandleClosed(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleClosed(data));
            }

            MarketEvent::CandleUpdated(mut data) => {
                {
                    let mut risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;
                    let mut engine = self.indicator_engine.lock().await;
                    
                    let cvd = data.candle.taker_buy_volume * 2.0 - data.candle.volume;
                    if data.candle.timeframe == "4h" {
                        risk.symbol_cvd_4h.insert(data.candle.symbol.clone(), cvd);
                    } else if data.candle.timeframe == "1d" {
                        risk.symbol_cvd_1d.insert(data.candle.symbol.clone(), cvd);
                    }

                    data.indicators = engine.process_unclosed(&data.candle);
                    let atr = data.indicators.atr14.unwrap_or(data.candle.close * 0.02);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol, data.candle.close, atr);
                    
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                }
                
                let _ = self.app_handle.emit("market-event", &MarketEvent::CandleUpdated(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleUpdated(data));
            }
            MarketEvent::DepthUpdated { symbol, is_liquidation, price, value_usd, timestamp: _ } => {
                let mut risk = self.risk_manager.lock().await;
                if !is_liquidation {
                    risk.update_oi(symbol, value_usd);
                } else {
                    risk.recent_liquidations_usd += value_usd;
                    // Tích lũy vào cluster dựa trên việc So sánh giá thanh lý với giá hiện tại (giả định dùng giá thanh lý làm mốc)
                    // Đây là logic đơn giản: lấy giá thanh lý làm cluster
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

    async fn handle_system_event(&self, event: SystemEvent) {
        match event {
            SystemEvent::HealthChanged { previous, current, timestamp } => {
                info!("[SYSTEM HEALTH] State changed from {:?} to {:?} at {}", previous, current, timestamp);
            }
            _ => {}
        }
    }

    /// [SPEC 2.2] Tự động bù dữ liệu thiếu
    async fn fill_gaps(&mut self, symbol: &str, timeframe: &str, current_open_time: i64) -> Result<()> {
        let last_stored = self.db.get_last_update_time(symbol, timeframe).await?;
        if last_stored == 0 { return Ok(()); }

        let interval_ms = timeframe_to_ms(timeframe);

        let gap = current_open_time - last_stored;
        if gap > (interval_ms as f64 * 1.5) as i64 {
            let missing_count = (gap / interval_ms) as u32;
            warn!("Gap filling for {} {}: {} candles", symbol, timeframe, missing_count);
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

fn timeframe_to_ms(tf: &str) -> i64 {
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
