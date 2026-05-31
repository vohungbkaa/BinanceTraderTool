use serde::{Deserialize, Serialize};
use crate::core::models::NormalizedCandleData;
use crate::core::events::MarketEvent;
use tokio::sync::broadcast;
use tracing::{info, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegimeContext {
    pub structural_trend: String,
    pub operational_state: String,
    pub risk_status: String,
    pub market_score: i32,
    pub allow_alt_scan: bool,
    pub action_mode: String,
}

pub struct MarketRegimeEngine {
    // Thêm các tham số cấu hình nếu cần
}

impl MarketRegimeEngine {
    pub fn new() -> Self {
        Self {}
    }

    /// Khởi chạy Engine Phase 1 như một task độc lập
    pub async fn run(
        &self, 
        mut event_rx: broadcast::Receiver<MarketEvent>, 
        event_tx: broadcast::Sender<MarketEvent>
    ) {
        info!("Phase 1: Market Regime Engine is running and listening for events...");

        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if let MarketEvent::CandleClosed(data) = event {
                        // Chỉ phân tích bối cảnh khi có nến BTC đóng (giả định BTC định hình thị trường)
                        if data.candle.symbol == "BTCUSDT" {
                            let context = self.analyze(&data).await;
                            
                            // Phát tán sự kiện kết quả bối cảnh lên bus để Phase 2 nhận
                            if let Err(e) = event_tx.send(MarketEvent::RegimeUpdated(context)) {
                                error!("Failed to broadcast RegimeUpdated: {}", e);
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    error!("MarketRegimeEngine lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("MarketRegimeEngine event channel closed. Exiting...");
                    break;
                }
            }
        }
    }

    async fn analyze(&self, data: &NormalizedCandleData) -> MarketRegimeContext {
        info!("Phase 1: Analyzing Market Regime for {}...", data.candle.symbol);

        // Logic (giữ nguyên hoặc cập nhật theo Spec)
        let structural_trend = if let Some(ema200) = data.indicators.ema200 {
            if data.candle.close > ema200 { "Macro_Bullish".to_string() } else { "Macro_Bearish".to_string() }
        } else {
            "Macro_Neutral".to_string()
        };

        let context = MarketRegimeContext {
            structural_trend,
            operational_state: "Active_Bullish".to_string(),
            risk_status: "Normal".to_string(),
            market_score: 85,
            allow_alt_scan: true,
            action_mode: "Aggressive_Long".to_string(),
        };

        info!("Phase 1: Analysis Complete. Bias: {}", context.action_mode);
        context
    }
}
