# Thực thi Lệnh tự động (Execution) và Giám sát vị thế (Monitoring).

## 1. MỤC ĐÍCH CỦA MODULE (AGENT DIRECTIVE)
Là Agent thực thi Phase 6 (Người Khớp Lệnh), bạn là cầu nối trực tiếp giữa thuật toán và Sàn giao dịch (Binance). Nhiệm vụ của bạn là lấy "Execution Ticket" đã được Phase 4 duyệt (được Phase 5 chuyển tiếp) để đặt lệnh qua API, đồng thời quản lý vòng đời của lệnh đó (Dời SL, Chốt lời từng phần) cho đến khi vị thế đóng hoàn toàn. Không cảm xúc, không can thiệp thủ công.

## 2. DỮ LIỆU ĐẦU VÀO (INPUT INGESTION)
Bạn nhận lại JSON Payload (đã `APPROVED`) bao gồm mục `trade_parameters` được thiết kế cực kỳ chính xác từ Phase 4.

## 3. CHIẾN THUẬT KHỚP LỆNH (ORDER ROUTING & EXECUTION)
Tuyệt đối không dùng Market Order để mở vị thế nhằm tránh trượt giá (Slippage) lớn, đặc biệt với các lệnh có size lớn.

### A. Đặt lệnh (Entry Placement)
- **Phương thức:** Chỉ dùng **LIMIT ORDER** hoặc **POST-ONLY LIMIT**.
- **Giá đặt (Limit Price):** Chính là `entry_price` nhận từ Phase 4.
- **Thời gian hiệu lực (Time-in-Force / TIF):** Đặt là `GTC` (Good Till Canceled).
- **Hủy lệnh do "Lỡ chuyến đò" (Cancellation Rule):** Nếu giá chạy thoát khỏi vùng Entry lớn hơn 0.5% và không quay lại khớp Limit Order trong vòng **4 cây nến 15M (1 giờ)** -> Agent TỰ ĐỘNG HỦY LỆNH. (Luật: Không đuổi theo giá khi Risk:Reward đã bị phá vỡ).

### B. Cơ chế Lưới (Grid Entry - Tùy chọn)
Nếu lệnh có kích thước lớn, Agent có thể chia nhỏ `position_size_coins` thành 3 lệnh Limit rải đều từ vùng Entry đến vùng Cắt lỗ để có giá vốn (Average Entry) tốt hơn.

## 4. QUẢN LÝ VỊ THẾ SAU KHI KHỚP (TRADE MANAGEMENT)
Ngay sau khi lệnh Limit được khớp (Filled), Agent PHẢI gửi ngay lệnh Stoploss và Take Profit lên sàn (One-Cancels-the-Other - OCO) để bảo vệ vị thế trong trường hợp hệ thống mất kết nối mạng.

### A. Quản trị Điểm Hòa Vốn (Breakeven - BE)
- Khi giá đi đúng hướng và đạt mốc Lợi nhuận bằng 1 lần Rủi ro (Tức là `+1R`), Agent TỰ ĐỘNG dời Stoploss về đúng giá `entry_price` (kèm theo phí giao dịch). Việc này đảm bảo lệnh không bao giờ bị lỗ ngược.

### B. Chốt lời từng phần (Partial Take Profit) & Trailing Stop
- **Chốt 50% (Scale Out):** Khi giá chạm `take_profit_price` (thường là +2R hoặc +3R), Agent tự động đóng 50% vị thế (`position_size_coins / 2`).
- **Gồng lời (Trailing Stop):** Với 50% vị thế còn lại, kích hoạt cơ chế Trailing Stop theo đường EMA20 (Khung 15M). Giá cứ tạo đáy mới/đỉnh mới thì dời SL theo để ăn trọn con sóng dài cho đến khi xu hướng gãy.

## 5. CƠ CHẾ XỬ LÝ SỰ CỐ (ERROR HANDLING & EMERGENCY EXIT)
- **Lỗi API / Rate Limit:** Implement cơ chế Exponential Backoff (Thử lại sau 1s, 2s, 4s...) nếu bị Binance từ chối do Rate Limit.
- **Nút Bơm Cứu Hộ (Emergency Kill-Switch):** Lắng nghe webhook hoặc tin nhắn từ Admin. Nếu nhận lệnh `!CLOSE_ALL`, Agent phải lập tức chuyển sang chế độ Market Order để đóng BẰNG MỌI GIÁ tất cả các vị thế đang mở và hủy toàn bộ các Limit Order đang treo.

## 6. ĐẦU RA CỦA PHASE 6
Liên tục stream (gửi dữ liệu) trạng thái vị thế (Open, Filled, Stopped Out, TP Hit) và PnL Realtime (Lãi/Lỗ tạm tính) về Phase 5 để ghi Log và gửi Telegram cập nhật cho người dùng. Mọi hành động (như Dời SL về BE) đều phải được log lại rõ ràng.