# BINANCE FUTURES SCANNER — SYSTEM OVERVIEW (ENHANCED)

## 1. MỤC TIÊU HỆ THỐNG
Xây dựng một hệ thống quét (scanner) và giao dịch tự động trên Binance Futures với các tiêu chí:
- **An toàn:** Chỉ giao dịch khi bối cảnh thị trường thuận lợi.
- **Chất lượng:** Tập trung vào các Altcoin có sức mạnh tương đối (Relative Strength) tốt nhất.
- **Kỷ luật:** Loại bỏ cảm xúc, chỉ vào lệnh khi hội tụ đủ các điều kiện Setup xác suất cao.
- **Tối ưu:** Quản trị rủi ro chặt chẽ ở cả cấp độ lệnh và danh mục.

## 2. TRIẾT LÝ HOẠT ĐỘNG
- **Dòng tiền là chìa khóa:** Theo dõi sự chuyển dịch của dòng tiền từ BTC sang Altcoin và ngược lại.
- **Xác suất và Kỳ vọng:** Không dự đoán, chỉ hành động dựa trên các kịch bản có xác suất thắng cao và tỷ lệ Risk/Reward (R:R) tốt.
- **Chất lượng hơn Số lượng (Quality over Quantity):** Tối ưu hóa tần suất giao dịch, chấp nhận bỏ lỡ các tín hiệu nhiễu.

## 3. LỘ TRÌNH 8 PHASE (8-PHASE ARCHITECTURE)

### PHASE 0: DATA PIPELINE & PREPROCESSING
- Thiết lập kết nối Websocket realtime. Xử lý Rate-limit, Health-check và Reconnect.

### PHASE 1: MARKET REGIME & CONTEXT
- Phân tích BTC Trend, BTC.D và Biến động (Volatility) để xác định bối cảnh vĩ mô.

### PHASE 2: RELATIVE STRENGTH SCANNER
- Lọc thanh khoản (Volume, OI, Spread) và tìm kiếm các Leaders có RS tốt nhất.

### PHASE 3: ENTRY SETUP VALIDATION
- Chờ đợi Pullback về vùng giá trị và xác nhận Liquidity Sweep trên khung thời gian nhỏ.

### PHASE 4: RISK MANAGEMENT & SCORING
- Tính toán Position size và chấm điểm Setup. Áp dụng các bộ lọc rủi ro cấp danh mục.

### PHASE 5: SIGNAL ENGINE (TELEGRAM & DATABASE)
- Gửi cảnh báo chuyên nghiệp và lưu trữ dữ liệu để phục vụ phân tích.

### PHASE 6: EXECUTION & MONITORING (ADVANCED)
- Tự động vào lệnh (Limit orders), dời Stoploss và quản lý lợi nhuận (Trailing stop).

### PHASE 7: BACKTESTING & VALIDATION
- Kiểm thử chiến thuật trên dữ liệu lịch sử và Paper trading để chứng minh hiệu quả.

## 4. PHÂN TÁCH KIẾN TRÚC (ARCHITECTURAL DISTINCTION)
Để đảm bảo tính module và dễ mở rộng, hệ thống được phân tách thành các Engine độc lập:
- **Data Engine:** Chịu trách nhiệm thu thập và lưu trữ cache dữ liệu.
- **Decision Engine (Phase 1-4):** Chịu trách nhiệm phân tích và ra quyết định trade/không trade.
- **Signal Engine (Phase 5):** Chịu trách nhiệm thông báo và giao tiếp người dùng.
- **Execution Engine (Phase 6):** Chịu trách nhiệm tương tác trực tiếp với API sàn để đặt lệnh.

---
*Hệ thống được thiết kế theo dạng Module, cho phép dễ dàng nâng cấp và thay đổi chiến thuật mà không ảnh hưởng đến cấu trúc tổng thể.*
