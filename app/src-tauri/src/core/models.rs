use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Candle {
    pub symbol: String,
    pub timeframe: String,
    pub open_time: i64,
    pub close_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Indicators {
    pub ema20: Option<f64>,
    pub ema50: Option<f64>,
    pub ema200: Option<f64>,
    pub atr14: Option<f64>,
    pub adx14: Option<f64>,
    pub plus_di: Option<f64>,
    pub minus_di: Option<f64>,
    pub structure: String,
    pub close_above_ema200_count: u32,
    pub ema50_slope: f64,
}

impl Default for Indicators {
    fn default() -> Self {
        Self {
            ema20: None, ema50: None, ema200: None,
            atr14: None, adx14: None, plus_di: None, minus_di: None,
            structure: "None".to_string(),
            close_above_ema200_count: 0,
            ema50_slope: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Microstructure {
    pub oi_change_4h_pct: f64,
    pub price_change_4h_pct: f64,
    pub funding_rate_avg: f64,
    pub liquidation_surge_detected: bool,
    pub spread_anomaly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MacroEvents {
    pub next_event_name: String,
    pub time_to_event_minutes: i64,
    pub is_event_block_window: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Up,
    Down,
    Sideway,
}

impl Default for TrendDirection {
    fn default() -> Self {
        Self::Sideway
    }
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Sideway => "SIDEWAY",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MarketIndices {
    pub btc_d_trend: TrendDirection,
    pub total3_btc_trend: TrendDirection,
    pub market_breadth_pct_above_ema50: f64,
    pub market_breadth_pct_above_ema200: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CandleMetadata {
    pub is_warmup: bool,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NormalizedCandleData {
    pub timestamp: i64,
    pub candle: Candle,
    pub indicators: Indicators,
    pub market_indices: MarketIndices,
    pub microstructure: Microstructure,
    pub macro_events: MacroEvents,
    pub metadata: CandleMetadata,
    pub range_24h_pct: f64,
    pub range_p40_90d: f64,
    pub atr_surge_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolConfig {
    pub symbol: String,
    pub status: String,
    pub listed_at: i64,
    pub volume_24h: f64,
}
