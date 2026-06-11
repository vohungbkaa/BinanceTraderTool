use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use tracing::{info, warn};
use std::collections::{HashSet, HashMap};
use serde::{Deserialize, Serialize};

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
    /// CHI TIẾT THUẬT TOÁN (BACKEND):
    /// 1. Lọc sơ bộ: Chỉ lấy các cặp giao dịch Futures (PERPETUAL) đang hoạt động (TRADING). Bỏ qua Stablecoins, BTC, ETH.
    /// 2. Lấy ra Top 150 theo Open Interest (OI): Mục đích là để loại bỏ các đồng coin có Volume ảo (bị làm giá) trước khi chấm điểm. Chỉ những coin có tiền thật đang giữ lệnh mới được chọn.
    /// 3. Xác định Min/Max: Tìm giá trị nhỏ nhất và lớn nhất trong danh sách cho 4 tiêu chí: Volume, Open Interest (OI), Độ biến động (ATR), và Funding Rate tuyệt đối.
    /// 4. Chuẩn hóa Min-Max (Đưa về thang điểm 0 - 100):
    ///    - Điểm Volume & Điểm OI: Tính theo tỉ lệ thuận `(Value - Min) / (Max - Min) * 100`. Thanh khoản và dòng tiền càng lớn thì điểm càng cao.
    ///    - Điểm Biến động (ATR): Tính theo tỉ lệ nghịch `100 - ((Value - Min) / (Max - Min) * 100)`. Điểm cao nhất dành cho các đồng coin đang tích lũy, biên độ nén lại, nhằm tránh nhảy vào các coin đã bay quá mạnh dễ dính quét Stoploss.
    ///    - Điểm Funding Rate: Tính theo tỉ lệ nghịch `100 - ((|Funding| - Min) / (Max - Min) * 100)`. Điểm cao nhất dành cho các đồng coin có Funding Rate xoay quanh mức 0 (thể hiện tâm lý mua/bán đang cân bằng, ít rủi ro thanh lý hàng loạt).
    /// 5. Tính điểm tổng hợp (Composite Score 0-100) dựa trên trọng số:
    ///    `40% Điểm Volume + 30% Điểm OI + 20% Điểm Biến động + 10% Điểm Funding`.
    /// 6. Sắp xếp giảm dần theo điểm tổng hợp và trả về danh sách (Mặc định lấy 100 coin tốt nhất).
    ///
    /// GHI CHÚ GIAO DIỆN QUẢN TRỊ (ADMIN UI):
    /// - Cột "Hạng (Rank)" đã được thay bằng "Điểm Tổng Hợp (Composite Score)".
    /// - Ngay dưới mỗi thông số của coin sẽ hiển thị rõ "Score: X" (thang 0-100) để người dùng dễ dàng hiểu tại sao đồng coin này lại được chọn vào Top.
    /// - Có chú thích rõ ràng: Điểm cao ở mức Biến động (ATR) nghĩa là giá đang nén; Điểm cao ở Funding nghĩa là thị trường đang cân bằng.
    pub async fn get_universe_candidates(&self) -> Result<Vec<UniverseCandidate>> {
        info!("MetadataManager: Building Universe with Min-Max Normalized Composite Scoring...");
        let limit = crate::core::config::AppConfig::load().altcoin_count;

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

        // 2. Fetch 24h Tickers để lấy Volume và Proxy Volatility (ATR%)
        let tickers = self.rest_client.fetch_24h_tickers().await?;
        
        #[derive(Clone)]
        struct RawData {
            symbol: String,
            vol: f64,
            volatility: f64,
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

            let vol = t["quoteVolume"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let high = t["highPrice"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let low = t["lowPrice"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let open = t["openPrice"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let p_change = t["priceChangePercent"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let last_price = t["lastPrice"].as_str()?.parse::<f64>().unwrap_or(0.0);
            
            // Volatility Proxy (ATR% estimate) = (High - Low) / Open
            let volatility = if open > 0.0 { (high - low) / open } else { 0.0 };

            Some(RawData { symbol, vol, volatility, p_change, last_price })
        }).collect();

        // 3. Sort by Volume & take Top 200 (Initial broad filter)
        raw_candidates.sort_by(|a, b| b.vol.partial_cmp(&a.vol).unwrap());
        raw_candidates.truncate(200);

        let top_200_symbols: Vec<String> = raw_candidates.iter().map(|c| c.symbol.clone()).collect();

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

        // 5. Fetch Open Interest (Parallel)
        let oi_map = self.rest_client.fetch_open_interest_bulk(&top_200_symbols).await?;

        // 6. Merge Data & Filter Top 150 by OI (Tránh Wash Trading)
        let mut candidates: Vec<UniverseCandidate> = raw_candidates.into_iter().map(|r| {
            let open_interest = *oi_map.get(&r.symbol).unwrap_or(&0.0);
            let funding_rate_abs = *funding_map.get(&r.symbol).unwrap_or(&0.0);
            UniverseCandidate {
                symbol: r.symbol,
                quote_volume: r.vol,
                open_interest,
                volatility: r.volatility,
                funding_rate_abs,
                vol_score: 0.0, oi_score: 0.0, atr_score: 0.0, fund_score: 0.0, composite_score: 0.0,
                price_change_percent: r.p_change,
                last_price: r.last_price,
            }
        }).collect();

        candidates.sort_by(|a, b| b.open_interest.partial_cmp(&a.open_interest).unwrap());
        candidates.truncate(150);

        // 7. Calculate Min-Max Normalization (0 - 100)
        let min_vol = candidates.iter().map(|c| c.quote_volume).fold(f64::INFINITY, f64::min);
        let max_vol = candidates.iter().map(|c| c.quote_volume).fold(0.0, f64::max);
        
        let min_oi = candidates.iter().map(|c| c.open_interest).fold(f64::INFINITY, f64::min);
        let max_oi = candidates.iter().map(|c| c.open_interest).fold(0.0, f64::max);
        
        let min_atr = candidates.iter().map(|c| c.volatility).fold(f64::INFINITY, f64::min);
        let max_atr = candidates.iter().map(|c| c.volatility).fold(0.0, f64::max);
        
        let min_fund = candidates.iter().map(|c| c.funding_rate_abs).fold(f64::INFINITY, f64::min);
        let max_fund = candidates.iter().map(|c| c.funding_rate_abs).fold(0.0, f64::max);

        // Helper function for Min-Max 0-100 (Safe division)
        let normalize = |val: f64, min: f64, max: f64| -> f64 {
            if max == min { 100.0 } else { ((val - min) / (max - min)) * 100.0 }
        };

        // 8. Apply Scores & Calculate Composite
        for c in candidates.iter_mut() {
            // Tỷ lệ thuận (Cao là tốt)
            c.vol_score = normalize(c.quote_volume, min_vol, max_vol);
            c.oi_score = normalize(c.open_interest, min_oi, max_oi);
            
            // Tỷ lệ nghịch (Thấp là tốt -> Lấy 100 - giá trị chuẩn hóa)
            // ATR thấp = Nén biến động (Sweet spot); Funding gần 0 = Cân bằng (Tránh Squeeze)
            c.atr_score = 100.0 - normalize(c.volatility, min_atr, max_atr);
            c.fund_score = 100.0 - normalize(c.funding_rate_abs, min_fund, max_fund);

            // Composite Score = 40% Vol + 30% OI + 20% ATR (Nén) + 10% Fund (Cân bằng)
            c.composite_score = (c.vol_score * 0.4) 
                              + (c.oi_score * 0.3) 
                              + (c.atr_score * 0.2) 
                              + (c.fund_score * 0.1);
        }

        // 9. Sort by Composite Score Descending (Highest Score is Best)
        candidates.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap());

        // 10. Truncate to Final Target (Top 100)
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
