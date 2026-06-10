use serde::{Deserialize, Serialize};
use super::models::NormalizedCandleData;
use crate::engine::regime::MarketRegimeContext;

use crate::engine::scanner::ScannerPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum MarketEvent {
    CandleUpdated(NormalizedCandleData),
    CandleClosed(NormalizedCandleData),
    
    // Sự kiện mới: Bối cảnh thị trường đã được cập nhật (Phase 1 phát ra)
    RegimeUpdated(MarketRegimeContext),

    // Sự kiện mới: Danh sách quét Altcoin đã được cập nhật (Phase 2 phát ra)
    ScannerUpdated(ScannerPayload),

    DepthUpdated {
        symbol: String,
        is_liquidation: bool,
        price: f64,
        value_usd: f64,
        timestamp: i64,
    },
    FundingUpdated {
        symbol: String,
        funding_rate: f64,
        timestamp: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded(String),
    Critical(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum SystemEvent {
    HealthChanged {
        previous: HealthState,
        current: HealthState,
        timestamp: i64,
    },
    DataGapDetected {
        symbol: String,
        timeframe: String,
        from_time: i64,
        to_time: i64,
        timestamp: i64,
    },
}
