use crate::core::db::Database;
use crate::core::indicators::SymbolIndicatorState;
use crate::core::pipeline::timeframe_to_ms;
use crate::core::rest::BinanceRestClient;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::events::MarketEvent;
use tauri::AppHandle;
use tauri::Emitter;

pub struct BreadthEngine {
    rest_client: BinanceRestClient,
    db: Arc<Database>,
    app_handle: AppHandle,
    // Sử dụng Interior Mutability để không cần lock toàn bộ Engine khi await
    pub market_breadth_ema50: Arc<RwLock<f64>>,
    pub market_breadth_ema200: Arc<RwLock<f64>>,
}

impl BreadthEngine {
    pub fn new(rest_client: BinanceRestClient, db: Arc<Database>, app_handle: AppHandle) -> Self {
        Self {
            rest_client,
            db,
            app_handle,
            market_breadth_ema50: Arc::new(RwLock::new(0.0)),
            market_breadth_ema200: Arc::new(RwLock::new(0.0)),
        }
    }

    /// [SPEC 2.3] Tính toán Market Breadth với cơ chế Cache để bảo vệ IP
    /// Hàm này không yêu cầu Mutex Lock vì chỉ đọc dữ liệu và trả về kết quả tính toán.
    pub async fn calculate_breadth(&self, top_altcoins: &[String]) -> Result<(f64, f64)> {
        self.calculate_breadth_internal(top_altcoins, true).await
    }

    async fn calculate_breadth_internal(
        &self,
        top_altcoins: &[String],
        emit_progress: bool,
    ) -> Result<(f64, f64)> {
        info!("BreadthEngine: Calculating Market Breadth (High-Speed Mode)...");

        let mut count_above_ema50 = 0;
        let mut count_above_ema200 = 0;
        let total = top_altcoins.len();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let config = crate::core::config::AppConfig::load();
        let tf = config.altcoin_analysis_timeframe;

        if total == 0 {
            return Ok((0.0, 0.0));
        }

        let completed_symbols = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let semaphore = Arc::new(tokio::sync::Semaphore::new(20)); // 20 luồng song song
        let mut join_handles = Vec::new();

        for symbol in top_altcoins {
            let symbol = symbol.clone();
            let db = self.db.clone();
            let rest_client = self.rest_client.clone();
            let tf_clone = tf.clone();
            let app_handle = emit_progress.then(|| self.app_handle.clone());
            let completed_symbols = completed_symbols.clone();
            let total = total;
            let permit = semaphore.clone().acquire_owned().await.unwrap();

            join_handles.push(tokio::spawn(async move {
                let _permit = permit;
                let last_update = db
                    .get_last_update_time(&symbol, &tf_clone)
                    .await
                    .unwrap_or(0);
                let interval_ms = timeframe_to_ms(&tf_clone);

                let is_fresh = (now_ms - last_update) < interval_ms;

                let candles = if is_fresh {
                    db.get_candles(&symbol, &tf_clone, 200)
                        .await
                        .unwrap_or_default()
                } else {
                    // Chỉ tải phần thiếu
                    let missing_candles = ((now_ms - last_update) / interval_ms).min(200) as u32;
                    let fetch_limit = if last_update == 0 {
                        200
                    } else {
                        missing_candles + 2
                    };

                    match rest_client
                        .fetch_klines(&symbol, &tf_clone, fetch_limit)
                        .await
                    {
                        Ok(data) => {
                            let mut state = SymbolIndicatorState::new();
                            for c in &data {
                                if c.close_time < now_ms {
                                    let inds = state.next(c);
                                    let normalized_data =
                                        crate::core::models::NormalizedCandleData {
                                            candle: c.clone(),
                                            indicators: inds,
                                            ..Default::default()
                                        };
                                    let _ = db.insert_closed_candle(&normalized_data).await;
                                }
                            }
                            // Sau khi chèn nến mới, lấy đủ 200 nến cuối từ DB để tính EMA chuẩn
                            db.get_candles(&symbol, &tf_clone, 200)
                                .await
                                .unwrap_or_default()
                        }
                        Err(_) => Vec::new(),
                    }
                };

                let mut last_close = 0.0;
                let mut final_indicators = None;
                if let Some(last) = candles.last() {
                    last_close = last.close;
                    let mut state = SymbolIndicatorState::new();
                    for c in candles {
                        final_indicators = Some(state.next(&c));
                    }
                }

                let done = completed_symbols.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if let Some(handle) = app_handle {
                    let progress = 60.0 + (done as f64 / total as f64) * 30.0;
                    let _ = handle.emit(
                        "market-event",
                        &MarketEvent::SyncProgress {
                            step: "BREADTH".to_string(),
                            progress,
                            message: format!("Breadth Analysis: {}/{}", done, total),
                        },
                    );
                }

                (last_close, final_indicators)
            }));
        }

        let mut results = Vec::new();
        for handle in join_handles {
            if let Ok(res) = handle.await {
                results.push(res);
            }
        }

        for (last_close, final_indicators) in results {
            if let Some(inds) = final_indicators {
                if let Some(ema50) = inds.ema50 {
                    if last_close > ema50 {
                        count_above_ema50 += 1;
                    }
                }
                if let Some(ema200) = inds.ema200 {
                    if last_close > ema200 {
                        count_above_ema200 += 1;
                    }
                }
            }
        }

        let ema50_val = (count_above_ema50 as f64 / total as f64) * 100.0;
        let ema200_val = (count_above_ema200 as f64 / total as f64) * 100.0;

        Ok((ema50_val, ema200_val))
    }

    /// Cập nhật kết quả tính toán vào trạng thái nội tại (Yêu cầu Mutex Lock)
    pub async fn apply_results(&self, ema50: f64, ema200: f64) {
        let mut e50 = self.market_breadth_ema50.write().await;
        let mut e200 = self.market_breadth_ema200.write().await;
        *e50 = ema50;
        *e200 = ema200;
    }
}
