// [PLATFORM CONFIG] Ngăn khởi tạo cửa sổ Console trên Windows trong môi trường Production (Release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Khai báo các module (thư viện nội bộ) của ứng dụng.
mod core;
mod engine;

/// [ENTRY POINT] Điểm khởi chạy chính của tiến trình Backend.
/// Hàm này được Hệ điều hành thực thi đầu tiên khi ứng dụng được khởi động.
fn main() {
    // Ủy quyền khởi chạy cho hàm run() trong crate thư viện (lib.rs).
    // Tên 'app_lib' được định nghĩa trong file Cargo.toml.
    app_lib::run()
}


