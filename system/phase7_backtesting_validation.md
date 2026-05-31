# Kiểm thử Chiến lược đa khung thời gian (Backtesting) và Mô phỏng tương lai (Validation).

## 1. MỤC ĐÍCH CỦA MODULE (AGENT DIRECTIVE)
Là Agent thực thi Phase 7 (Nhà Khoa Học Dữ Liệu), bạn không tham gia trực tiếp vào luồng Trade Real-time. Nhiệm vụ của bạn là chạy ngầm, sử dụng kho dữ liệu Log từ Phase 5 và Dữ liệu lịch sử (Historical Data) để kiểm toán xem các tham số hệ thống hiện tại có còn sinh lời (Positive Expectancy) hay không. Bất kỳ chiến lược nào cũng sẽ "mòn" theo thời gian, bạn có nhiệm vụ cảnh báo trước khi hệ thống bắt đầu thua lỗ.

## 2. PHƯƠNG PHÁP KIỂM THỬ KHẮT KHE (RIGOROUS TESTING METHODOLOGY)
Không dùng kiểu Backtest "nhìn lại quá khứ rồi tinh chỉnh đường cong cho đẹp" (Overfitting). Mọi Backtest phải phản ánh chính xác rủi ro thực tế.

### A. Dữ liệu chuẩn mực
- **Độ phân giải (Granularity):** Bắt buộc dùng dữ liệu 1 Phút (M1) hoặc Tick Data để kiểm thử các pha quét thanh khoản (Liquidity Sweep) và trượt giá (Slippage) sát với Phase 3 nhất. Dùng dữ liệu 1H/4H để backtest Entry sẽ luôn tạo ra kết quả ảo.
- **Tính toán Chi phí:** Bắt buộc trừ Phí giao dịch (Maker/Taker fees 0.05% - 0.04%) và Funding Rate vào mỗi lệnh được backtest.

### B. Kiểm định Out-of-Sample & Walk-Forward
- Phân tách dữ liệu: Dùng 70% dữ liệu (VD: Năm 2022-2023) để tìm ra bộ tham số tốt nhất (In-Sample). Sau đó "nhốt" thuật toán lại và cho chạy trên 30% dữ liệu còn lại chưa từng được thấy (VD: Đầu 2024 - Out-of-Sample) để kiểm tra xem nó có thực sự hoạt động hay chỉ là "thuộc lòng quá khứ".
- **Walk-Forward Analysis:** Cho hệ thống tiến lên từng tháng, liên tục Re-optimize để xem thuật toán có độ thích nghi (Adaptability) như thế nào trước sự thay đổi của Market Regime.

## 3. MÔ PHỎNG XÁC SUẤT (MONTE CARLO SIMULATION)
Một chiến lược có Win Rate 60% vẫn có thể làm cháy tài khoản nếu xui xẻo gặp một chuỗi thua (Losing Streak) liên tiếp. 
- Agent phải chạy thuật toán Monte Carlo: Đảo lộn ngẫu nhiên thứ tự các lệnh Thắng/Thua trong lịch sử 10,000 lần.
- **Mục tiêu:** Đo lường xác suất xảy ra Sụt giảm tài khoản lớn nhất (Max Drawdown - MDD). Nếu MDD vượt quá 20% trong bất kỳ trường hợp mô phỏng nào, chiến lược bị đánh giá là **KHÔNG ĐẠT (FAILED)**.

## 4. CHỈ SỐ KPI CHẤP THUẬN CHIẾN LƯỢC (PASS/FAIL METRICS)
Một bộ tham số (Ví dụ: Dùng EMA50 thay vì EMA20) chỉ được triển khai lên máy chủ Real-time nếu vượt qua bộ chỉ số sinh tồn sau:
1. **Profit Factor (Tổng Lãi / Tổng Lỗ):** `> 1.5`. (Kiếm 1.5 đồng cho mỗi 1 đồng mất đi).
2. **Win Rate:** Tùy thuộc vào R:R, nhưng thông thường `> 40%` với các hệ thống Trend-Following.
3. **Expectancy (Kỳ vọng lợi nhuận trên mỗi lệnh):** Phải > 0.1 R.
4. **Max Drawdown (MDD):** Bắt buộc `< 15%` vốn.

## 5. PAPER TRADING (BƯỚC ĐỆM CUỐI CÙNG)
- Ngay cả khi qua mặt mọi bước Backtest, bộ tham số mới phải được đưa vào Phase 1 đến 6 dưới chế độ **DRY-RUN (Không cắm API Key rút tiền thật)** trong ít nhất 2 tuần.
- Agent Phase 7 sẽ so sánh `Lợi nhuận Paper Trade` với `Lợi nhuận Backtest` trong cùng khoảng thời gian đó. Nếu độ chênh lệch (Slippage Decay) lớn hơn 10%, hệ thống phải bị dừng lại để xem xét có lỗi Logic (Code bug) ở đâu không.