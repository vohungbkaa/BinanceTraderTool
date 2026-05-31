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
use crate::core::models::NormalizedCandleData;

pub struct DataPipeline {
    market_event_rx: mpsc::Receiver<MarketEvent>,
    system_event_rx: mpsc::Receiver<SystemEvent>,
    ws_client: Arc<Mutex<BinanceWsClient>>,
    rest_client: BinanceRestClient,
    db: Arc<Database>,
    global_event_tx: broadcast::Sender<MarketEvent>,
    indicator_engine: IndicatorEngine,
    risk_manager: Arc<Mutex<RiskManager>>,
    metadata_manager: MetadataManager,
    breadth_engine: Arc<Mutex<BreadthEngine>>,
    symbols: Vec<String>,
    app_handle: AppHandle,
}

impl DataPipeline {
    pub fn new(
        symbols: Vec<String>, 
        db: Arc<Database>, 
        global_event_tx: broadcast::Sender<MarketEvent>,
        app_handle: AppHandle,
    ) -> Self {
        let (market_tx, market_rx) = mpsc::channel(1000);
        let (system_tx, system_rx) = mpsc::channel(100);

        let rest_client = BinanceRestClient::new();
        let mut ws_client = BinanceWsClient::new(market_tx, system_tx);
        ws_client.update_symbols(symbols.clone());

        Self {
            market_event_rx: market_rx,
            system_event_rx: system_rx,
            ws_client: Arc::new(Mutex::new(ws_client)),
            rest_client: rest_client.clone(),
            db: db.clone(),
            global_event_tx,
            indicator_engine: IndicatorEngine::new(),
            risk_manager: Arc::new(Mutex::new(RiskManager::new())),
            metadata_manager: MetadataManager::new(rest_client.clone()),
            breadth_engine: Arc::new(Mutex::new(BreadthEngine::new(rest_client, db.clone()))),
            symbols,
            app_handle,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Data Pipeline (Phase 0)...");

        // 1. [SPEC 2.2] Metadata Sync & Market Breadth Initial Calculation
        self.sync_metadata_and_breadth().await?;

        // 2. Intelligent Warm-up
        self.perform_warmup().await?;

        // 3. Start News/Risk Update loop (30 minutes interval)
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

        // 4. Start Metadata & Breadth Sync loop
        let breadth_engine_clone = Arc::clone(&self.breadth_engine);
        let metadata_manager_clone = MetadataManager::new(self.rest_client.clone());
        let risk_manager_for_total3 = Arc::clone(&self.risk_manager);
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                info!("Scheduled sync: Updating Top 100 Symbols and Market Breadth...");
                if let Ok(top_100) = metadata_manager_clone.get_top_altcoins().await {
                    let mut engine = breadth_engine_clone.lock().await;
                    let _ = engine.update_breadth(&top_100).await;
                    
                    // [SPEC 2.3] Cập nhật xu hướng TOTAL3 (Ước tính dựa trên Breadth)
                    let mut risk = risk_manager_for_total3.lock().await;
                    use crate::core::models::TrendDirection;
                    risk.total3_trend = if engine.market_breadth_ema50 > 50.0 { TrendDirection::Up } else { TrendDirection::Down };
                }
            }
        });

        // 5. Start WebSocket client
        let ws_client_clone = Arc::clone(&self.ws_client);
        tokio::spawn(async move {
            let mut client = ws_client_clone.lock().await;
            if let Err(e) = client.run().await {
                error!("WebSocket client stopped with error: {}", e);
            }
        });

        // 6. Main event loop
        loop {
            tokio::select! {
                Some(market_event) = self.market_event_rx.recv() => {
                    self.handle_market_event(market_event).await;
                }
                Some(system_event) = self.system_event_rx.recv() => {
                    self.handle_system_event(system_event).await;
                }
                else => {
                    info!("Pipeline channels closed. Exiting...");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn sync_metadata_and_breadth(&mut self) -> Result<()> {
        let top_100 = self.metadata_manager.get_top_altcoins().await?;
        let mut breadth = self.breadth_engine.lock().await;
        breadth.update_breadth(&top_100).await?;
        Ok(())
    }

    async fn perform_warmup(&mut self) -> Result<()> {
        info!("Performing intelligent warm-up (DB Cache First)...");
        let timeframes = vec!["15m", "4h", "1d"];
        let now_ms = chrono::Utc::now().timestamp_millis();

        for symbol in &self.symbols {
            for tf in &timeframes {
                let last_update = self.db.get_last_update_time(symbol, tf).await.unwrap_or(0);
                
                // [SPEC 2.2] Freshness threshold dựa trên timeframe
                let interval_ms = match *tf {
                    "15m" => 15 * 60 * 1000,
                    "4h" => 4 * 60 * 60 * 1000,
                    "1d" => 24 * 60 * 60 * 1000,
                    _ => 3600_000,
                };
                
                let is_fresh = (now_ms - last_update) < interval_ms;
                let candles = self.db.get_candles(symbol, tf, 200).await?;
                let has_enough = candles.len() >= 200;

                if is_fresh && has_enough {
                    for candle in candles {
                        let inds = self.indicator_engine.process(&candle);
                        // [FIX] Cập nhật lại indicators vào DB nếu chúng đang bị NULL
                        let data = NormalizedCandleData {
                            candle: candle.clone(),
                            indicators: inds,
                            ..Default::default()
                        };
                        let _ = self.db.insert_closed_candle(&data).await;
                    }
                    info!("Warm-up complete for {} {} (Used DB Cache & Updated Indicators)", symbol, tf);
                } else {
                    info!("Fetching fresh data for {} {}: is_fresh={}, count={}", symbol, tf, is_fresh, candles.len());
                    match self.rest_client.fetch_klines(symbol, tf, 200).await {
                        Ok(data) => {
                            for c in &data {
                                let inds = self.indicator_engine.process(c);
                                let mock_normalized = NormalizedCandleData {
                                    candle: c.clone(),
                                    indicators: inds,
                                    ..Default::default()
                                };
                                let _ = self.db.insert_closed_candle(&mock_normalized).await;
                            }
                            info!("Warm-up complete for {} {} (Fetched from Binance & Saved)", symbol, tf);
                        }
                        Err(e) => {
                            warn!("Failed to fetch {} {}: {}", symbol, tf, e);
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }
        }
        Ok(())
    }

    pub async fn handle_market_event(&mut self, event: MarketEvent) {
        match event {
            MarketEvent::CandleClosed(mut data) => {
                // [SPEC 2.2] Gap Filling: Kiểm tra xem nến mới nhận có bị hụt so với DB không
                if let Err(e) = self.fill_gaps(&data.candle.symbol, &data.candle.timeframe, data.candle.open_time).await {
                    error!("Gap filling error: {}", e);
                }

                data.indicators = self.indicator_engine.process(&data.candle);
                
                if data.candle.timeframe == "1d" {
                    data.range_24h_pct = (data.candle.high - data.candle.low) / data.candle.open;
                    data.range_p40_90d = self.db.get_p40_range_90d(&data.candle.symbol).await.unwrap_or(0.0);
                }

                {
                    let mut risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;
                    
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol);
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
                    let risk = self.risk_manager.lock().await;
                    let breadth = self.breadth_engine.lock().await;
                    data.indicators = self.indicator_engine.process_unclosed(&data.candle);
                    data.microstructure = risk.get_microstructure_risk(&data.candle.symbol);
                    
                    let mut indices = risk.get_market_indices();
                    indices.market_breadth_pct_above_ema50 = breadth.market_breadth_ema50;
                    indices.market_breadth_pct_above_ema200 = breadth.market_breadth_ema200;
                    data.market_indices = indices;
                }
                
                let _ = self.app_handle.emit("market-event", &MarketEvent::CandleUpdated(data.clone()));
                let _ = self.global_event_tx.send(MarketEvent::CandleUpdated(data));
            }
            MarketEvent::DepthUpdated { symbol, spread_bps, liquidity_score, timestamp: _ } => {
                let mut risk = self.risk_manager.lock().await;
                if spread_bps > 0.5 {
                    risk.update_oi(symbol, liquidity_score);
                } else {
                    risk.recent_liquidations_usd += liquidity_score;
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

        let interval_ms = match timeframe {
            "15m" => 15 * 60 * 1000,
            "4h" => 4 * 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60000,
        };

        let gap = current_open_time - last_stored;
        if gap > (interval_ms as f64 * 1.5) as i64 {
            let missing_count = (gap / interval_ms) as u32;
            warn!("Gap filling for {} {}: {} candles", symbol, timeframe, missing_count);
            if let Ok(missing) = self.rest_client.fetch_klines(symbol, timeframe, missing_count + 1).await {
                for c in missing {
                    if c.open_time > last_stored && c.open_time < current_open_time {
                        let d = NormalizedCandleData {
                            candle: c.clone(),
                            indicators: self.indicator_engine.process(&c),
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
