use anyhow::Result;
use crate::core::rest::BinanceRestClient;
use crate::core::db::Database;
use tracing::{info, warn};
use crate::core::indicators::SymbolIndicatorState;
use std::sync::Arc;

pub struct BreadthEngine {
    rest_client: BinanceRestClient,
    db: Arc<Database>,
    pub market_breadth_ema50: f64,
    pub market_breadth_ema200: f64,
}

impl BreadthEngine {
    pub fn new(rest_client: BinanceRestClient, db: Arc<Database>) -> Self {
        Self {
            rest_client,
            db,
            market_breadth_ema50: 0.0,
            market_breadth_ema200: 0.0,
        }
    }

    /// [SPEC 2.3] Tính toán Market Breadth với cơ chế Cache để bảo vệ IP
    pub async fn update_breadth(&mut self, top_altcoins: &[String]) -> Result<()> {
        info!("BreadthEngine: Calculating Market Breadth (Intelligent Cache mode)...");
        
        let mut count_above_ema50 = 0;
        let mut count_above_ema200 = 0;
        let total = top_altcoins.len();
        let now_ms = chrono::Utc::now().timestamp_millis();

        if total == 0 { return Ok(()); }

        for symbol in top_altcoins {
            // Kiểm tra Cache: Cần 200 nến 1D và dữ liệu phải mới (trong vòng 1 giờ)
            let last_update = self.db.get_last_update_time(symbol, "1d").await.unwrap_or(0);
            let candles_in_db = self.db.get_candles(symbol, "1d", 200).await.unwrap_or_default();
            
            let is_fresh = (now_ms - last_update) < 3600_000; // 1 giờ
            let has_enough = candles_in_db.len() >= 200;

            let candles = if is_fresh && has_enough {
                candles_in_db
            } else {
                info!("BreadthEngine: Cache stale or incomplete for {}. Fetching from Binance...", symbol);
                match self.rest_client.fetch_klines(symbol, "1d", 200).await {
                    Ok(data) => {
                        let mut state = SymbolIndicatorState::new();
                        // Lưu cache
                        for c in &data {
                            let inds = state.next(c);
                            let mock_normalized = crate::core::models::NormalizedCandleData {
                                candle: c.clone(),
                                indicators: inds,
                                ..Default::default()
                            };
                            let _ = self.db.insert_closed_candle(&mock_normalized).await;
                        }
                        // Sleep để tránh block IP
                        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                        data
                    }
                    Err(_) => Vec::new(),
                }
            };

            if let Some(last_candle_val) = candles.last().cloned() {
                let mut state = SymbolIndicatorState::new();
                let mut final_indicators = None;
                for c in candles {
                    final_indicators = Some(state.next(&c));
                }

                if let Some(inds) = final_indicators {
                    if let Some(ema50) = inds.ema50 {
                        if last_candle_val.close > ema50 { count_above_ema50 += 1; }
                    }
                    if let Some(ema200) = inds.ema200 {
                        if last_candle_val.close > ema200 { count_above_ema200 += 1; }
                    }
                }
            }
        }

        self.market_breadth_ema50 = (count_above_ema50 as f64 / total as f64) * 100.0;
        self.market_breadth_ema200 = (count_above_ema200 as f64 / total as f64) * 100.0;

        info!("Market Breadth Updated: >EMA50: {:.2}%, >EMA200: {:.2}%", 
            self.market_breadth_ema50, self.market_breadth_ema200);

        Ok(())
    }
}
