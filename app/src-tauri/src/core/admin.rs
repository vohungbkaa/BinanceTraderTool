use tauri::State;
use crate::core::db::Database;
use crate::core::rest::BinanceRestClient;
use crate::core::metadata::MetadataManager;
use serde::{Deserialize, Serialize};

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
pub async fn get_top_altcoins_metadata(
    app_handle: tauri::AppHandle,
    db: State<'_, std::sync::Arc<Database>>,
) -> Result<Vec<crate::core::metadata::UniverseCandidate>, String> {
    let rest_client = BinanceRestClient::new();
    let manager = MetadataManager::new(rest_client);

    let candidates = manager.get_universe_candidates(Some(&app_handle)).await.map_err(|e| e.to_string())?;

    // Lưu vào DB để có thể truy xuất nhanh sau này
    let _ = db.save_universe_candidates(&candidates).await;

    Ok(candidates)
}

#[tauri::command]
pub async fn get_stored_universe(
    db: State<'_, std::sync::Arc<Database>>,
) -> Result<Vec<crate::core::metadata::UniverseCandidate>, String> {
    db.get_stored_universe_candidates().await.map_err(|e| e.to_string())
}
