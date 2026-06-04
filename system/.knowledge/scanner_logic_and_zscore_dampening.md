# Phân Tích Logic & Xử Lý Tín Hiệu Nhiễu (Scanner Phase 2)

Tài liệu này ghi chú lại các quyết định thiết kế và cách giải quyết các vấn đề phát sinh trong thực tế khi vận hành Phase 2 (Altcoin Scanner), đặc biệt là trong việc tính toán Z-Score và ra quyết định Short.

---

## 1. Vấn Đề Khuếch Đại Z-Score Trong Thị Trường Đi Ngang (Flat Market)

### Hiện tượng (Case Study: `CRCLUSDT`)
Trong thực tế, có những thời điểm BTC giảm nhẹ (vd: `-3.67%`), và một Altcoin như CRCL giảm nhỉnh hơn một chút (vd: `-5.44%`). Mức chênh lệch cơ sở chỉ là `-1.77%`.
Tuy nhiên, thuật toán Scanner lại đánh giá CRCL rơi vào Hạng D (Cực Yếu) và báo tín hiệu Short.

### Nguyên nhân Toán học
Khi thị trường chung đang đi ngang hoặc biến động hẹp, **Độ lệch chuẩn (Standard Deviation) của toàn bộ các Altcoin sẽ cực kỳ nhỏ**.
Công thức tính Z-Score:
`Z-Score = (Base Difference - Mean) / StdDev`
Khi mẫu số (`StdDev`) tiệm cận về 0, một mức chênh lệch nhỏ xíu trên tử số cũng bị **khuếch đại** thành một Z-Score khổng lồ. Điều này tạo ra tín hiệu nhiễu (False Positive). Hệ thống lầm tưởng coin đang cực kỳ yếu, nhưng thực chất chỉ là nhiễu động nhỏ trong vùng sideway.

### Giải pháp: Flat Market Dampening (Làm Mượt)
Để khắc phục, hệ thống áp dụng bộ lọc giảm xóc:
- Nếu chênh lệch (Diff) ở khung 4H < 1.0% VÀ khung 1D < 2.0% -> Thị trường được xác định là "Flat".
- Điểm Z-Score cuối cùng (Final RS) sẽ bị **giảm 50% cường độ** (`* 0.5`).
- Kết quả: Các coin biến động nhỏ giọt sẽ bị kéo rank từ D lên C, loại khỏi danh sách giao dịch rủi ro cao.

---

## 2. Tư Duy "Scalp Short" Đối Với Coin Đang Rơi

### Hiện tượng (Case Study: `MSTRUSDT`)
Bot báo tín hiệu `SHORT (Scalp Short)` đối với MSTRUSDT khi nó đã giảm hơn `-6.00%` trong ngày. Câu hỏi đặt ra là: *"Coin đã giảm sâu rồi có nên Short đuổi không?"*

### Logic Giao Dịch Của Bot (Momentum Trading)
Bản chất của việc tìm coin Hạng D (RS Score âm) là đi tìm những tài sản đang bị rút tiền mạnh nhất, vỡ hỗ trợ, và có lực bán tháo. Triết lý ở đây là **"Quán tính (Momentum)"**. Một vật đang rơi sẽ có xu hướng tiếp tục rơi dễ hơn là đi ngang.

Bot lọc ra MSTRUSDT là hoàn toàn đúng logic vì:
1.  Nó thực sự yếu hơn BTC một cách rõ rệt (Không bị nhầm lẫn do Flat Market như CRCL).
2.  Nó đã gãy cấu trúc EMA (EMA 50 cắt dưới EMA 200).
3.  Nó **không nằm trong diện Pump Protection** (Coin đang tăng > 5% trong ngày sẽ bị cấm Short để tránh cản tàu hỏa).

### Scalp Short vs Fomo Short
Hệ thống gọi đây là **Scalp Short**, mang ý nghĩa chiến thuật đặc thù:
- **Không phải để Short đuổi (Fomo):** Bot không bao giờ vào lệnh ngay lập tức ở giá hiện tại khi nến đang cắm mỏ.
- **Canh Me Hồi (Pullback):** Danh sách này được đưa sang Phase 3. Phase 3 sẽ chờ đợi MSTRUSDT hồi nhẹ lên các mốc kháng cự ngắn hạn (vd: EMA 15m, 5m). Khi giá vừa chạm kháng cự và cụp đầu, lệnh Short mới được kích hoạt.
- **Đánh nhanh rút gọn:** Mục tiêu là ăn phần dư âm của đà giảm (1-2%), không hold lệnh qua ngày để tránh rủi ro giá tạo đáy chữ V bật lên.

**Kết luận:** Scanner làm đúng nhiệm vụ là "Chỉ điểm mục tiêu yếu nhất". Việc vào lệnh như thế nào cho an toàn là trách nhiệm bảo vệ của Phase 3.

---

## 3. Lỗi Bắt Ngược Sóng "Cản Tàu Hỏa" (Counter-Trend False Positives)

### Hiện tượng (Case Study: `AERGOUSDT`)
Bot báo tín hiệu `SHORT (Scalp Short)` với Hạng D (`RS Score = -0.83`) đối với AERGOUSDT, mặc dù đồng coin này đang có mức tăng trưởng **+7.60%** trong ngày (khỏe hơn hẳn thị trường chung).

### Nguyên nhân: Xung đột Động lượng 4H và Xu hướng 1D
AERGOUSDT có một đợt tăng mạnh lên +7.60% (Trend 1D rất mạnh), nhưng sau đó ngay lập tức bị xả mạnh ở khung 4H hiện tại (rút râu trên rất dài).
Vì công thức RS Score áp dụng trọng số đa khung `(RS_4H * 0.7) + (RS_1D * 0.3)`, cú xả cực mạnh ở khung 4H (chiếm 70% trọng số) đã kéo sập toàn bộ điểm số, khiến tổng RS bị âm nặng.
Lúc này, ngưỡng bảo vệ "Pump Protection" cũ được cài đặt quá lỏng (`> 15.0%`), nên mức tăng +7.60% đã lọt qua lưới lọc, khiến hệ thống ra lệnh Short một đồng coin đang có xu hướng tăng mạnh trong ngày.

### Giải pháp: Nâng Cấp Khiên Bảo Vệ (Dynamic Pump Protection)
Tuyệt đối không Short chặn đầu xe lửa. Một đồng coin dù có bị xả ở khung 4H nhưng nếu nó vẫn giữ được đà tăng mạnh trong ngày thì đó có thể chỉ là một cú rũ hàng (shakeout) trước khi tăng tiếp.
Bộ lọc Pump Protection được siết chặt lại:
- **Cấm Short** nếu coin đang tăng `> 5.0%` trong ngày (bất chấp điểm 4H xấu đến đâu).
- **Cấm Short** nếu coin đang khỏe hơn BTC `> 5.0%` (Ví dụ: Coin -1% nhưng BTC -7% -> Coin đang cực kỳ lì đòn).

Với lớp khiên này, các trường hợp "xanh vỏ đỏ lòng" như AERGOUSDT sẽ bị loại bỏ hoàn toàn khỏi danh sách Short. Mọi nỗ lực đánh chặn đầu xu hướng đều bị cấm.
