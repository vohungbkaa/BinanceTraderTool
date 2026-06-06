use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use crate::core::indicators::IndicatorEngine;
use crate::engine::regime::{MarketRegimeEngine, ActionMode};
use crate::engine::scanner::ScannerEngine;
use crate::core::models::{NormalizedCandleData, MarketIndices, Microstructure, MacroEvents, CandleMetadata, AltcoinSnapshot};
use tracing::{info, warn};

pub struct SystemSimulator {
    rest_client: BinanceRestClient,
    indicator_engine: IndicatorEngine,
    regime_engine: MarketRegimeEngine,
    scanner_engine: ScannerEngine,
}

impl SystemSimulator {
    pub fn new() -> Self {
        let rest_client = BinanceRestClient::new();
        let indicator_engine = std::sync::Arc::new(tokio::sync::Mutex::new(IndicatorEngine::new()));
        Self {
            rest_client: rest_client.clone(),
            indicator_engine: IndicatorEngine::new(),
            regime_engine: MarketRegimeEngine::new(),
            scanner_engine: ScannerEngine::new(rest_client, indicator_engine),
        }
    }

    /// Chạy Backtest trên dữ liệu lịch sử để kiểm chứng xem hệ thống có bao giờ cho "Đèn xanh" không.
    /// Giả lập:
    /// - BTCUSDT trong 3 tháng qua (1000 nến 4H).
    /// - Bối cảnh dòng tiền (Flow) được giả lập là TỐT để cô lập và test logic Price Action.
    pub async fn run_historical_validation(&mut self, symbol: &str, limit_4h: u32) -> Result<()> {
        println!("\n========================================================");
        println!("🚀 BẮT ĐẦU SIMULATION CHI TIẾT: PHASE 0 -> PHASE 1 -> PHASE 2");
        println!("========================================================");
        
        // 1. Danh sách Altcoins thực tế để test (Tránh fetch 100 con để né Rate Limit)
        let test_alts = vec![
            "ETHUSDT", "SOLUSDT", "BNBUSDT", "ADAUSDT", "DOGEUSDT", 
            "XRPUSDT", "DOTUSDT", "MATICUSDT", "LINKUSDT", "AVAXUSDT"
        ];

        // 2. Fetch dữ liệu BTC (Benchmark)
        let limit_1d = (limit_4h / 6) + 200;
        let mut btc_1d = self.rest_client.fetch_klines(symbol, "1d", limit_1d).await?;
        btc_1d.reverse();
        let mut btc_4h = self.rest_client.fetch_klines(symbol, "4h", limit_4h).await?;
        btc_4h.reverse();

        // 3. Tải và xử lý dữ liệu cho từng Altcoin
        let config = crate::core::config::AppConfig::load();
        let alt_tf = config.altcoin_analysis_timeframe;
        let mut alt_data_map = std::collections::HashMap::new();
        for alt_sym in &test_alts {
            info!("Fetching and processing real history for {}...", alt_sym);
            let mut a_1d = self.rest_client.fetch_klines(alt_sym, &alt_tf, limit_1d).await?;
            a_1d.reverse();
            let mut a_4h = self.rest_client.fetch_klines(alt_sym, "4h", limit_4h).await?;
            a_4h.reverse();

            // Tính toán Indicators lịch sử cho Altcoin (để có EMA50, EMA200 chuẩn)
            let mut processed_1d = Vec::new();
            let mut local_engine = IndicatorEngine::new();
            for c in a_1d {
                let inds = local_engine.process(&c);
                processed_1d.push((c, inds));
            }

            let mut processed_4h = Vec::new();
            let mut local_engine_4h = IndicatorEngine::new();
            for c in a_4h {
                let inds = local_engine_4h.process(&c);
                processed_4h.push((c, inds));
            }
            alt_data_map.insert(alt_sym.to_string(), (processed_1d, processed_4h));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // 4. Chuẩn bị dữ liệu BTC đã xử lý
        let mut btc_processed_1d = Vec::new();
        let mut btc_engine_1d = IndicatorEngine::new();
        for c in btc_1d {
            let inds = btc_engine_1d.process(&c);
            btc_processed_1d.push((c, inds));
        }

        let mut trigger_count = 0;
        let mut total_candidates_found = 0;

        // 5. Vòng lặp mô phỏng thời gian thực (4H steps)
        for (i, candle_4h) in btc_4h.iter().enumerate() {
            let inds_4h = self.indicator_engine.process(candle_4h);
            
            // Tìm BTC 1D matching
            let matching_btc_1d = btc_processed_1d.iter()
                .filter(|(c, _)| c.close_time <= candle_4h.close_time)
                .last();

            if let Some((btc_c_1d, btc_i_1d)) = matching_btc_1d {
                let data_btc_1d = NormalizedCandleData {
                    candle: btc_c_1d.clone(),
                    indicators: btc_i_1d.clone(),
                    ..Default::default()
                };
                let data_btc_4h = NormalizedCandleData {
                    candle: candle_4h.clone(),
                    indicators: inds_4h,
                    market_indices: MarketIndices {
                        // Giả lập flow theo trend BTC để test logic RS
                        btc_d_trend: if candle_4h.close < btc_i_1d.ema200.unwrap_or(0.0) { crate::core::models::TrendDirection::Up } else { crate::core::models::TrendDirection::Down },
                        total3_btc_trend: if candle_4h.close < btc_i_1d.ema200.unwrap_or(0.0) { crate::core::models::TrendDirection::Down } else { crate::core::models::TrendDirection::Up },
                        market_breadth_pct_above_ema50: 50.0,
                        ..Default::default()
                    },
                    range_p40_90d: 5.0,
                    ..Default::default()
                };

                // PHASE 1: Đánh giá bối cảnh
                let context = self.regime_engine.evaluate_historical(&data_btc_1d, &data_btc_4h).await;

                if context.allow_alt_scan {
                    trigger_count += 1;
                    
                    // PHASE 2: Quét Altcoin dựa trên dữ liệu THẬT đã tải
                    let mut current_alt_snapshots = Vec::new();
                    let btc_change_1d = (btc_c_1d.close - btc_c_1d.open) / btc_c_1d.open * 100.0;
                    let btc_change_4h = (candle_4h.close - candle_4h.open) / candle_4h.open * 100.0;

                    for alt_sym in &test_alts {
                        if let Some((a_hist_1d, a_hist_4h)) = alt_data_map.get(*alt_sym) {
                            let a_4h = a_hist_4h.get(i); // Lấy nến cùng index i với BTC 4H
                            let a_1d = a_hist_1d.iter().filter(|(c, _)| c.close_time <= candle_4h.close_time).last();

                            if let (Some((c4, i4)), Some((c1, _i1))) = (a_4h, a_1d) {
                                current_alt_snapshots.push(AltcoinSnapshot {
                                    symbol: alt_sym.to_string(),
                                    price: c4.close,
                                    ema50_15m: 0.0,
                                    ema200_15m: 0.0,
                                    ema50_4h: i4.ema50.unwrap_or(0.0),
                                    ema200_4h: i4.ema200.unwrap_or(0.0),
                                    ema200_1d: _i1.ema200.unwrap_or(0.0),
                                    change_15m_pct: 0.0,
                                    change_1d_pct: (c1.close - c1.open) / c1.open * 100.0,
                                    change_4h_pct: (c4.close - c4.open) / c4.open * 100.0,
                                    vol_growth_4h_zscore: 1.0, // Mocked for now
                                    oi_growth_4h_pct: 5.0,     // Mocked for now
                                    distance_to_ema50_4h_pct: (c4.close - i4.ema50.unwrap_or(0.0)) / i4.ema50.unwrap_or(1.0) * 100.0,
                                });
                            }
                        }
                    }

                    let shortlist = self.scanner_engine.scan(&context, btc_change_1d, btc_change_4h, &current_alt_snapshots);
                    
                    if !shortlist.is_empty() {
                        total_candidates_found += shortlist.len();
                        let date_str = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(candle_4h.close_time / 1000)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                        
                        println!("--------------------------------------------------");
                        println!("🟢 [SIGNAL] {} | Mode: {} | Found: {} alts", date_str, context.action_mode, shortlist.len());
                        for alt in shortlist {
                            println!("   -> 🎯 {} | Rating: {} | RS: {:.2} | Rank: {:.1}", alt.symbol, alt.rs_rating, alt.rs_score, alt.rank_score);
                        }
                    }
                }
            }
        }

        println!("\n========================================================");
        println!("🏁 TỔNG KẾT BACKTEST THỰC TẾ (10 Altcoins mẫu)");
        println!(" - Tổng số nến 4H: {}", limit_4h);
        println!(" - Số lần Phase 1 mở Gateway: {}", trigger_count);
        println!(" - Tổng số Altcoin lọt vào Shortlist: {}", total_candidates_found);
        println!(" - Hiệu suất lọc: Trung bình {:.1} coin mỗi khi Gateway mở.", total_candidates_found as f64 / trigger_count as f64);
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
