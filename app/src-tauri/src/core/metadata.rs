use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use tracing::{info, warn};
use std::collections::{HashSet, HashMap};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniverseCandidate {
    pub symbol: String,
    pub quote_volume: f64,
    pub volume_change_24h_pct: f64,
    pub open_interest: f64,
    pub oi_change_24h_pct: f64,
    pub volatility: f64,
    pub funding_rate: f64,
    
    pub vol_score: f64,
    pub vol_change_score: f64,
    pub oi_score: f64,
    pub oi_change_score: f64,
    pub atr_score: f64,
    pub fund_score: f64,
    
    pub composite_score: f64,
    
    pub price_change_percent: f64,
    pub last_price: f64,
}

pub struct MetadataManager {
    rest_client: BinanceRestClient,
    // (ATR_Value, Yesterday_Volume, Timestamp_ms)
    atr_cache: Arc<RwLock<HashMap<String, (f64, f64, i64)>>>, 
}

impl MetadataManager {
    pub fn new(rest_client: BinanceRestClient) -> Self {
        Self {
            rest_client,
            atr_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// [SPEC 2.2] Xây dựng danh sách 100 đồng Altcoin tiềm năng nhất để đưa vào quét tín hiệu.
    ///
    /// CHI TIẾT THUẬT TOÁN (BACKEND - ADVANCED QUANT MODEL v1.2):
    /// 1. Lọc sơ bộ: Chỉ lấy các cặp giao dịch Futures (PERPETUAL) đang hoạt động (TRADING). Bỏ qua Stablecoins, BTC, ETH.
    /// 2. Top Volume & Top OI: Lấy Top 300 Volume, sau đó cắt lấy Top 150 OI để loại bỏ wash trading.
    /// 3. Tính toán Điểm Thành Phần (0 - 100):
    ///    - Điểm Volume & OI: Sử dụng Logarit tự nhiên `ln(value + 1)` trước khi Min-Max Normalization.
    ///    - Điểm Volume Growth & OI Change 24h: Tỉ lệ thuận. Tăng trưởng dòng tiền càng mạnh, điểm càng cao.
    ///    - Điểm Biến động (ATR Sweet Spot): Áp dụng phân phối hình chuông. Vùng lý tưởng (3% - 8%) đạt 100 điểm.
    ///    - Điểm Funding Rate (Percentile): Loại bỏ 10% coin có Funding cao nhất và 10% thấp nhất (rủi ro Squeeze). 80% ở giữa nhận 100 điểm.
    /// 4. Tính điểm tổng hợp (Composite Score 0-100) dựa trên trọng số:
    ///    `25% Vol + 10% Vol_Growth + 20% OI + 15% OI_Growth + 20% ATR + 10% Funding`.
    pub async fn get_universe_candidates(&self) -> Result<Vec<UniverseCandidate>> {
        info!("MetadataManager: Building Universe with Advanced Quant Scoring (v1.2)...");
        let limit = crate::core::config::AppConfig::load().altcoin_count;

        // Constants for Scoring Logic
        const INITIAL_UNIVERSE_LIMIT: usize = 300;
        const OI_FILTER_LIMIT: usize = 150;
        
        const ATR_SWEET_MIN: f64 = 0.02;      // 2%
        const ATR_SWEET_MAX: f64 = 0.08;      // 8%
        const ATR_HIGH_THRESH: f64 = 0.15;    // 15%
        
        const ATR_CACHE_TTL_MS: i64 = 4 * 60 * 60 * 1000; // Cache 4 giờ

        // 1. Lọc Metadata từ Exchange Info
        let exchange_info = self.rest_client.fetch_exchange_info().await?;
        let mut valid_symbols = HashSet::new();
        if let Some(symbols) = exchange_info["symbols"].as_array() {
            for s in symbols {
                if s["contractType"].as_str() == Some("PERPETUAL") 
                    && s["status"].as_str() == Some("TRADING") {
                    if let Some(sym) = s["symbol"].as_str() {
                        valid_symbols.insert(sym.to_string());
                    }
                }
            }
        }

        // 2. Fetch 24h Tickers
        let tickers = self.rest_client.fetch_24h_tickers().await?;
        #[derive(Clone)]
        struct RawData { symbol: String, vol: f64, p_change: f64, last_price: f64 }
        let mut raw_candidates: Vec<RawData> = tickers.into_iter().filter_map(|t| {
            let symbol = t["symbol"].as_str()?.to_string();
            if !valid_symbols.contains(&symbol) || !symbol.ends_with("USDT")
                || symbol.starts_with("USDC") || symbol.starts_with("BUSD")
                || symbol == "BTCUSDT" || symbol == "ETHUSDT" { return None; }
            let vol = t["quoteVolume"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let p_change = t["priceChangePercent"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let last_price = t["lastPrice"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            Some(RawData { symbol, vol, p_change, last_price })
        }).collect();

        // 3. Sort Volume
        raw_candidates.sort_by(|a, b| b.vol.partial_cmp(&a.vol).unwrap_or(Ordering::Equal));
        raw_candidates.truncate(INITIAL_UNIVERSE_LIMIT);
        let top_symbols: Vec<String> = raw_candidates.iter().map(|c| c.symbol.clone()).collect();

        // 4. Funding Map
        let mut funding_map = HashMap::new();
        if let Ok(premiums) = self.rest_client.fetch_premium_index().await {
            for p in premiums {
                if let (Some(sym), Some(fr_str)) = (p["symbol"].as_str(), p["lastFundingRate"].as_str()) {
                    if let Ok(fr) = fr_str.parse::<f64>() { funding_map.insert(sym.to_string(), fr); }
                }
            }
        }

        // 5. OI Data
        let oi_map = self.rest_client.fetch_open_interest_bulk(&top_symbols).await?;
        let oi_hist_map = self.rest_client.fetch_oi_hist_24h_bulk(&top_symbols).await?;

        // 6. ATR & Yesterday Vol Cache
        let now = chrono::Utc::now().timestamp_millis();
        let mut symbols_to_fetch_atr = Vec::new();
        let mut atr_map = HashMap::new();
        let mut yesterday_vol_map = HashMap::new();
        {
            let cache = self.atr_cache.read().await;
            for sym in &top_symbols {
                if let Some((atr_val, y_vol, ts)) = cache.get(sym) {
                    if now - ts < ATR_CACHE_TTL_MS {
                        atr_map.insert(sym.clone(), *atr_val);
                        yesterday_vol_map.insert(sym.clone(), *y_vol);
                        continue;
                    }
                }
                symbols_to_fetch_atr.push(sym.clone());
            }
        }
        if !symbols_to_fetch_atr.is_empty() {
            if let Ok(klines_map) = self.rest_client.fetch_klines_bulk(&symbols_to_fetch_atr, "1d", 15).await {
                let mut cache_write = self.atr_cache.write().await;
                for (sym, candles) in klines_map {
                    if candles.len() < 14 {
                        cache_write.insert(sym.clone(), (-1.0, 0.0, now));
                        atr_map.insert(sym.clone(), -1.0);
                        yesterday_vol_map.insert(sym, 0.0);
                        continue;
                    }
                    let yesterday_vol = candles[candles.len() - 2].quote_volume;
                    let mut tr_sum = 0.0;
                    let mut valid_tr_count = 0;
                    for i in 1..candles.len() {
                        let current = &candles[i]; let prev = &candles[i - 1];
                        let tr = (current.high - current.low).max((current.high - prev.close).abs()).max((current.low - prev.close).abs());
                        if current.close > 0.0 { tr_sum += tr / current.close; valid_tr_count += 1; }
                    }
                    let atr_pct = if valid_tr_count > 0 { tr_sum / valid_tr_count as f64 } else { 0.0 };
                    cache_write.insert(sym.clone(), (atr_pct, yesterday_vol, now));
                    atr_map.insert(sym.clone(), atr_pct);
                    yesterday_vol_map.insert(sym, yesterday_vol);
                }
            }
        }

        // 7. Merge Candidates
        let mut candidates: Vec<UniverseCandidate> = raw_candidates.into_iter().map(|r| {
            let oi_nominal = *oi_map.get(&r.symbol).unwrap_or(&0.0);
            let open_interest = oi_nominal * r.last_price;
            let oi_hist = *oi_hist_map.get(&r.symbol).unwrap_or(&0.0);
            let oi_change_24h_pct = if oi_hist > 0.0 { (oi_nominal - oi_hist) / oi_hist * 100.0 } else { 0.0 };
            let y_vol = *yesterday_vol_map.get(&r.symbol).unwrap_or(&0.0);
            let volume_change_24h_pct = if y_vol > 0.0 { (r.vol - y_vol) / y_vol * 100.0 } else { 0.0 };
            let funding_rate = *funding_map.get(&r.symbol).unwrap_or(&0.0);
            let volatility = *atr_map.get(&r.symbol).unwrap_or(&-1.0); 
            UniverseCandidate {
                symbol: r.symbol, quote_volume: r.vol, volume_change_24h_pct,
                open_interest, oi_change_24h_pct, volatility, funding_rate,
                vol_score: 0.0, vol_change_score: 0.0, oi_score: 0.0, oi_change_score: 0.0, atr_score: 0.0, fund_score: 0.0, 
                composite_score: 0.0, price_change_percent: r.p_change, last_price: r.last_price,
            }
        }).collect();

        candidates.sort_by(|a, b| b.open_interest.partial_cmp(&a.open_interest).unwrap_or(Ordering::Equal));
        candidates.truncate(OI_FILTER_LIMIT);

        // 8. Final Scoring Logic
        let log_vols: Vec<f64> = candidates.iter().map(|c| (c.quote_volume + 1.0).ln()).collect();
        let log_ois: Vec<f64> = candidates.iter().map(|c| (c.open_interest + 1.0).ln()).collect();
        let min_log_vol = log_vols.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_log_vol = log_vols.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_log_oi = log_ois.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_log_oi = log_ois.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_oi_chg = candidates.iter().map(|c| c.oi_change_24h_pct).fold(f64::INFINITY, f64::min);
        let max_oi_chg = candidates.iter().map(|c| c.oi_change_24h_pct).fold(f64::NEG_INFINITY, f64::max);
        let min_vol_chg = candidates.iter().map(|c| c.volume_change_24h_pct).fold(f64::INFINITY, f64::min);
        let max_vol_chg = candidates.iter().map(|c| c.volume_change_24h_pct).fold(f64::NEG_INFINITY, f64::max);

        let mut sorted_fundings: Vec<f64> = candidates.iter().map(|c| c.funding_rate).collect();
        sorted_fundings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let p10_val = *sorted_fundings.get((sorted_fundings.len() as f64 * 0.10) as usize).unwrap_or(&-0.001);
        let p90_val = *sorted_fundings.get((sorted_fundings.len() as f64 * 0.90) as usize).unwrap_or(&0.001);

        let normalize = |val: f64, min: f64, max: f64| -> f64 {
            if (max - min).abs() < f64::EPSILON { 50.0 } else { ((val - min) / (max - min)) * 100.0 }
        };

        for (i, c) in candidates.iter_mut().enumerate() {
            c.vol_score = normalize(log_vols[i], min_log_vol, max_log_vol);
            c.oi_score = normalize(log_ois[i], min_log_oi, max_log_oi);
            c.oi_change_score = normalize(c.oi_change_24h_pct, min_oi_chg, max_oi_chg);
            c.vol_change_score = normalize(c.volume_change_24h_pct, min_vol_chg, max_vol_chg);
            let v = c.volatility;
            c.atr_score = if v < 0.0 { 50.0 } else if v < ATR_SWEET_MIN { (v / ATR_SWEET_MIN) * 100.0 } 
                          else if v <= ATR_SWEET_MAX { 100.0 } 
                          else if v <= ATR_HIGH_THRESH { 100.0 - ((v - ATR_SWEET_MAX) / (ATR_HIGH_THRESH - ATR_SWEET_MAX)) * 100.0 } 
                          else { 0.0 };
            c.fund_score = if c.funding_rate <= p10_val || c.funding_rate >= p90_val { 0.0 } else { 100.0 };
            c.composite_score = (c.vol_score * 0.25) + (c.vol_change_score * 0.10) + (c.oi_score * 0.20) 
                              + (c.oi_change_score * 0.15) + (c.atr_score * 0.20) + (c.fund_score * 0.10);
        }

        candidates.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap_or(Ordering::Equal));
        candidates.truncate(limit);
        Ok(candidates)
    }

    pub async fn get_top_altcoins(&self) -> Result<Vec<String>> {
        let candidates = self.get_universe_candidates().await?;
        Ok(candidates.into_iter().map(|c| c.symbol).collect())
    }
}