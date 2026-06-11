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
    // 1. [LOGGING SETUP] Thiết lập hệ thống ghi nhật ký hoạt động.
    // Logs sẽ được ghi vào thư mục ./logs và đồng thời in ra màn hình Terminal.
    let file_appender = tracing_appender::rolling::daily("./logs", "binance_bot.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("app_lib=debug".parse().unwrap()))
        .with_timer(LocalTimer)
        .with_writer(std::io::stdout.and(non_blocking))
        .init();

    // 2. [TAURI BUILDER] Cấu hình khung ứng dụng Tauri.
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Đăng ký các "Command" (hàm Rust) để Frontend (Vue) có thể gọi qua Invoke API.
        .invoke_handler(tauri::generate_handler![greet, get_config])
        .setup(|app| {
            info!("Tauri application setup...");
            let app_handle = app.handle().clone();

            // [ASYNC RUNTIME] Khởi chạy các tác vụ logic nặng ở luồng nền (Background Tasks).
            // Sử dụng async runtime để không làm treo giao diện người dùng (UI Thread).
            tauri::async_runtime::spawn(async move {
                
                // 1. [EVENT BUS] Tạo Global Event Bus (Broadcast Channel).
                // Đây là "mạng bưu điện" nội bộ cho phép các module gửi/nhận dữ liệu mà không phụ thuộc trực tiếp vào nhau.
                let (global_tx, _) = broadcast::channel::<MarketEvent>(4096);

                // 2. [DATABASE] Khởi tạo kết nối SQLite.
                // Arc (Atomic Reference Count) giúp chia sẻ quyền truy cập DB an toàn giữa nhiều luồng.
                let db_url = "sqlite://data.db?mode=rwc";
                let db = match Database::new(db_url).await {
                    Ok(db) => Arc::new(db),
                    Err(e) => {
                        error!("Failed to initialize database: {}", e);
                        return;
                    }
                };

                // 3. [PHASE 1] Khởi chạy Market Regime Engine (Background Task).
                // Tác vụ này chạy độc lập (tokio::spawn) để luôn sẵn sàng lắng nghe và phân tích bối cảnh thị trường.
                // CHÚ Ý: Phải chạy trước Phase 0 để không bỏ lỡ bất kỳ nến dữ liệu đầu tiên nào.
                let mut regime_engine = MarketRegimeEngine::new();
                let regime_rx = global_tx.subscribe(); // Đăng ký nhận tin từ Bus.
                let regime_tx = global_tx.clone();     // Cổng phát tin của Engine.
                tokio::spawn(async move {
                    regime_engine.run(regime_rx, regime_tx).await;
                });

                // 4. [PHASE 0] Khởi chạy Data Pipeline (Hệ thống nạp dữ liệu gốc).
                // Đây là "nguồn cấp dữ liệu" chính. Sử dụng .await để chiếm dụng luồng nền hiện tại.
                // Khi vòi nước này mở ra, dữ liệu từ Binance sẽ bắt đầu đổ vào Bus cho các Phase khác xử lý.
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



