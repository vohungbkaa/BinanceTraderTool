#[cfg(test)]
mod integration_tests {
    use crate::core::pipeline::DataPipeline;
    use crate::core::db::Database;
    use crate::core::events::MarketEvent;
    use crate::core::models::{Candle, NormalizedCandleData};
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tracing::info;

    // #[tokio::test]
    // async fn test_full_phase0_pipeline_flow() {
    //     // 1. Setup Infrastructure
    //     let db_url = "sqlite::memory:";
    //     let db = Arc::new(Database::new(db_url).await.unwrap());
    //     let (global_tx, mut global_rx) = broadcast::channel::<MarketEvent>(100);
    //     
    //     // 2. Initialize Pipeline
    //     let symbols = vec!["BTCUSDT".to_string()];
    //     // Requires AppHandle which cannot be easily mocked in cargo test.
    //     // let mut pipeline = DataPipeline::new(symbols, db.clone(), global_tx.clone(), app_handle);
    //
    // }
}
