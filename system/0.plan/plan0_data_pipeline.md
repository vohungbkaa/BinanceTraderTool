# PHASE 0 - DATA PIPELINE & PREPROCESSING

## 1. MỤC TIÊU
Xây dựng lớp hạ tầng dữ liệu ổn định, bảo đảm dữ liệu luôn realtime, hợp lệ và sẵn sàng cho các module phân tích phía sau mà không bị ảnh hưởng bởi giới hạn tần suất yêu cầu (Rate Limit) từ sàn.

## 2. QUY TẮC LỌC DANH SÁCH SYMBOL (SYMBOL FILTERING RULES)
Để tối ưu tải hệ thống và bảo đảm tính thanh khoản, hệ thống áp dụng các tiêu chí lọc symbol sau:

- **Loại tài sản:** Chỉ sử dụng Binance USDT-M Perpetual Futures.
- **Lịch sử dữ liệu:** Chỉ chọn các symbol có thời gian niêm yết lớn hơn **30 ngày**.
- **Thanh khoản:** Loại bỏ các symbol có khối lượng giao dịch **24h** thấp hơn ngưỡng cấu hình, ví dụ dưới **5M USDT**.
- **Cập nhật định kỳ:** Tự động làm mới danh sách symbol hợp lệ mỗi **24 giờ**.
- **Whitelist/Blacklist:** Hỗ trợ cấu hình thủ công các symbol ưu tiên hoặc cần loại trừ.

## 3. THU THẬP DỮ LIỆU THỊ TRƯỜNG (MARKET DATA INGESTION)

### A. PHÂN LOẠI LUỒNG DỮ LIỆU (DATA STREAM CLASSIFICATION)
Hệ thống phân biệt rõ **3 nhóm dữ liệu** để áp dụng logic xử lý phù hợp:

1. **Dữ liệu nến đang chạy (Live Candle / Kline Update):** Dữ liệu của nến chưa đóng (`is_closed: false`). Dùng cho giám sát realtime, cảnh báo sớm hoặc trailing stop.
2. **Nến đã đóng (Closed Candle):** Dữ liệu của nến đã xác nhận (`is_closed: true`). **Đây là nguồn dữ liệu duy nhất được dùng cho logic ra quyết định vào lệnh nhằm tránh tín hiệu nhiễu.**
3. **Dữ liệu sự kiện thị trường (Market Events):** Funding Rate, thanh khoản, và các dữ liệu bổ sung khác.

### B. PHƯƠNG THỨC THU THẬP
- **WebSocket (Ưu tiên):**
  - `<symbol>@kline_<interval>`: Đăng ký trực tiếp các khung thời gian **15m**, **4h**, **1d** từ Binance để bảo đảm độ chính xác và giảm sai số do tự tổng hợp dữ liệu.
  - `<symbol>@ticker`: Cập nhật giá và biến động **24h**.
  - `<symbol>@depth5`: Lấy **5** mức Bid/Ask để tính Spread và thanh khoản.
    - **Spread (bps):** `(Ask1 - Bid1) / MidPrice * 10000`
    - **Liquidity Score:** Tổng giá trị danh nghĩa của **top 5 levels**
  - `!markPrice@arr`: Funding Rate toàn thị trường.
- **REST API:** Dùng để lấy dữ liệu lịch sử khi khởi động (**Warm-up**) hoặc bù dữ liệu thiếu (**Gap Filling**).

## 4. LƯU TRỮ VÀ DUY TRÌ DỮ LIỆU (STORAGE & PERSISTENCE)

### A. CHIẾN LƯỢC LƯU TRỮ 3 TẦNG
1. **Bộ nhớ đệm nóng (Hot Cache - RAM/Redis):** Lưu từ **200 đến 500** nến gần nhất để tính toán chỉ báo nhanh.
2. **Kho lưu trữ bền vững (Persistent Store - SQLite/PostgreSQL):** Lưu lịch sử nến đã đóng và snapshot chỉ báo để hỗ trợ khởi động lại mà không cần gọi API quá nhiều.
3. **Kho lưu trữ dài hạn (Cold Archive - Optional):** Lưu dữ liệu raw dưới dạng **Parquet/CSV** cho mục đích backtest và phân tích dài hạn.

### B. TÍNH TOÁN CHỈ BÁO (INDICATORS)
- **Giá trị tạm thời (Provisional Values):** Tính trên nến đang chạy và cập nhật liên tục theo từng tick.
- **Giá trị xác nhận (Confirmed Values):** Chỉ lưu và sử dụng khi nến đã chính thức đóng.
- **Trạng thái warm-up:** Đánh dấu `is_warmup: true` nếu chưa đủ số lượng nến tối thiểu để tính chỉ báo, ví dụ cần ít nhất **200 nến** cho **EMA200**.

## 5. CHUẨN HÓA VÀ KIỂM TRA DỮ LIỆU (DATA SCHEMA & VALIDATION)

### A. SCHEMA MẪU CHO OHLCV VÀ INDICATORS
```json
{
  "symbol": "SOLUSDT",
  "timeframe": "15m",
  "open_time": 1716960000,
  "close_time": 1716960899,
  "open": 172.1,
  "high": 173.4,
  "low": 171.8,
  "close": 173.0,
  "volume": 125034.5,
  "is_closed": true,
  "indicators": {
    "ema20": 171.4,
    "ema50": 168.9,
    "ema200": 155.2,
    "atr14": 2.31,
    "adx14": 27.8
  },
  "metadata": {
    "is_warmup": false,
    "latency_ms": 120
  }
}
