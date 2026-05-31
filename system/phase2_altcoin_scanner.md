# Lọc và xếp hạng Altcoin theo Sức mạnh tương đối (RS) dựa trên bối cảnh thị trường (Market Regime).

## 1. MỤC ĐÍCH CỦA MODULE (AGENT DIRECTIVE)
Là Agent thực thi Phase 2, nhiệm vụ của bạn là tiếp nhận `Context` (Bối cảnh) từ Phase 1 và tiến hành quét toàn bộ thị trường để tìm ra các Altcoin phù hợp nhất. Bạn sử dụng Sức mạnh tương đối (Relative Strength - RS) để tìm ra các "Leader" (dẫn dắt đà tăng) hoặc "Laggard" (yếu kém nhất, dẫn dắt đà giảm). 

*Lưu ý cốt lõi:* Bạn CHỈ ĐƯỢC PHÉP hoạt động khi nhận được trạng thái `allow_alt_scan = true` từ Phase 1. Nếu Phase 1 gửi tín hiệu ngắt, bạn phải dừng toàn bộ tiến trình quét.

## 2. BỘ LỌC TƯ CÁCH (ELIGIBILITY FILTER)
Trước khi tính toán sức mạnh, bạn phải loại bỏ ngay các Altcoin không đủ tiêu chuẩn thanh khoản để bảo vệ hệ thống khỏi trượt giá (Slippage) và nguy cơ thao túng (Manipulation):
- **Khối lượng giao dịch (Volume 24h):** > 50M USDT (Tính trung bình 3 ngày gần nhất).
- **Hợp đồng mở (Open Interest - OI):** > 10M USDT.
- **Thanh khoản Orderbook (Liquidity Score):** Spread < 0.05% và trượt giá cho một lệnh quy mô chuẩn (VD: $50,000) không vượt quá 0.1%.
- **Thời gian niêm yết (Listing Age):** > 30 ngày (Loại bỏ các token mới lên sàn có biến động quá nhiễu).

## 3. THUẬT TOÁN TÍNH RELATIVE STRENGTH (NORMALIZED RS SCORE)
Để so sánh công bằng giữa một coin có hệ số biến động lớn (như MEME coin) và một coin vốn hoá lớn, bạn PHẢI dùng Z-Score để chuẩn hoá (Normalize) RS. Không dùng % chênh lệch đơn thuần.

### A. Phương pháp tính toán
1.  **Biến thiên cơ sở:** `Diff = %Change(Altcoin, lookback) - %Change(BTC, lookback)`.
2.  **Chuẩn hóa Z-Score:** `RS_Score = (Diff - Mean_Diff_Market) / StdDev_Diff_Market`. (Trong đó Market là rổ các Altcoin đã lọt qua Bộ lọc tư cách).
3.  **Trọng số Đa khung thời gian (Timeframes Weighting):**
    `Final_RS = (RS_Score_4H * 0.7) + (RS_Score_1D * 0.3)`.

### B. Phân loại Sức mạnh (RS Rating)
- **RS > 1.5:** Hạng A (Mạnh vượt trội - Dòng tiền chủ động gom hàng).
- **0.5 < RS <= 1.5:** Hạng B (Khỏe hơn thị trường).
- **-0.5 <= RS <= 0.5:** Hạng C (Đi ngang, Beta xấp xỉ 1 so với thị trường).
- **RS < -0.5:** Hạng D (Yếu vượt trội - Dòng tiền rút ra, ưu tiên các thiết lập Short).

## 4. QUY TẮC LỌC DANH SÁCH THEO BỐI CẢNH (CONTEXTUAL SELECTION RULES)
Đây là cổng logic quan trọng nhất. Bạn phải map trực tiếp biến `action_mode` nhận từ Phase 1 để áp dụng tiêu chí lọc tương ứng. Tuyệt đối không dùng một bộ lọc chung cho mọi bối cảnh.

| Action Mode (Từ Phase 1) | Tiêu chí Đưa vào Shortlist (Screening Rules) |
| :--- | :--- |
| **`Aggressive_Long`** | RS Rating **A hoặc B**. <br> Giá hiện tại > EMA200 (1D) VÀ EMA50(4H) > EMA200(4H). <br> OI đang tăng mạnh đồng pha với giá (Khẳng định tiền mới vào). |
| **`Aggressive_Short`** | RS Rating **D**. <br> Giá hiện tại < EMA200 (1D) VÀ EMA50(4H) < EMA200(4H). <br> OI tăng trong khi giá giảm (Build-up Short). |
| **`Scalp_Long` / `Scalp_Short`** | Nới lỏng cấu trúc EMA 1D. Tập trung vào RS khung ngắn hạn (15m, 1H) > 1.5. Động lượng (ADX khung 1H) > 25. Đánh nhanh rút gọn. |
| **`Mean_Reversion`** | Tìm kiếm sự thái quá (Over-extended). <br> Phục hồi (Long): RS < -2.5 và giá cách xa EMA50(1H) (Oversold). <br> Bán khống (Short): RS > 2.5 và giá cách xa EMA50(1H) (Overbought). |
| **`Off_System`** | Dừng toàn bộ hoạt động quét, trả về Shortlist rỗng (`[]`). |

## 5. CHẤM ĐIỂM VÀ XẾP HẠNG SHORTLIST (SHORTLIST RANKING)
Các Altcoin lọt qua vòng Screening ở Mục 4 sẽ được chấm điểm để chọn ra Top 3 - Top 5 coin "tinh nhuệ" nhất chuyển giao cho Phase 3.

**Công thức Xếp hạng (Ranking Score):**
`Rank_Score = (Final_RS * 0.4) + (Volume_Growth_Score * 0.3) + (OI_Growth_Score * 0.3)`

- **Volume Growth:** Mức độ đột biến Volume 4H so với trung bình 24H (Tính bằng Z-Score).
- **OI Growth:** Mức độ đột biến Hợp đồng mở 4H so với trung bình 24H. (Lưu ý: Nếu OI giảm - Deleveraging, điểm score này sẽ bị trừ).

## 6. CẤU TRÚC ĐẦU RA YÊU CẦU CHO PHASE 3 (JSON OUTPUT)
Đầu ra của Agent Phase 2 phải là một danh sách các "Ứng viên" (Candidates) đã được xếp hạng, kèm theo Context hiện tại để Phase 3 biết áp dụng chiến thuật vào lệnh (Trigger) nào.

```json
{
  "scan_timestamp": 1716964000,
  "market_context": {
    "action_mode": "Aggressive_Long",
    "market_score": 85
  },
  "shortlist": [
    {
      "symbol": "SOLUSDT",
      "rs_score": 2.45,
      "rs_rating": "A",
      "direction": "LONG",
      "rank_score": 92.5,
      "metrics": {
        "vol_growth_4h_pct": 125.4,
        "oi_growth_4h_pct": 18.2,
        "distance_to_ema50_4h_pct": 3.2
      },
      "reason": "Top RS Leader (A), OI surge, aligned with Aggressive_Long mode."
    },
    {
      "symbol": "INJUSDT",
      "rs_score": 1.85,
      "rs_rating": "A",
      "direction": "LONG",
      "rank_score": 85.0,
      "metrics": {
        "vol_growth_4h_pct": 45.2,
        "oi_growth_4h_pct": 12.1,
        "distance_to_ema50_4h_pct": 1.5
      },
      "reason": "Strong RS, healthy pullback near EMA50 support."
    }
  ]
}
```

## 7. CHU KỲ VẬN HÀNH VÀ LOẠI BỎ (LIFECYCLE & EVICTION POLICY)
- **Global Scan (Quét toàn chợ):** Định kỳ mỗi 1 giờ, hoặc chạy tức thời khi có thông điệp chuyển trạng thái (Phase Change) từ Phase 1 gửi sang.
- **Shortlist Monitor (Theo dõi mục tiêu):** Các symbol nằm trong `shortlist` sẽ được bật stream dữ liệu độ trễ thấp (WebSocket 15m, 1m) để cấp đạn cho Phase 3 tìm điểm vào lệnh.
- **Bơm/Xả Shortlist (Eviction Policy):** Một symbol sẽ bị **đá khỏi Shortlist** ngay lập tức nếu:
  - Bị giáng cấp RS (ví dụ: đang giữ vị thế Leader hạng A tụt xuống hạng C).
  - Gãy cấu trúc thị trường (ví dụ: đang nằm trong danh sách Long nhưng thủng hỗ trợ EMA50 khung 4H kèm Volume xả lớn).