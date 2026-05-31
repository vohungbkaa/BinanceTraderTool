use crate::core::models::{MacroEvents, Microstructure, MarketIndices};
use reqwest::Client;
use serde::Deserialize;
use tracing::{info};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
struct EconomicEvent {
    pub title: String,
    pub country: String,
    pub impact: String,
    pub date: String,
    pub minutes_until: i64,
}

pub struct RiskManager {
    client: Client,
    pub recent_liquidations_usd: f64,
    cached_events: Arc<Mutex<Vec<EconomicEvent>>>,
    
    // [SPEC 2.3] State cho Vị thế & Dòng tiền
    pub btc_dominance: f64,
    pub symbol_oi: HashMap<String, f64>,
    pub symbol_oi_prev: HashMap<String, f64>,
    pub symbol_funding: HashMap<String, f64>,
    
    // [SPEC 2.3] TOTAL3 / BTC Approximation trend
    pub total3_trend: String,
}

impl RiskManager {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            recent_liquidations_usd: 0.0,
            cached_events: Arc::new(Mutex::new(Vec::new())),
            btc_dominance: 0.0,
            symbol_oi: HashMap::new(),
            symbol_oi_prev: HashMap::new(),
            symbol_funding: HashMap::new(),
            total3_trend: "SIDEWAY".to_string(),
        }
    }

    pub async fn update_economic_calendar(&self) -> anyhow::Result<()> {
        let mut events = self.cached_events.lock().await;
        events.clear();
        events.push(EconomicEvent {
            title: "FOMC Meeting".to_string(),
            country: "USD".to_string(),
            impact: "High".to_string(),
            date: "".to_string(),
            minutes_until: 120,
        });
        Ok(())
    }

    pub async fn get_macro_events(&self) -> MacroEvents {
        let events = self.cached_events.lock().await;
        let high_impact = events.iter()
            .filter(|e| e.impact == "High" && e.country == "USD")
            .min_by_key(|e| e.minutes_until.abs());

        if let Some(event) = high_impact {
            let is_block = event.minutes_until > -30 && event.minutes_until < 30;
            MacroEvents {
                next_event_name: event.title.clone(),
                time_to_event_minutes: event.minutes_until,
                is_event_block_window: is_block,
            }
        } else {
            MacroEvents {
                next_event_name: "NONE".to_string(),
                time_to_event_minutes: 999,
                is_event_block_window: false,
            }
        }
    }

    /// [SPEC 2.3] Cập nhật OI và lưu vết để tính thay đổi %
    pub fn update_oi(&mut self, symbol: String, current_oi: f64) {
        if let Some(old_oi) = self.symbol_oi.get(&symbol) {
            self.symbol_oi_prev.insert(symbol.clone(), *old_oi);
        }
        self.symbol_oi.insert(symbol, current_oi);
    }

    /// [SPEC 2.2 & 2.3] Trả về dữ liệu vị thế
    pub fn get_microstructure_risk(&self, symbol: &str) -> Microstructure {
        let oi_change = if let (Some(curr), Some(prev)) = (self.symbol_oi.get(symbol), self.symbol_oi_prev.get(symbol)) {
            if *prev > 0.0 { (curr - prev) / prev * 100.0 } else { 0.0 }
        } else { 0.0 };

        Microstructure {
            oi_change_4h_pct: oi_change,
            price_change_4h_pct: 0.0, // Sẽ được cập nhật từ Pipeline/Candle
            funding_rate_avg: *self.symbol_funding.get(symbol).unwrap_or(&0.01),
            liquidation_surge_detected: self.recent_liquidations_usd > 10_000_000.0,
            spread_anomaly: false,
        }
    }

    /// [SPEC 2.3] Lấy chỉ số thị trường (BTC.D và TOTAL3)
    pub fn get_market_indices(&self) -> MarketIndices {
        MarketIndices {
            btc_d_trend: if self.btc_dominance > 50.0 { "UP".to_string() } else { "DOWN".to_string() },
            total3_btc_trend: self.total3_trend.clone(),
            market_breadth_pct_above_ema50: 0.0,
            market_breadth_pct_above_ema200: 0.0,
        }
    }
}
