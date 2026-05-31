# TECHNICAL IMPLEMENTATION BLUEPRINT: PHASE 0 & PHASE 1
*(STRICT DIRECTIVE FOR REAL-MONEY TRADING SYSTEM)*

Tài liệu này là bản vẽ kỹ thuật chi tiết nhất (Developer-Ready) được trích xuất từ Specs. Mọi Logic code phải tuân thủ chính xác các điều kiện dưới đây, không được làm tròn hoặc bỏ sót, vì hệ thống giao dịch tiền thật (Real-money) cực kỳ nhạy cảm với sai số.

---

## PHẦN 1: PHASE 0 - DATA PIPELINE (Chuẩn bị Dữ liệu)

Phase 0 **chỉ theo dõi BTCUSDT** để làm Benchmark, cộng với dữ liệu vĩ mô toàn thị trường. KHÔNG theo dõi các Altcoin lẻ tẻ (trừ khi fetch REST để tính Market Breadth).

### 1. Luồng WebSocket & Sự Kiện Nến
- **Tín hiệu an toàn:** Mọi tính toán kỹ thuật, lưu Database, và trigger Phase 1 **CHỈ ĐƯỢC PHÉP** chạy khi nhận được cờ `k.x == true` (Nến đã đóng hoàn toàn) từ Binance WebSocket. Cờ `k.x == false` chỉ dùng để đẩy lên UI (Live tick).
- **Timeframes (Bắt buộc phải có):** `15m` (cho Risk), `4H` (cho Micro Trend), `1D` (cho Macro Trend).

### 2. Cấu trúc Payload bắt buộc (Gửi sang Phase 1)
Developer phải map chính xác các trường sau vào `NormalizedCandleData` trước khi broadcast:

#### A. Khối `btc_data` (Tính toán cho BTCUSDT)
*Tính bằng `IndicatorEngine` với dữ liệu đã Warm-up đủ 200 nến.*
- **Khung 1D (Vĩ mô):** `close`, `ema200`, `structure` (Phải nhận diện được Fractal Pivot: HH, HL, LH, LL), `close_above_ema200_count` (Đếm số nến 1D liên tiếp đóng trên EMA200).
- **Khung 4H (Vi mô):** `close`, `ema50`, `ema200`, `adx_14`, `plus_di`, `minus_di`, `structure`, `range_24h_pct` (Biên độ % 24h qua), `range_p40_90d` (Percentile 40 của biên độ 24h trong 90 ngày qua lấy từ DB).
- **Khung 15m/1H (Biến động):** `atr_surge_ratio` (Công thức: `ATR hiện tại / Trung bình ATR 14 phiên`).

#### B. Khối `market_indices` (Dòng tiền vĩ mô)
- `btc_d_trend`: Tính từ stream `BTCDOMUSDT`. Cứ > 50 là "UP", ngược lại là "DOWN".
- `market_breadth_pct_above_ema50` & `market_breadth_pct_above_ema200`: Chạy định kỳ, đếm số lượng Top 100 Altcoin (loại BTC/ETH, Vol > 5M) có Close(1D) > EMA.
- `total3_btc_trend`: Dốc lên ("UP") nếu Breadth EMA50 > 50%.

#### C. Khối `microstructure` & `macro_events` (Rủi ro)
- `oi_change_4h_pct`: `(OI_hiện_tại - OI_4h_trước) / OI_4h_trước * 100`.
- `liquidation_surge_detected`: TRUE nếu tổng thanh lý `!forceOrder@arr` > 10,000,000 USD trong thời gian ngắn.
- `funding_rate_avg`: Lấy từ `markPriceUpdate`.
- `macro_events`: Check API lịch kinh tế. Trả về `time_to_event_minutes` và `is_event_block_window` (TRUE nếu nằm trong khoảng `T-60 phút` đến `T+30 phút` của sự kiện High Impact USD).

---

## PHẦN 2: PHASE 1 - REGIME ENGINE (Quyết định Giao dịch)

Phase 1 nhận Object từ Phase 0 và chạy qua 4 bộ lọc nghiêm ngặt (TUYỆT ĐỐI KHÔNG BYPASS).

### BỘ LỌC 1: QUẢN TRỊ RỦI RO (RISK FIRST)
Đánh giá field `risk_status` đầu tiên. Nếu vi phạm, DỪNG MỌI HOẠT ĐỘNG.
1. **Event Risk:** Nếu `is_event_block_window == true` -> Set trạng thái `Event_Block`. Đóng băng hệ thống.
2. **Microstructure Risk:** Nếu `liquidation_surge_detected == true` HOẶC Spread nở rộng bất thường -> Set `Microstructure_Reset`. Chờ thị trường xả xong.
3. **Volatility Risk:** Nếu `atr_surge_ratio > 3.0` (Biến động gấp 3 lần bình thường) -> Set `Volatility_Alert`. Cảnh báo rủi ro trượt giá.
*=> Nếu vượt qua tất cả, set `risk_status = "Normal"`. Tiếp tục Bước 2.*

### BỘ LỌC 2: PHÂN TÍCH XU HƯỚNG ĐA TẦNG
Tính toán `structural_trend` và `operational_state`.

**1. Tầng Vĩ mô (Dựa trên `btc_data.1D`):**
- `Macro_Bullish`: BẮT BUỘC `close > ema200` >= 3 phiên liên tiếp **VÀ** `structure` phải là HH hoặc HL.
- `Macro_Bearish`: BẮT BUỘC `close < ema200` >= 3 phiên liên tiếp **VÀ** `structure` phải là LL hoặc LH.

**2. Tầng Vi mô (Dựa trên `btc_data.4H`):**
- `Active_Bullish`: BẮT BUỘC `close > ema50 > ema200` **VÀ** `structure` (HH/HL) **VÀ** `+DI > -DI` **VÀ** `adx_14 > 25`.
    *(Lưu ý Hysteresis: Nếu đã đạt Active_Bullish, giữ nguyên trạng thái này cho đến khi `adx_14 < 20` mới hủy).*
- `Active_Bearish`: BẮT BUỘC `close < ema50 < ema200` **VÀ** `structure` (LL/LH) **VÀ** `-DI > +DI` **VÀ** `adx_14 > 25`.
- `Dynamic_Sideway`: BẮT BUỘC `range_24h_pct < range_p40_90d` **VÀ** `adx_14 < 20`.
- `Pullback`: Xu hướng 4H ngược chiều 1D, nhưng cấu trúc Pivot 4H chưa bị phá vỡ hoàn toàn (ví dụ giá rớt dưới EMA50 nhưng chưa thủng đáy HL trước đó).

### BỘ LỌC 3: ĐÁNH GIÁ DÒNG TIỀN & ĐỘNG LƯỢNG (SCORING)
Sử dụng công thức `Market_Score = (Trend * 0.3) + (Risk * 0.2) + (Positioning * 0.2) + (Flow * 0.3)`. Hệ điểm quy về thang 100.

**1. Positioning (Khớp Giá và OI 4H):**
- Giá TĂNG + OI TĂNG (>0%) -> Trend Healthy (Được 90/100 điểm Positioning)
- Giá GIẢM + OI TĂNG (>0%) -> Build-up Short (Được 80/100 điểm)
- Giá TĂNG + OI GIẢM (<0%) -> Short Cover (Squeeze) (Được 40/100 điểm)
- Giá GIẢM + OI GIẢM (<0%) -> Deleveraging (Được 30/100 điểm)
- *PENALTY:* Trừ 20 điểm nếu Funding Rate > 0.05% hoặc < -0.05% (dấu hiệu lệnh bị dồn 1 phía quá mức - Crowded Trade).

**2. Flow (Dòng tiền):**
- Nếu `btc_d_trend == "DOWN"` và `market_breadth_pct_above_ema50` ĐANG TĂNG -> Chấm kịch khung điểm Flow.

### BỘ LỌC 4: GATEWAY & ACTION MODE (KẾT LUẬN)
Quyết định `allow_alt_scan` (Mặc định luôn là `false`). Để bật `true`, phải CÙNG LÚC đạt:
1. `risk_status == "Normal"`.
2. `Market_Score >= 40`. *(Lưu ý Hysteresis: Để kích hoạt "Full Size/Aggressive", trạng thái Trend phải giữ ổn định ít nhất 3 cây nến 4H, nếu không chỉ cho đánh Scalp).*
3. `operational_state` KHÔNG phải là `Dynamic_Sideway` (trừ khi Market_Score > 60).
4. Có sự đồng thuận Dòng tiền: (Macro 1D = Bullish) **THÌ** (`btc_d_trend` = DOWN **HOẶC** `total3_btc_trend` = UP).

**Xác định `action_mode` gửi cho Phase 2:**
- Chấm `Aggressive_Long`: Nếu Score > 75 + Macro Bullish + Active Bullish + Flow đồng thuận.
- Chấm `Scalp_Long`: Nếu đang trong trạng thái `Pullback` hoặc Score từ 40-75.
- Chấm `Mean_Reversion`: Nếu `Dynamic_Sideway` nhưng rủi ro Normal.
- Chấm `Off_System`: Nếu ngắt ở Bộ Lọc 1 hoặc Score < 40.

*(Đóng gói toàn bộ thành JSON và Broadcast đi, kết thúc nhiệm vụ của Phase 1).*
