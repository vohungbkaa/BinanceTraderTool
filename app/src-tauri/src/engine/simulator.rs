use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use crate::core::indicators::IndicatorEngine;
use crate::engine::regime::{MarketRegimeEngine, ActionMode};
use crate::core::models::{NormalizedCandleData, MarketIndices, Microstructure, MacroEvents, CandleMetadata};
use tracing::{info, warn};

pub struct SystemSimulator {
    rest_client: BinanceRestClient,
    indicator_engine: IndicatorEngine,
    regime_engine: MarketRegimeEngine,
}

impl SystemSimulator {
    pub fn new() -> Self {
        Self {
            rest_client: BinanceRestClient::new(),
            indicator_engine: IndicatorEngine::new(),
            regime_engine: MarketRegimeEngine::new(),
        }
    }

    /// Chạy Backtest trên dữ liệu lịch sử để kiểm chứng xem hệ thống có bao giờ cho "Đèn xanh" không.
    /// Giả lập:
    /// - BTCUSDT trong 3 tháng qua (1000 nến 4H).
    /// - Bối cảnh dòng tiền (Flow) được giả lập là TỐT để cô lập và test logic Price Action.
    pub async fn run_historical_validation(&mut self, symbol: &str, limit_4h: u32) -> Result<()> {
        println!("\n========================================================");
        println!("🚀 BẮT ĐẦU SIMULATION & BACKTEST: PHASE 0 -> PHASE 1");
        println!("========================================================");
        println!("Symbol: {}", symbol);
        println!("Số lượng nến 4H kiểm tra: {}", limit_4h);

        // 1. Fetch dữ liệu 1D (Cần nhiều nến hơn để tính EMA200 chính xác)
        let limit_1d = (limit_4h / 6) + 200; // Đảm bảo đủ 200 nến 1D để Warm-up EMA200
        info!("Fetching {} 1D candles for warm-up...", limit_1d);
        let mut candles_1d = self.rest_client.fetch_klines(symbol, "1d", limit_1d).await?;
        candles_1d.reverse(); // Đảo ngược để nến cũ nhất nằm đầu mảng (chronological)

        // 2. Fetch dữ liệu 4H
        info!("Fetching {} 4H candles for evaluation...", limit_4h);
        let mut candles_4h = self.rest_client.fetch_klines(symbol, "4h", limit_4h).await?;
        candles_4h.reverse();

        // 3. Process Indicators cho toàn bộ mảng 1D để build lịch sử
        let mut processed_1d = Vec::new();
        for candle in candles_1d {
            let inds = self.indicator_engine.process(&candle);
            // Giả lập dữ liệu Flow & Risk mặc định TỐT
            let data = NormalizedCandleData {
                timestamp: candle.close_time,
                candle: candle.clone(),
                indicators: inds,
                market_indices: MarketIndices {
                    btc_d_trend: crate::core::models::TrendDirection::Down, // Giả lập tiền chảy vào Altcoin
                    total3_btc_trend: crate::core::models::TrendDirection::Up,
                    market_breadth_pct_above_ema50: 60.0,
                    market_breadth_pct_above_ema200: 50.0,
                },
                microstructure: Microstructure {
                    oi_change_4h_pct: 2.0, // Giả lập OI tăng
                    price_change_4h_pct: 1.0,
                    funding_rate_avg: 0.01,
                    liquidation_surge_detected: false,
                    spread_anomaly: false,
                },
                macro_events: MacroEvents {
                    is_event_block_window: false,
                    ..Default::default()
                },
                range_24h_pct: 0.0, // Chưa tính trong mock này
                range_p40_90d: 5.0, // Giả lập P40 khá cao để không bị dính Sideway dễ dàng
                atr_surge_ratio: 1.0, // Không có bão biến động
                metadata: CandleMetadata::default(),
            };
            processed_1d.push(data);
        }

        let mut trigger_count = 0;
        let mut macro_bullish_count = 0;
        let mut macro_bearish_count = 0;
        let mut active_bullish_count = 0;
        let mut active_bearish_count = 0;
        let mut score_above_40_count = 0;

        // 4. Duyệt qua từng nến 4H theo trình tự thời gian (như chạy Live)
        for candle_4h in candles_4h {
            let inds_4h = self.indicator_engine.process(&candle_4h);
            
            // 5. Tìm nến 1D tương ứng (Nến 1D cuối cùng đã đóng tính đến thời điểm đóng của nến 4H này)
            let matching_1d = processed_1d.iter()
                .filter(|d| d.candle.close_time <= candle_4h.close_time)
                .last();

            if let Some(data_1d) = matching_1d {
                // Tự động giả lập Flow đồng thuận với Trend 1D để test logic Price Action
                let is_macro_bear = if let Some(ema200) = data_1d.indicators.ema200 { data_1d.candle.close < ema200 } else { false };
                
                let btc_d_mock = if is_macro_bear { crate::core::models::TrendDirection::Up } else { crate::core::models::TrendDirection::Down };
                let total3_mock = if is_macro_bear { crate::core::models::TrendDirection::Down } else { crate::core::models::TrendDirection::Up };

                let data_4h = NormalizedCandleData {
                    timestamp: candle_4h.close_time,
                    candle: candle_4h.clone(),
                    indicators: inds_4h,
                    market_indices: MarketIndices {
                        btc_d_trend: btc_d_mock,
                        total3_btc_trend: total3_mock,
                        market_breadth_pct_above_ema50: if is_macro_bear { 30.0 } else { 60.0 },
                        market_breadth_pct_above_ema200: if is_macro_bear { 20.0 } else { 50.0 },
                    },
                    microstructure: Microstructure {
                        oi_change_4h_pct: 2.0,
                        price_change_4h_pct: 1.0,
                        funding_rate_avg: 0.01,
                        liquidation_surge_detected: false,
                        spread_anomaly: false,
                    },
                    macro_events: MacroEvents {
                        is_event_block_window: false,
                        ..Default::default()
                    },
                    range_24h_pct: (candle_4h.high - candle_4h.low) / candle_4h.open * 100.0,
                    range_p40_90d: 5.0,
                    atr_surge_ratio: 1.0,
                    metadata: CandleMetadata::default(),
                };

                // Đưa vào Regime Engine để chấm điểm (Phase 1)
                let context = self.regime_engine.evaluate_historical(data_1d, &data_4h).await;

                use crate::engine::regime::{StructuralTrend, OperationalState};
                if context.structural_trend == StructuralTrend::MacroBullish { macro_bullish_count += 1; }
                if context.structural_trend == StructuralTrend::MacroBearish { macro_bearish_count += 1; }
                if context.operational_state == OperationalState::ActiveBullish { active_bullish_count += 1; }
                if context.operational_state == OperationalState::ActiveBearish { active_bearish_count += 1; }
                if context.market_score >= 40 { score_above_40_count += 1; }

                // 6. GHI NHẬN KẾT QUẢ NẾU GATEWAY MỞ
                if context.allow_alt_scan {
                    trigger_count += 1;
                    
                    let date_str = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(candle_4h.close_time / 1000)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();

                    println!("--------------------------------------------------");
                    println!("🟢 [GATEWAY OPEN] Tại thời điểm: {}", date_str);
                    println!(" - Giá BTC: ${:.2}", candle_4h.close);
                    println!(" - Điểm Market Score: {}/100", context.market_score);
                    println!(" - Action Mode: {}", context.action_mode);
                    println!(" - Macro Trend (1D): {}", context.structural_trend);
                    println!(" - Micro State (4H): {}", context.operational_state);
                    println!(" - ADX 4H: {:.2}", data_4h.indicators.adx14.unwrap_or(0.0));
                }
            }
        }

        println!("========================================================");
        println!("🏁 TỔNG KẾT SIMULATION");
        println!(" - Đã kiểm tra {} nến 4H lịch sử.", limit_4h);
        println!(" - Macro Bullish: {} lần", macro_bullish_count);
        println!(" - Macro Bearish: {} lần", macro_bearish_count);
        println!(" - Active Bullish (4H): {} lần", active_bullish_count);
        println!(" - Active Bearish (4H): {} lần", active_bearish_count);
        println!(" - Lần đạt điểm > 40: {} lần", score_above_40_count);
        println!(" - Số lần hệ thống cấp đèn xanh (allow_alt_scan = true): {}", trigger_count);
        
        if trigger_count == 0 {
            println!(" ⚠️ Hệ thống KHÔNG tìm thấy cơ hội nào. Điều kiện quá khắt khe hoặc thị trường vừa qua quá xấu!");
        } else {
            let freq = limit_4h as f64 / trigger_count as f64;
            println!(" - Tần suất: Trung bình mỗi {:.1} nến 4H (khoảng {:.1} ngày) có 1 tín hiệu bật đèn xanh.", freq, freq * 4.0 / 24.0);
        }
        println!("========================================================");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_full_historical_backtest() {
        // Chạy test này bằng lệnh: cargo test --lib engine::simulator::tests::run_full_historical_backtest -- --nocapture
        let mut simulator = SystemSimulator::new();
        // Test trên 1000 nến 4H gần nhất (Tương đương khoảng 166 ngày ~ 5.5 tháng qua)
        let result = simulator.run_historical_validation("BTCUSDT", 1000).await;
        assert!(result.is_ok(), "Simulator encountered an error!");
    }
}
