// [CẤU HÌNH WINDOWS] Ngăn hiện cửa sổ Console (màn hình đen) khi chạy bản release trên Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Khai báo các module nội bộ của dự án.
mod core;
mod engine;

/// [ENTRY POINT] Điểm khởi đầu tuyệt đối của phần Backend (Rust).
/// Hệ điều hành sẽ gọi hàm này đầu tiên khi ứng dụng được kích hoạt.
fn main() {
    // Gọi hàm run() từ thư viện nội bộ 'app_lib' (được định nghĩa trong lib.rs).
    // Tên 'app_lib' được cấu hình trong file Cargo.toml.
    app_lib::run()
}


