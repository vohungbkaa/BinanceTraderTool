use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use tracing::{info, warn};
use std::collections::{HashSet, HashMap};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniverseCandidate {
    pub symbol: String,
    pub quote_volume: f64,
    pub open_interest: f64,
    pub volatility: f64,
    pub funding_rate_abs: f64,
    
    pub vol_score: f64,
    pub oi_score: f64,
    pub atr_score: f64,
    pub fund_score: f64,
    
    pub composite_score: f64,
    
    pub price_change_percent: f64,
    pub last_price: f64,
}

pub struct MetadataManager {
    rest_client: BinanceRestClient,
}

impl MetadataManager {
    pub fn new(rest_client: BinanceRestClient) -> Self {
        Self {
            rest_client,
        }
    }

    /// [SPEC 2.2] Xây dựng danh sách 100 đồng Altcoin tiềm năng nhất để đưa vào quét tín hiệu.
    /// Thuật toán này giúp bộ lọc thông minh hơn, tìm ra những đồng coin có dòng tiền thật và biên độ giá an toàn.
    ///
    /// CHI TIẾT THUẬT TOÁN (BACKEND - ADVANCED QUANT MODEL):
    /// 1. Lọc sơ bộ: Chỉ lấy hợp đồng PERPETUAL & trạng thái TRADING. Bỏ qua Stablecoins, BTC, ETH.
    /// 2. Top Volume & Top OI: Lấy Top 200 Volume, sau đó cắt lấy Top 150 OI để loại bỏ wash trading.
    /// 3. Tính toán Điểm Thành Phần (0 - 100):
    ///    - Điểm Volume & OI: Sử dụng Logarit tự nhiên `ln(value + 1)` trước khi Min-Max Normalization để tránh bị Outlier (DOGE, XRP) bóp méo phân phối.
    ///    - Điểm Biến động (ATR Sweet Spot): Áp dụng phân phối hình chuông. Vùng lý tưởng (3% - 8%) đạt 100 điểm. Dưới 2% (chết lâm sàng) hoặc trên 15% (quá nóng) sẽ bị trừ điểm nặng.
    ///    - Điểm Funding Rate (Threshold): Dưới 0.01% đạt 100 điểm tuyệt đối. Từ 0.01% đến 0.03% giảm dần về 0. Cực đoan > 0.03% nhận 0 điểm để tránh rủi ro Squeeze.
    /// 4. Tính điểm tổng hợp (Composite Score 0-100) dựa trên trọng số:
    ///    `40% Vol + 30% OI + 20% ATR + 10% Funding`.
    pub async fn get_universe_candidates(&self) -> Result<Vec<UniverseCandidate>> {
        info!("MetadataManager: Building Universe with Advanced Quant Scoring...");
        let limit = crate::core::config::AppConfig::load().altcoin_count;

        // Constants for Scoring Logic (Can be moved to AppConfig later)
        const INITIAL_UNIVERSE_LIMIT: usize = 300;
        const OI_FILTER_LIMIT: usize = 150;
        
        const ATR_SWEET_MIN: f64 = 0.02;      // 2% - Mức tối thiểu của vùng lý tưởng
        const ATR_SWEET_MAX: f64 = 0.08;      // 8%
        const ATR_HIGH_THRESH: f64 = 0.15;    // 15%
        
        const FUNDING_SAFE_THRESH: f64 = 0.0005;  // 0.05%
        const FUNDING_RISK_THRESH: f64 = 0.0015;  // 0.15%

        // 1. Lọc Metadata từ Exchange Info (Chỉ lấy PERPETUAL & TRADING)
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

        // 2. Fetch 24h Tickers để lấy Volume, Price Change và Last Price
        let tickers = self.rest_client.fetch_24h_tickers().await?;
        
        #[derive(Clone)]
        struct RawData {
            symbol: String,
            vol: f64,
            p_change: f64,
            last_price: f64,
        }

        let mut raw_candidates: Vec<RawData> = tickers.into_iter().filter_map(|t| {
            let symbol = t["symbol"].as_str()?.to_string();
            
            // Exclude rules
            if !valid_symbols.contains(&symbol)
                || !symbol.ends_with("USDT")
                || symbol.starts_with("USDC") || symbol.starts_with("BUSD")
                || symbol.starts_with("TUSD") || symbol.starts_with("FDUSD")
                || symbol.starts_with("USDP") || symbol.starts_with("USDE")
                || symbol.starts_with("DAI")  || symbol.starts_with("EUR")
                || symbol.starts_with("WBTC") || symbol.starts_with("WETH")
                || symbol == "BTCUSDT" || symbol == "ETHUSDT" {
                return None;
            }

            // Safe parsing using and_then and ok()
            let vol = t["quoteVolume"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let p_change = t["priceChangePercent"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let last_price = t["lastPrice"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            
            Some(RawData { symbol, vol, p_change, last_price })
        }).collect();

        // 3. Sort by Volume & take Top 300 (Wider initial filter to not miss high OI / low Vol coins)
        raw_candidates.sort_by(|a, b| b.vol.partial_cmp(&a.vol).unwrap());
        raw_candidates.truncate(INITIAL_UNIVERSE_LIMIT);

        let top_symbols: Vec<String> = raw_candidates.iter().map(|c| c.symbol.clone()).collect();

        // 4. Fetch Premium Index (Funding Rate)
        let mut funding_map = HashMap::new();
        if let Ok(premiums) = self.rest_client.fetch_premium_index().await {
            for p in premiums {
                if let (Some(sym), Some(fr_str)) = (p["symbol"].as_str(), p["lastFundingRate"].as_str()) {
                    if let Ok(fr) = fr_str.parse::<f64>() {
                        funding_map.insert(sym.to_string(), fr.abs()); // Dùng Absolute Funding
                    }
                }
            }
        }

        // 5. Fetch Open Interest (Parallel bulk fetch for the broader universe)
        let oi_map = self.rest_client.fetch_open_interest_bulk(&top_symbols).await?;

        // 6. Fetch Historical Klines (Parallel) to calculate True ATR (14 periods of 1D)
        // We fetch 15 candles to calculate True Range properly (needs previous close)
        info!("Fetching historical 1D klines for True ATR calculation...");
        let klines_map = self.rest_client.fetch_klines_bulk(&top_symbols, "1d", 15).await?;
        
        let mut atr_map = HashMap::new();
        for (sym, candles) in klines_map {
            if candles.len() < 14 {
                atr_map.insert(sym, -1.0); // Not enough data -> Neutral flag
                continue;
            }
            
            let mut tr_sum = 0.0;
            let mut valid_tr_count = 0;
            
            for i in 1..candles.len() {
                let current = &candles[i];
                let prev = &candles[i - 1];
                
                let h_l = current.high - current.low;
                let h_pc = (current.high - prev.close).abs();
                let l_pc = (current.low - prev.close).abs();
                
                let tr = h_l.max(h_pc).max(l_pc);
                
                // Cần chuẩn hóa True Range thành % (ATR Percentage) so với giá Close hiện tại để so sánh giữa các coin
                if current.close > 0.0 {
                    tr_sum += tr / current.close;
                    valid_tr_count += 1;
                }
            }
            
            let atr_pct = if valid_tr_count > 0 { tr_sum / valid_tr_count as f64 } else { 0.0 };
            atr_map.insert(sym, atr_pct);
        }

        // 7. Merge Data & Filter Top 150 by OI (Tránh Wash Trading)
        let mut candidates: Vec<UniverseCandidate> = raw_candidates.into_iter().map(|r| {
            let oi_nominal = *oi_map.get(&r.symbol).unwrap_or(&0.0);
            
            // [QUAN TRỌNG] Quy đổi OI ra USDT để phản ánh đúng dòng tiền Dollar-value
            let open_interest = oi_nominal * r.last_price;
            
            let funding_rate_abs = *funding_map.get(&r.symbol).unwrap_or(&0.0);
            let volatility = *atr_map.get(&r.symbol).unwrap_or(&-1.0); // True ATR%, default -1.0 cho an toàn (Neutral score)
            
            UniverseCandidate {
                symbol: r.symbol,
                quote_volume: r.vol,
                open_interest,
                volatility,
                funding_rate_abs,
                vol_score: 0.0, oi_score: 0.0, atr_score: 0.0, fund_score: 0.0, composite_score: 0.0,
                price_change_percent: r.p_change,
                last_price: r.last_price,
            }
        }).collect();

        candidates.sort_by(|a, b| b.open_interest.partial_cmp(&a.open_interest).unwrap_or(Ordering::Equal));
        candidates.truncate(OI_FILTER_LIMIT);

        // 7. Advanced Scoring Logic
        
        // Logarithmic transformation for Volume and OI
        let log_vols: Vec<f64> = candidates.iter().map(|c| (c.quote_volume + 1.0).ln()).collect();
        let log_ois: Vec<f64> = candidates.iter().map(|c| (c.open_interest + 1.0).ln()).collect();
        
        let min_log_vol = log_vols.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_log_vol = log_vols.iter().fold(0.0f64, |a, &b| a.max(b));
        
        let min_log_oi = log_ois.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_log_oi = log_ois.iter().fold(0.0f64, |a, &b| a.max(b));

        // Safely normalize avoiding division by zero
        let normalize = |val: f64, min: f64, max: f64| -> f64 {
            if (max - min).abs() < f64::EPSILON { 
                50.0 // Return neutral score if all values are identical
            } else { 
                ((val - min) / (max - min)) * 100.0 
            }
        };

        for (i, c) in candidates.iter_mut().enumerate() {
            // A. Log-Normalized Volume & OI Scores (40% & 30%)
            c.vol_score = normalize(log_vols[i], min_log_vol, max_log_vol);
            c.oi_score = normalize(log_ois[i], min_log_oi, max_log_oi);
            
            // B. ATR Sweet Spot Logic (20%)
            let v = c.volatility;
            c.atr_score = if v < 0.0 {
                50.0 // Coin mới niêm yết chưa đủ data -> Điểm trung tính 50
            } else if v < ATR_SWEET_MIN {
                (v / ATR_SWEET_MIN) * 100.0
            } else if v <= ATR_SWEET_MAX {
                100.0
            } else if v <= ATR_HIGH_THRESH {
                100.0 - ((v - ATR_SWEET_MAX) / (ATR_HIGH_THRESH - ATR_SWEET_MAX)) * 100.0
            } else {
                0.0
            };

            // C. Funding Rate Threshold Logic (10%)
            let f = c.funding_rate_abs;
            c.fund_score = if f <= FUNDING_SAFE_THRESH {
                100.0
            } else if f <= FUNDING_RISK_THRESH {
                100.0 - ((f - FUNDING_SAFE_THRESH) / (FUNDING_RISK_THRESH - FUNDING_SAFE_THRESH)) * 100.0
            } else {
                0.0
            };

            // Composite Score
            c.composite_score = (c.vol_score * 0.4) 
                              + (c.oi_score * 0.3) 
                              + (c.atr_score * 0.2) 
                              + (c.fund_score * 0.1);
        }

        // 8. Sort by Composite Score Descending (Highest Score is Best)
        candidates.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap_or(Ordering::Equal));

        // 9. Truncate to Final Target (Top 100)
        candidates.truncate(limit);

        Ok(candidates)
    }

    /// [SPEC 2.2] Trả về danh sách Tên Symbol cho Pipeline sử dụng
    pub async fn get_top_altcoins(&self) -> Result<Vec<String>> {
        let candidates = self.get_universe_candidates().await?;
        let top_n: Vec<String> = candidates.into_iter().map(|c| c.symbol).collect();
        info!("Successfully filtered top {} altcoins based on Min-Max composite score", top_n.len());
        Ok(top_n)
    }
}
