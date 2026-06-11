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

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::FormatTime;

struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. [TRACING/LOGGING] Thiết lập hệ thống giám sát và nhật ký hoạt động.
    // Dữ liệu log được lưu vào tập tin theo ngày tại thư mục ./logs và in ra Standard Output.
    let file_appender = tracing_appender::rolling::daily("./logs", "binance_bot.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("app_lib=debug".parse().unwrap()))
        .with_timer(LocalTimer)
        .with_writer(std::io::stdout.and(non_blocking))
        .init();

    // 2. [APPLICATION BUILDER] Khởi tạo khung ứng dụng Tauri.
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Đăng ký các hàm xử lý (Handlers) cho phép Frontend Invoke trực tiếp vào Backend.
        .invoke_handler(tauri::generate_handler![
            greet, 
            get_config,
            crate::core::admin::get_db_candles,
            crate::core::admin::get_top_altcoins_metadata
        ])
        .setup(|app| {
            info!("Tauri application setup...");
            let app_handle = app.handle().clone();

            // [ASYNC RUNTIME] Khởi tạo môi trường thực thi bất đồng bộ cho các logic nghiệp vụ.
            // Các tác vụ này chạy độc lập với luồng xử lý giao diện (Main/UI Thread).
            tauri::async_runtime::spawn(async move {
                
                // 1. [GLOBAL EVENT BUS] Khởi tạo Broadcast Channel cho toàn hệ thống.
                // Cho phép truyền tin theo mô hình Pub/Sub giữa các module độc lập.
                let (global_tx, _) = broadcast::channel::<MarketEvent>(4096);

                // 2. [PERSISTENCE LAYER] Khởi tạo kết nối cơ sở dữ liệu SQLite.
                // Sử dụng Arc (Atomic Reference Count) để chia sẻ quyền truy cập DB an toàn qua các Thread.
                let db_url = "sqlite://data.db?mode=rwc";
                let db = match Database::new(db_url).await {
                    Ok(db) => Arc::new(db),
                    Err(e) => {
                        error!("Failed to initialize database: {}", e);
                        return;
                    }
                };

                // Đăng ký DB instance vào Managed State của Tauri để các Handler có thể sử dụng (qua tauri::State)
                app_handle.manage(db.clone());

                // 3. [PHASE 1] Market Regime Engine (Phân tích bối cảnh thị trường).
                // Kích hoạt Engine dưới dạng tác vụ nền để lắng nghe sự kiện từ Event Bus.
                // LƯU Ý: Phải khởi chạy Subscriber (Engine) trước khi Publisher (Pipeline) phát dữ liệu.
                let mut regime_engine = MarketRegimeEngine::new();
                let regime_rx = global_tx.subscribe(); 
                let regime_tx = global_tx.clone();     
                tokio::spawn(async move {
                    regime_engine.run(regime_rx, regime_tx).await;
                });

                // 4. [PHASE 0] Data Pipeline (Hệ thống tiếp nhận và điều phối dữ liệu).
                // Đây là Ingestion Engine chính của hệ thống. Sử dụng await để chiếm dụng luồng hiện tại.
                // Khi Pipeline hoạt động, dữ liệu từ sàn sẽ được nạp và phân phối lên Event Bus.
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



