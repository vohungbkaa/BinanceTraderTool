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

    /// [TỐI ƯU CỰC ĐẠI] Tải dữ liệu thật, ưu tiên dùng Cache từ DB để tránh lãng phí API
    pub async fn fetch_real_snapshots(&self, symbols: &[String], db: &crate::core::db::Database) -> Vec<AltcoinSnapshot> {
        info!("ScannerEngine: Syncing market data for {} altcoins (Smart Cache Mode)...", symbols.len());
        let mut snapshots = Vec::new();
        let now_ms = chrono::Utc::now().timestamp_millis();

        for symbol in symbols {
            // 1. Kiểm tra xem trong DB đã có nến của đồng này chưa và nến cuối là khi nào
            let last_update = db.get_last_update_time(symbol, "4h").await.unwrap_or(0);
            let mut candles = db.get_candles(symbol, "4h", 200).await.unwrap_or_default();
            
            // 2. Nếu thiếu dữ liệu hoặc dữ liệu quá cũ (quá 4h), mới đi gọi API
            if candles.len() < 200 || (now_ms - last_update) > 14400_000 {
                let limit_to_fetch = if candles.is_empty() { 200 } else { 5 }; // Chỉ fetch nến mới nếu đã có lịch sử
                match self.rest_client.fetch_klines(symbol, "4h", limit_to_fetch).await {
                    Ok(new_candles) => {
                        for c in new_candles {
                            // Tính indicators và lưu vào DB để dùng cho lần sau
                            let mut inds_engine = self.indicator_engine.lock().await;
                            let inds = inds_engine.process(&c);
                            let normalized = crate::core::models::NormalizedCandleData {
                                timestamp: c.close_time,
                                candle: c,
                                indicators: inds,
                                ..Default::default()
                            };
                            let _ = db.insert_closed_candle(&normalized).await;
                        }
                        // Lấy lại bộ nến đầy đủ từ DB sau khi đã update
                        candles = db.get_candles(symbol, "4h", 200).await.unwrap_or_default();
                    }
                    Err(e) => warn!("Scanner: Fetch failed for {}: {}", symbol, e),
                }
                // Sleep nhẹ để bảo vệ IP khi phải gọi API
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }

            // 3. Tạo snapshot từ dữ liệu đã có trong DB
            if candles.len() >= 200 {
                let last_4h = candles.last().unwrap();
                let mut inds_engine = self.indicator_engine.lock().await;
                let mut final_inds = crate::core::models::Indicators::default();
                for c in &candles {
                    final_inds = inds_engine.process(c);
                }

                snapshots.push(AltcoinSnapshot {
                    symbol: symbol.clone(),
                    price: last_4h.close,
                    ema50_4h: final_inds.ema50.unwrap_or(0.0),
                    ema200_4h: final_inds.ema200.unwrap_or(0.0),
                    ema200_1d: 0.0, // Tương tự có thể tối ưu cho 1D
                    change_1d_pct: 0.0,
                    change_4h_pct: (last_4h.close - candles[0].close) / candles[0].close * 100.0,
                    vol_growth_4h_zscore: 1.0,
                    oi_growth_4h_pct: 0.0,
                    distance_to_ema50_4h_pct: (last_4h.close - final_inds.ema50.unwrap_or(last_4h.close)) / final_inds.ema50.unwrap_or(last_4h.close) * 100.0,
                });
            }
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
