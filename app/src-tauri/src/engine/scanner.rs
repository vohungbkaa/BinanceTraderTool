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

    /// [TỐI ƯU CỰC ĐẠI] Lấy dữ liệu CHỈ TỪ DB (Closed Candles) kết hợp giá Live Ticker
    pub async fn fetch_real_snapshots(&self, symbols: &[String], tickers_24h: &[serde_json::Value], db: std::sync::Arc<crate::core::db::Database>) -> Vec<AltcoinSnapshot> {
        info!("ScannerEngine: Calculating snapshots from DB & Live Tickers...");
        let mut snapshots = Vec::new();

        // Tạo HashMap để tra cứu nhanh ticker 24h
        let mut ticker_map = std::collections::HashMap::new();
        for t in tickers_24h {
            if let (Some(sym), Some(price), Some(change)) = (t["symbol"].as_str(), t["lastPrice"].as_str(), t["priceChangePercent"].as_str()) {
                if let (Ok(p), Ok(pct)) = (price.parse::<f64>(), change.parse::<f64>()) {
                    ticker_map.insert(sym.to_string(), (p, pct));
                }
            }
        }

        // Tối ưu DB queries bằng cách spawn đồng thời
        let mut tasks = Vec::new();
        let config = crate::core::config::AppConfig::load();
        let alt_tf = config.altcoin_analysis_timeframe;

        for symbol in symbols {
            let symbol = symbol.clone();
            let db = db.clone();
            let live_data = ticker_map.get(&symbol).cloned();
            let indicator_engine = self.indicator_engine.clone();
            let alt_tf = alt_tf.clone();

            tasks.push(tokio::spawn(async move {
                if let Some((live_price, change_1d_pct)) = live_data {
                    // Lấy nến đóng gần nhất từ DB cho 15m, 4H, 1D
                    let candles_15m = db.get_candles(&symbol, "15m", 200).await.unwrap_or_default();
                    let candles_4h = db.get_candles(&symbol, "4h", 200).await.unwrap_or_default();
                    let candles_1d = db.get_candles(&symbol, &alt_tf, 200).await.unwrap_or_default();

                    let mut inds_engine = indicator_engine.lock().await;

                    // Tính Indicators 15m
                    let mut ema50_15m = 0.0;
                    let mut ema200_15m = 0.0;
                    let mut change_15m_pct = 0.0;
                    if candles_15m.len() >= 200 {
                        let mut final_inds = crate::core::models::Indicators::default();
                        for c in &candles_15m { final_inds = inds_engine.process(c); }
                        ema50_15m = final_inds.ema50.unwrap_or(0.0);
                        ema200_15m = final_inds.ema200.unwrap_or(0.0);
                        
                        // Change 15m tính từ nến đóng trước đó đến giá Live
                        let prev_close = candles_15m.last().unwrap().close;
                        change_15m_pct = (live_price - prev_close) / prev_close * 100.0;
                    }

                    // Tính Indicators 4H và Volume Z-Score
                    let mut ema50_4h = 0.0;
                    let mut ema200_4h = 0.0;
                    let mut change_4h_pct = 0.0;
                    let mut vol_growth_4h_zscore = 0.0;
                    if candles_4h.len() >= 200 {
                        let mut final_inds = crate::core::models::Indicators::default();
                        for c in &candles_4h { final_inds = inds_engine.process(c); }
                        ema50_4h = final_inds.ema50.unwrap_or(0.0);
                        ema200_4h = final_inds.ema200.unwrap_or(0.0);

                        // Change 4H span 2 nến + live price
                        if candles_4h.len() >= 2 {
                            let ref_open = candles_4h[candles_4h.len() - 2].open;
                            change_4h_pct = (live_price - ref_open) / ref_open * 100.0;
                        }

                        // Z-Score Volume (Dựa trên 20 nến 4H quá khứ + current)
                        let vols: Vec<f64> = candles_4h.iter().rev().take(20).map(|c| c.volume).collect();
                        let mean_vol = vols.iter().sum::<f64>() / vols.len().max(1) as f64;
                        let variance = vols.iter().map(|v| (v - mean_vol).powi(2)).sum::<f64>() / (vols.len() - 1).max(1) as f64;
                        let std_dev = variance.sqrt();
                        if std_dev > 0.0 {
                            let last_vol = candles_4h.last().unwrap().volume; // Cần thay bằng live volume nếu có
                            vol_growth_4h_zscore = (last_vol - mean_vol) / std_dev;
                        }
                    }

                    // Tính Indicators 1D
                    let mut ema200_1d = 0.0;
                    if candles_1d.len() >= 200 {
                        let mut final_inds = crate::core::models::Indicators::default();
                        for c in &candles_1d { final_inds = inds_engine.process(c); }
                        ema200_1d = final_inds.ema200.unwrap_or(0.0);
                    }

                    let distance_to_ema50_4h_pct = if ema50_4h > 0.0 {
                        (live_price - ema50_4h) / ema50_4h * 100.0
                    } else { 0.0 };

                    return Some(AltcoinSnapshot {
                        symbol,
                        price: live_price,
                        ema50_15m,
                        ema200_15m,
                        ema50_4h,
                        ema200_4h,
                        ema200_1d,
                        change_15m_pct,
                        change_1d_pct,
                        change_4h_pct,
                        vol_growth_4h_zscore,
                        oi_growth_4h_pct: 0.0, // TODO: OI Pipeline
                        distance_to_ema50_4h_pct,
                    });
                }
                None
            }));
        }

        for task in tasks {
            if let Ok(Some(snap)) = task.await {
                snapshots.push(snap);
            }
        }

        snapshots
    }

    /// [SPEC 3] Tính toán Z-Score Relative Strength
    fn calculate_zscore(values: &[f64]) -> Vec<f64> {
        if values.is_empty() { return vec![]; }
        if values.len() == 1 { return vec![0.0]; }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1).max(1) as f64;
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
        let diffs_15m: Vec<f64> = altcoins.iter().map(|a| a.change_15m_pct - 0.0).collect(); // Giả định BTC 15m diff tạm thời
        let diffs_1d: Vec<f64> = altcoins.iter().map(|a| a.change_1d_pct - btc_change_1d).collect();
        let diffs_4h: Vec<f64> = altcoins.iter().map(|a| a.change_4h_pct - btc_change_4h).collect();

        // 2. Chuẩn hóa Z-Score
        let zscores_15m = Self::calculate_zscore(&diffs_15m);
        let zscores_1d = Self::calculate_zscore(&diffs_1d);
        let zscores_4h = Self::calculate_zscore(&diffs_4h);

        let mut candidates = Vec::new();

        // 3. Đánh giá từng Altcoin
        for (i, alt) in altcoins.iter().enumerate() {
            let is_flat_market = diffs_4h[i].abs() < 1.0 && diffs_1d[i].abs() < 2.0;
            
            let mut final_rs = if is_flat_market {
                ((zscores_4h[i] * 0.7) + (zscores_1d[i] * 0.3)) * 0.5
            } else {
                (zscores_4h[i] * 0.7) + (zscores_1d[i] * 0.3)
            };

            // [SPEC 4] Chỉnh trọng số RS nếu là Scalp Mode
            if context.action_mode == ActionMode::ScalpLong || context.action_mode == ActionMode::ScalpShort {
                final_rs = (zscores_15m[i] * 0.6) + (zscores_4h[i] * 0.4);
            }
            
            let rating = Self::get_rating(final_rs);
            let rank_score = (final_rs * 0.4) + (alt.vol_growth_4h_zscore * 0.3) + (alt.oi_growth_4h_pct * 0.3);

            let mut is_valid = false;
            let mut direction = "";
            let mut reason = "";

            match context.action_mode {
                ActionMode::AggressiveLong => {
                    let price_above_ema = alt.price > alt.ema200_1d && alt.ema50_4h > alt.ema200_4h;
                    let good_rs = rating == RsRating::A || rating == RsRating::B;
                    
                    if good_rs && price_above_ema {
                        is_valid = true;
                        direction = "LONG";
                        reason = "RS Leader, Trend Bullish";
                    }
                },
                ActionMode::ScalpLong => {
                    let good_rs = final_rs > 1.5; // Tập trung RS khung ngắn hạn
                    if good_rs && alt.ema50_15m > alt.ema200_15m {
                        is_valid = true;
                        direction = "LONG";
                        reason = "Strong short-term RS for Scalping (15m)";
                    }
                },
                ActionMode::AggressiveShort => {
                    let price_below_ema = alt.price < alt.ema200_1d && alt.ema50_4h < alt.ema200_4h;
                    let weak_rs = rating == RsRating::D;
                    let pump_protection = alt.change_1d_pct > 5.0 || diffs_1d[i] > 5.0;

                    if weak_rs && price_below_ema && !pump_protection {
                        is_valid = true;
                        direction = "SHORT";
                        reason = "RS Laggard (D), Trend Bearish";
                    }
                },
                ActionMode::ScalpShort => {
                    let weak_rs = final_rs < -1.5;
                    let pump_protection = alt.change_1d_pct > 5.0 || diffs_1d[i] > 5.0;
                    if weak_rs && alt.ema50_15m < alt.ema200_15m && !pump_protection {
                        is_valid = true;
                        direction = "SHORT";
                        reason = "Weak short-term RS for Scalping (15m)";
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
                        vol_growth_4h_pct: alt.vol_growth_4h_zscore,
                        oi_growth_4h_pct: alt.oi_growth_4h_pct,
                        distance_to_ema50_4h_pct: alt.distance_to_ema50_4h_pct,
                    },
                    reason: reason.to_string(),
                });
            }
        }

        candidates.sort_by(|a, b| b.rank_score.partial_cmp(&a.rank_score).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(5);

        candidates
    }
}
