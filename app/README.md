# Binance Trading Bot V2 - Desktop Application

Ứng dụng Desktop chuyên dụng để quét tín hiệu và giao dịch tự động trên Binance Futures USDT-M, được xây dựng bằng công nghệ **Tauri + Vue 3 + Rust**.

## 1. Yêu cầu hệ thống

Trước khi bắt đầu, hãy đảm bảo bạn đã cài đặt:
- **Node.js** (Bản LTS)
- **Rust & Cargo** (Xem hướng dẫn tại [rust-lang.org](https://www.rust-lang.org/))
- **SQLite3** (Thường có sẵn trên macOS/Linux)

## 2. Hướng dẫn Chạy ứng dụng

### Chế độ Phát triển (Development)
Chế độ này khởi động cả giao diện Frontend và nhân Backend Rust với tính năng Log chi tiết trên Terminal.
```bash
cd app
npm run tauri dev
```
*Giao diện Desktop sẽ hiện ra và Log WebSocket/API sẽ in liên tục tại Terminal.*

### Build ứng dụng (Production)
Tạo file cài đặt `.dmg` (macOS) hoặc `.exe` (Windows).
```bash
cd app
npm run tauri build
```

---

## 3. Hướng dẫn làm việc với Database (SQLite)

Hệ thống sử dụng SQLite để lưu trữ nến đã đóng, chỉ báo kỹ thuật và rủi ro bối cảnh.
- **Vị trí file:** `app/src-tauri/data.db`

### Truy vấn nhanh qua Terminal
Bạn có thể dùng lệnh `sqlite3` để kiểm tra dữ liệu nến thực tế:

1. **Mở Database:**
   ```bash
   cd app/src-tauri
   sqlite3 data.db
   ```

2. **Xem 5 nến mới nhất có đầy đủ chỉ báo:**
   ```sql
   SELECT symbol, timeframe, open_time, close, ema200, structure 
   FROM closed_candles 
   ORDER BY open_time DESC 
   LIMIT 5;
   ```

3. **Xem các sự kiện rủi ro hệ thống:**
   ```sql
   SELECT event_type, payload, timestamp 
   FROM system_events 
   ORDER BY timestamp DESC;
   ```

4. **Thoát:** Gõ `.exit`

### Xóa Database để khởi động lại sạch
Nếu bạn muốn xóa toàn bộ cache và bắt đầu thu thập lại dữ liệu từ đầu:
```bash
cd app/src-tauri
rm data.db
```

---

## 4. Cấu trúc mã nguồn

- **Backend (Rust - Core Logic):** `src-tauri/src/core/`
  - `websocket.rs`: Thu thập dữ liệu realtime.
  - `indicators.rs`: Tính toán chỉ báo kỹ thuật.
  - `db.rs`: Quản lý lưu trữ SQLite.
  - `pipeline.rs`: Điều phối luồng dữ liệu (Event Bus).
- **Frontend (Vue 3 - UI):** `src/`
  - `App.vue`: Giao diện Dashboard chính.

## 5. Lưu ý Bảo mật & Hiệu năng
- **DB Caching:** Hệ thống tự động cache nến cũ vào SQLite để tránh bị Binance chặn IP khi khởi động lại bot liên tục.
- **Rate Limit:** Luôn đảm bảo thời gian nghỉ giữa các yêu cầu API (mặc định đã được cấu hình an toàn).
