use crate::core::models::{MacroEvents, Microstructure, MarketIndices, TrendDirection};
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
    pub symbol_cvd_4h: HashMap<String, f64>,
    pub symbol_cvd_1d: HashMap<String, f64>,
    pub symbol_liq_upper: HashMap<String, f64>,
    pub symbol_liq_lower: HashMap<String, f64>,
    
    // [SPEC 2.3] TOTAL3 / BTC Approximation trend
    pub total3_trend: TrendDirection,
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
            symbol_cvd_4h: HashMap::new(),
            symbol_cvd_1d: HashMap::new(),
            symbol_liq_upper: HashMap::new(),
            symbol_liq_lower: HashMap::new(),
            total3_trend: TrendDirection::Sideway,
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

    /// [SPEC 2.3] Cập nhật OI hiện tại
    pub fn update_oi(&mut self, symbol: String, current_oi: f64) {
        self.symbol_oi.insert(symbol, current_oi);
    }

    /// [SPEC 2.3] Snapshot OI 4H trước đó khi nến 4H đóng
    pub fn snapshot_4h_oi(&mut self, symbol: &str) {
        if let Some(&current_oi) = self.symbol_oi.get(symbol) {
            self.symbol_oi_prev.insert(symbol.to_string(), current_oi);
        }
    }

    /// [SPEC 2.2 & 2.3] Trả về dữ liệu vị thế
    pub fn get_microstructure_risk(&self, symbol: &str, current_price: f64, atr: f64) -> Microstructure {
        let oi_change = if let (Some(curr), Some(prev)) = (self.symbol_oi.get(symbol), self.symbol_oi_prev.get(symbol)) {
            if *prev > 0.0 { (curr - prev) / prev * 100.0 } else { 0.0 }
        } else { 0.0 };

        let funding = *self.symbol_funding.get(symbol).unwrap_or(&0.0);
        let upper_real = *self.symbol_liq_upper.get(symbol).unwrap_or(&0.0);
        let lower_real = *self.symbol_liq_lower.get(symbol).unwrap_or(&0.0);

        // Thuật toán dự phóng Heatmap (Estimated Clusters) dựa trên ATR và Funding Rate
        // 1. Dùng ATR để xác định biên độ (khoảng cách thanh lý trung bình)
        let base_dist = if atr > 0.0 { atr * 2.0 } else { current_price * 0.02 };
        
        // 2. Tinh chỉnh (Skew) dựa trên Funding
        // Nếu funding dương mạnh (Crowded Longs) -> Lower gần hơn, Upper xa hơn.
        // Giả sử mốc chuẩn là 0.01%. Chênh lệch 0.05% sẽ dịch chuyển 25% (0.25).
        let skew = ((funding - 0.01) * 5.0).clamp(-0.4, 0.4);

        let upper_est = current_price + base_dist * (1.0 + skew);
        let lower_est = current_price - base_dist * (1.0 - skew);

        Microstructure {
            oi_change_4h_pct: oi_change,
            price_change_4h_pct: 0.0,
            funding_rate_avg: funding,
            cvd_4h: *self.symbol_cvd_4h.get(symbol).unwrap_or(&0.0),
            cvd_1d: *self.symbol_cvd_1d.get(symbol).unwrap_or(&0.0),
            liquidation_surge_detected: self.recent_liquidations_usd > 10_000_000.0,
            liquidation_upper_real: upper_real,
            liquidation_lower_real: lower_real,
            liquidation_upper_est: upper_est,
            liquidation_lower_est: lower_est,
            spread_anomaly: false,
        }
    }

    /// [SPEC 2.3] Lấy chỉ số thị trường (BTC.D và TOTAL3)
    pub fn get_market_indices(&self) -> MarketIndices {
        MarketIndices {
            btc_d_trend: if self.btc_dominance > 50.0 { TrendDirection::Up } else { TrendDirection::Down },
            total3_btc_trend: self.total3_trend.clone(),
            market_breadth_pct_above_ema50: 0.0,
            market_breadth_pct_above_ema200: 0.0,
        }
    }
}
