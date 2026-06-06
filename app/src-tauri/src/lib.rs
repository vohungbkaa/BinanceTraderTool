use core::pipeline::DataPipeline;
use core::db::Database;
use core::events::MarketEvent;
use engine::regime::MarketRegimeEngine;
use tauri::Manager;
use tracing::{info, error};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use std::sync::Arc;
use tokio::sync::broadcast;

mod core;
mod engine;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_config() -> serde_json::Value {
    let config = crate::core::config::AppConfig::load();
    serde_json::to_value(config).unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let file_appender = tracing_appender::rolling::daily("./logs", "binance_bot.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout.and(non_blocking))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_config])
        .setup(|app| {
            info!("Tauri application setup...");
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                // 1. Tạo Global Event Bus (Broadcast)
                // Bus này cho phép nhiều module cùng gửi và nhận MarketEvent
                let (global_tx, _) = broadcast::channel::<MarketEvent>(4096);

                // 2. Khởi tạo Database
                let db_url = "sqlite://data.db?mode=rwc";
                let db = match Database::new(db_url).await {
                    Ok(db) => Arc::new(db),
                    Err(e) => {
                        error!("Failed to initialize database: {}", e);
                        return;
                    }
                };

                // 3. Khởi chạy Phase 1: Market Regime Engine (Background Task)
                // Nó sẽ lắng nghe nến từ bus và phát lại kết quả bối cảnh lên bus
                let mut regime_engine = MarketRegimeEngine::new();
                let regime_rx = global_tx.subscribe();
                let regime_tx = global_tx.clone();
                tokio::spawn(async move {
                    regime_engine.run(regime_rx, regime_tx).await;
                });

                // 4. Khởi chạy Phase 0: Data Pipeline (Background Task)
                // [SPEC 2.1] Chỉ theo dõi BTCUSDT để xác định Market Regime (Bối cảnh chung)
                // Các Altcoin khác sẽ được quét ở Phase 2 (Scanner)
                let initial_symbols = vec!["BTCUSDT".to_string()];
                let mut pipeline = DataPipeline::new(initial_symbols, db, global_tx, app_handle);
                
                if let Err(e) = pipeline.start().await {
                    error!("Data Pipeline stopped: {}", e);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}



