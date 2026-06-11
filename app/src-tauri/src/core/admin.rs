use tauri::State;
use crate::core::db::Database;
use crate::core::rest::BinanceRestClient;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AltcoinMetadata {
    pub symbol: String,
    pub quote_volume: f64,
    pub price_change_percent: f64,
    pub last_price: f64,
}

#[derive(Serialize)]
pub struct DbCandlesResponse {
    pub data: Vec<crate::core::models::NormalizedCandleData>,
    pub total: i64,
}

#[tauri::command]
pub async fn get_db_candles(
    symbol: String,
    timeframe: String,
    limit: usize,
    db: State<'_, std::sync::Arc<Database>>,
) -> Result<DbCandlesResponse, String> {
    let data = db.search_candles_with_indicators(&symbol, &timeframe, limit)
        .await
        .map_err(|e| e.to_string())?;
        
    let total = db.count_search_candles(&symbol, &timeframe)
        .await
        .unwrap_or(0);
        
    Ok(DbCandlesResponse { data, total })
}

#[tauri::command]
pub async fn get_top_altcoins_metadata() -> Result<Vec<AltcoinMetadata>, String> {
    let rest_client = BinanceRestClient::new();
    let tickers = rest_client.fetch_24h_tickers().await.map_err(|e| e.to_string())?;
    
    let config = crate::core::config::AppConfig::load();
    let limit = config.altcoin_count;
    
    let mut candidates: Vec<AltcoinMetadata> = tickers.into_iter()
        .filter_map(|t| {
            let symbol = t["symbol"].as_str()?.to_string();
            let quote_volume = t["quoteVolume"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let price_change_percent = t["priceChangePercent"].as_str()?.parse::<f64>().unwrap_or(0.0);
            let last_price = t["lastPrice"].as_str()?.parse::<f64>().unwrap_or(0.0);
            
            if quote_volume >= 5_000_000.0 
                && symbol.ends_with("USDT")
                && !symbol.starts_with("USDC") 
                && !symbol.starts_with("BUSD")
                && !symbol.starts_with("TUSD")
                && !symbol.starts_with("FDUSD")
                && !symbol.starts_with("USDP")
                && !symbol.starts_with("USDE")
                && !symbol.starts_with("DAI")
                && !symbol.starts_with("EUR")
                && !symbol.starts_with("WBTC")
                && !symbol.starts_with("WETH")
                && symbol != "BTCUSDT" 
                && symbol != "ETHUSDT"
                && symbol != "BTCDOMUSDT"
                && symbol != "DEFIUSDT" {
                Some(AltcoinMetadata {
                    symbol,
                    quote_volume,
                    price_change_percent,
                    last_price,
                })
            } else {
                None
            }
        })
        .collect();

    candidates.sort_by(|a, b| b.quote_volume.partial_cmp(&a.quote_volume).unwrap());
    candidates.truncate(limit);
    
    Ok(candidates)
}
