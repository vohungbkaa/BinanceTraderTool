use serde::{Deserialize, Serialize};
use crate::engine::regime::{MarketRegimeContext, ActionMode};
use crate::core::models::AltcoinSnapshot;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RsRating {
    A, // > 1.5
    B, // 0.5 to 1.5
    C, // -0.5 to 0.5
    D, // < -0.5
}

impl std::fmt::Display for RsRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMetrics {
    pub vol_growth_4h_pct: f64,
    pub oi_growth_4h_pct: f64,
    pub distance_to_ema50_4h_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanCandidate {
    pub symbol: String,
    pub rs_score: f64,
    pub rs_rating: RsRating,
    pub direction: String,
    pub rank_score: f64,
    pub metrics: ScanMetrics,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerPayload {
    pub scan_timestamp: i64,
    pub shortlist: Vec<ScanCandidate>,
}

pub struct ScannerEngine {
    rest_client: crate::core::rest::BinanceRestClient,
    indicator_engine: std::sync::Arc<tokio::sync::Mutex<crate::core::indicators::IndicatorEngine>>,
}

impl ScannerEngine {
    pub fn new(
        rest_client: crate::core::rest::BinanceRestClient,
        indicator_engine: std::sync::Arc<tokio::sync::Mutex<crate::core::indicators::IndicatorEngine>>,
    ) -> Self {
        Self { rest_client, indicator_engine }
    }

    /// [PHỤC VỤ LAZY LOADING] Tải dữ liệu thật cho danh sách Altcoin và chuyển đổi thành Snapshots
    pub async fn fetch_real_snapshots(&self, symbols: &[String]) -> Vec<AltcoinSnapshot> {
        info!("ScannerEngine: Fetching real-time market data for {} altcoins (Batch mode)...", symbols.len());
        let mut snapshots = Vec::new();

        // 1. Lấy dữ liệu BTC (Để làm mốc so sánh)
        let btc_klines_1d = self.rest_client.fetch_klines("BTCUSDT", "1d", 2).await.unwrap_or_default();
        let btc_klines_4h = self.rest_client.fetch_klines("BTCUSDT", "4h", 2).await.unwrap_or_default();
        
        let btc_change_1d = if btc_klines_1d.len() >= 2 { (btc_klines_1d[1].close - btc_klines_1d[0].close) / btc_klines_1d[0].close * 100.0 } else { 0.0 };
        let btc_change_4h = if btc_klines_4h.len() >= 2 { (btc_klines_4h[1].close - btc_klines_4h[0].close) / btc_klines_4h[0].close * 100.0 } else { 0.0 };

        for symbol in symbols {
            // Tải nến 4H (200 nến để tính EMA chuẩn)
            match self.rest_client.fetch_klines(symbol, "4h", 200).await {
                Ok(candles_4h) => {
                    if let Some(last_4h) = candles_4h.last() {
                        // Tính toán EMA cho Altcoin
                        let mut inds_engine = self.indicator_engine.lock().await;
                        let mut inds_4h = crate::core::models::Indicators::default();
                        for c in &candles_4h {
                            inds_4h = inds_engine.process(c);
                        }

                        // Tính % change 4h thực tế
                        let change_4h = if candles_4h.len() >= 2 { 
                            (last_4h.close - candles_4h[candles_4h.len()-2].close) / candles_4h[candles_4h.len()-2].close * 100.0 
                        } else { 0.0 };

                        // Fetch thêm nến 1D (chỉ lấy 200 nến để tính EMA200 1D)
                        match self.rest_client.fetch_klines(symbol, "1d", 200).await {
                            Ok(candles_1d) => {
                                let mut inds_1d = crate::core::models::Indicators::default();
                                for c in &candles_1d {
                                    inds_1d = inds_engine.process(c);
                                }
                                let last_1d = candles_1d.last().unwrap();
                                let change_1d = if candles_1d.len() >= 2 {
                                    (last_1d.close - candles_1d[candles_1d.len()-2].close) / candles_1d[candles_1d.len()-2].close * 100.0
                                } else { 0.0 };

                                snapshots.push(AltcoinSnapshot {
                                    symbol: symbol.clone(),
                                    price: last_4h.close,
                                    ema50_4h: inds_4h.ema50.unwrap_or(0.0),
                                    ema200_4h: inds_4h.ema200.unwrap_or(0.0),
                                    ema200_1d: inds_1d.ema200.unwrap_or(0.0),
                                    change_1d_pct: change_1d, 
                                    change_4h_pct: change_4h,
                                    vol_growth_4h_zscore: 1.0, 
                                    oi_growth_4h_pct: 0.0,
                                    distance_to_ema50_4h_pct: (last_4h.close - inds_4h.ema50.unwrap_or(last_4h.close)) / inds_4h.ema50.unwrap_or(last_4h.close) * 100.0,
                                });
                            },
                            Err(_) => {}
                        }
                    }
                }
                Err(e) => warn!("Scanner: Failed to fetch data for {}: {}", symbol, e),
            }
            // Sleep 250ms giữa mỗi coin để tránh HTTP 429 (Banned)
            // 100 coin * 0.25s = 20s cho mỗi vòng quét (An toàn cho API Weight)
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        snapshots
    }

    /// [SPEC 3] Tính toán Z-Score Relative Strength
    fn calculate_zscore(values: &[f64]) -> Vec<f64> {
        if values.is_empty() { return vec![]; }
        if values.len() == 1 { return vec![0.0]; }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return vec![0.0; values.len()];
        }

        values.iter().map(|v| (v - mean) / std_dev).collect()
    }

    fn get_rating(rs_score: f64) -> RsRating {
        if rs_score > 1.5 { RsRating::A }
        else if rs_score > 0.5 { RsRating::B }
        else if rs_score >= -0.5 { RsRating::C }
        else { RsRating::D }
    }

    pub fn scan(
        &self, 
        context: &MarketRegimeContext, 
        btc_change_1d: f64, 
        btc_change_4h: f64, 
        altcoins: &[AltcoinSnapshot]
    ) -> Vec<ScanCandidate> {
        
        if !context.allow_alt_scan || context.action_mode == ActionMode::OffSystem || altcoins.is_empty() {
            return vec![];
        }

        // 1. Tính toán Base Difference (Alt - BTC)
        let diffs_1d: Vec<f64> = altcoins.iter().map(|a| a.change_1d_pct - btc_change_1d).collect();
        let diffs_4h: Vec<f64> = altcoins.iter().map(|a| a.change_4h_pct - btc_change_4h).collect();

        // 2. Chuẩn hóa Z-Score
        let zscores_1d = Self::calculate_zscore(&diffs_1d);
        let zscores_4h = Self::calculate_zscore(&diffs_4h);

        let mut candidates = Vec::new();

        // 3. Đánh giá từng Altcoin
        for (i, alt) in altcoins.iter().enumerate() {
            // [SPEC 3.A.3] Trọng số Đa khung (4H: 0.7, 1D: 0.3)
            let final_rs = (zscores_4h[i] * 0.7) + (zscores_1d[i] * 0.3);
            let rating = Self::get_rating(final_rs);

            // [SPEC 5] Tính Rank Score
            let rank_score = (final_rs * 0.4) + (alt.vol_growth_4h_zscore * 0.3) + (alt.oi_growth_4h_pct * 0.3);

            let mut is_valid = false;
            let mut direction = "";
            let mut reason = "";

            // [SPEC 4] Bộ lọc theo Bối cảnh (Contextual Rules)
            match context.action_mode {
                ActionMode::AggressiveLong | ActionMode::ScalpLong => {
                    let price_above_ema = alt.price > alt.ema200_1d && alt.ema50_4h > alt.ema200_4h;
                    let good_rs = rating == RsRating::A || rating == RsRating::B;
                    let oi_increasing = alt.oi_growth_4h_pct > 0.0;

                    if context.action_mode == ActionMode::AggressiveLong {
                        if good_rs && price_above_ema && oi_increasing {
                            is_valid = true;
                            direction = "LONG";
                            reason = "RS Leader, Trend Bullish, OI Surge";
                        }
                    } else { // Scalp Long (Nới lỏng EMA 1D)
                        if good_rs && alt.ema50_4h > alt.ema200_4h {
                            is_valid = true;
                            direction = "LONG";
                            reason = "Strong short-term RS for Scalping";
                        }
                    }
                },
                ActionMode::AggressiveShort | ActionMode::ScalpShort => {
                    let price_below_ema = alt.price < alt.ema200_1d && alt.ema50_4h < alt.ema200_4h;
                    let weak_rs = rating == RsRating::D;
                    let oi_increasing = alt.oi_growth_4h_pct > 0.0; // Build-up short

                    if context.action_mode == ActionMode::AggressiveShort {
                        if weak_rs && price_below_ema && oi_increasing {
                            is_valid = true;
                            direction = "SHORT";
                            reason = "RS Laggard (D), Trend Bearish, Short Build-up";
                        }
                    } else { // Scalp Short
                        if weak_rs && alt.ema50_4h < alt.ema200_4h {
                            is_valid = true;
                            direction = "SHORT";
                            reason = "Weak short-term RS for Scalping";
                        }
                    }
                },
                ActionMode::MeanReversion => {
                    if final_rs < -2.5 && alt.distance_to_ema50_4h_pct < -5.0 {
                        is_valid = true;
                        direction = "LONG";
                        reason = "Oversold Extreme (Mean Reversion)";
                    } else if final_rs > 2.5 && alt.distance_to_ema50_4h_pct > 5.0 {
                        is_valid = true;
                        direction = "SHORT";
                        reason = "Overbought Extreme (Mean Reversion)";
                    }
                },
                ActionMode::OffSystem => {}
            }

            if is_valid {
                candidates.push(ScanCandidate {
                    symbol: alt.symbol.clone(),
                    rs_score: final_rs,
                    rs_rating: rating,
                    direction: direction.to_string(),
                    rank_score,
                    metrics: ScanMetrics {
                        vol_growth_4h_pct: alt.vol_growth_4h_zscore, // Map zscore to pct field for simplicity in this struct
                        oi_growth_4h_pct: alt.oi_growth_4h_pct,
                        distance_to_ema50_4h_pct: alt.distance_to_ema50_4h_pct,
                    },
                    reason: reason.to_string(),
                });
            }
        }

        // [SPEC 5] Xếp hạng và trả về Top 5
        candidates.sort_by(|a, b| b.rank_score.partial_cmp(&a.rank_score).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(5);

        candidates
    }
}
