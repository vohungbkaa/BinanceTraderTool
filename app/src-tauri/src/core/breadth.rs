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
        let config = crate::core::config::AppConfig::load();
        let tf = config.altcoin_analysis_timeframe;

        if total == 0 { return Ok(()); }

        let mut results = Vec::new();
        // Chia thành từng nhóm 5 symbols để tải song song
        for chunk in top_altcoins.chunks(5) {
            let mut tasks = Vec::new();
            for symbol in chunk {
                let symbol = symbol.clone();
                let db = self.db.clone();
                let rest_client = self.rest_client.clone();
                let tf_clone = tf.clone();
                
                tasks.push(tokio::spawn(async move {
                    let last_update = db.get_last_update_time(&symbol, &tf_clone).await.unwrap_or(0);
                    let candles_in_db = db.get_candles(&symbol, &tf_clone, 200).await.unwrap_or_default();
                    
                    let is_fresh = (now_ms - last_update) < 3600_000; // 1 giờ
                    let has_enough = candles_in_db.len() >= 200;

                    let candles = if is_fresh && has_enough {
                        candles_in_db
                    } else {
                        info!("BreadthEngine: Cache stale or incomplete for {}. Fetching from Binance...", symbol);
                        match rest_client.fetch_klines(&symbol, &tf_clone, 200).await {
                            Ok(data) => {
                                let mut state = SymbolIndicatorState::new();
                                for c in &data {
                                    let inds = state.next(c);
                                    let normalized_data = crate::core::models::NormalizedCandleData {
                                        candle: c.clone(),
                                        indicators: inds,
                                        ..Default::default()
                                    };
                                    let _ = db.insert_closed_candle(&normalized_data).await;
                                }
                                data
                            }
                            Err(_) => Vec::new(),
                        }
                    };

                    let mut final_indicators = None;
                    let mut last_close = 0.0;
                    if let Some(last_candle_val) = candles.last().cloned() {
                        last_close = last_candle_val.close;
                        let mut state = SymbolIndicatorState::new();
                        for c in candles {
                            final_indicators = Some(state.next(&c));
                        }
                    }
                    (last_close, final_indicators)
                }));
            }

            // Chờ tất cả task trong nhóm hoàn thành
            for task in tasks {
                if let Ok(res) = task.await {
                    results.push(res);
                }
            }
            // Sleep 500ms giữa các nhóm để nhả Rate Limit (rất an toàn)
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        for (last_close, final_indicators) in results {
            if let Some(inds) = final_indicators {
                if let Some(ema50) = inds.ema50 {
                    if last_close > ema50 { count_above_ema50 += 1; }
                }
                if let Some(ema200) = inds.ema200 {
                    if last_close > ema200 { count_above_ema200 += 1; }
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
