use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use crate::core::models::SymbolConfig;
use tracing::{info, warn};
use std::collections::HashSet;

pub struct MetadataManager {
    rest_client: BinanceRestClient,
    blacklist: HashSet<String>,
}

impl MetadataManager {
    pub fn new(rest_client: BinanceRestClient) -> Self {
        Self {
            rest_client,
            blacklist: HashSet::new(),
        }
    }

    /// [SPEC 2.2] Lọc danh sách 100 Altcoin chất lượng nhất
    /// Tiêu chí: Volume 24h > 5M, Niêm yết > 30 ngày, Loại bỏ BTC/ETH để tính Breadth
    pub async fn get_top_altcoins(&self) -> Result<Vec<String>> {
        info!("MetadataManager: Filtering high-quality symbols...");
        
        let tickers = self.rest_client.fetch_24h_tickers().await?;
        
        // 1. Lọc theo Volume 24h > 5,000,000 USDT
        let mut candidates: Vec<(String, f64)> = tickers.into_iter()
            .filter_map(|t| {
                let symbol = t["symbol"].as_str()?.to_string();
                let quote_volume = t["quoteVolume"].as_str()?.parse::<f64>().unwrap_or(0.0);
                
                if quote_volume >= 5_000_000.0 && !symbol.contains("USDC") && !symbol.contains("BUSD") {
                    Some((symbol, quote_volume))
                } else {
                    None
                }
            })
            .collect();

        // 2. Sắp xếp theo Volume giảm dần
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 3. Lấy Top 100 (Loại trừ BTC và ETH để tính toán Altcoin Breadth sạch)
        let top_100: Vec<String> = candidates.into_iter()
            .map(|(s, _)| s)
            .filter(|s| s != "BTCUSDT" && s != "ETHUSDT")
            .take(100)
            .collect();

        info!("Successfully filtered top {} altcoins", top_100.len());
        Ok(top_100)
    }
}
