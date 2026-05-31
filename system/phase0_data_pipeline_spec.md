# PHASE 0: DATA PIPELINE & PREPROCESSING - SPECIFICATION

## 1. MỤC ĐÍCH VÀ VAI TRÒ
Phase 0 (Data Pipeline) là nền tảng hạ tầng dữ liệu của toàn bộ hệ thống. Nó có nhiệm vụ thu thập, chuẩn hóa, và tính toán trước (preprocess) dữ liệu từ thị trường (Binance, nguồn tin tức kinh tế, on-chain) để cung cấp đầu vào "sạch", realtime và đầy đủ nhất cho **Phase 1 (Market Regime Engine)**.

## 2. PHƯƠNG THỨC THU THẬP DỮ LIỆU

Để đảm bảo dữ liệu luôn đầy đủ và realtime, Phase 0 sử dụng kết hợp hai phương thức:

### 2.1. WebSocket (Realtime Stream) - Nguồn chính
- **Mục đích:** Thu thập dữ liệu nến (Kline), Ticker, và Funding Rate theo thời gian thực.
- **Tách biệt luồng:** Phân biệt rõ `is_closed: false` (nến đang chạy) để cập nhật UI và `is_closed: true` (nến đã đóng) để kích hoạt logic giao dịch.

### 2.2. REST API (Initialization & Recovery) - Nguồn bổ trợ
- **Khởi tạo (Warm-up):** Sử dụng `GET /fapi/v1/klines` để lấy ít nhất **200 nến lịch sử** cho mỗi Symbol/Timeframe khi ứng dụng vừa khởi động. Điều này đảm bảo các chỉ báo như EMA200 có đủ dữ liệu để tính toán chính xác ngay lập tức.
- **Bù dữ liệu (Gap Filling):** Khi phát hiện mất kết nối WebSocket, hệ thống sẽ tính toán khoảng thời gian bị thiếu và gọi REST API để lấy lại các nến đã đóng trong thời gian đó.
- **Đồng bộ metadata:** Lấy danh sách Symbol hợp lệ và thông số đòn bẩy tối đa định kỳ mỗi 24 giờ.

---

## 3. NHỮNG BỔ SUNG ĐỂ ĐÁP ỨNG ĐẦU VÀO CHO PHASE 1
Dựa trên yêu cầu đánh giá bối cảnh vĩ mô tại Phase 1 (`phase1_market_regime_spec.md`), Phase 0 cần phải thu thập và cung cấp các luồng dữ liệu (Data Streams) đặc thù sau:

### 2.1. Dữ liệu Giá và Chỉ báo kỹ thuật (Technical Data)
- **Timeframes:** Cần tính toán đồng thời trên nhiều khung thời gian: 1D (Vĩ mô), 4H (Vi mô), 15m/1H (Rủi ro biến động).
- **Indicators cần tính sẵn:** Cung cấp sẵn các giá trị EMA50, EMA200, ADX(14), +DI, -DI, ATR(14/15) để Phase 1 không phải tính lại. Nhận diện cấu trúc Pivot (HH, HL, LH, LL).
- **Phân tích phân phối (Percentile):** Tính toán độ rộng biên độ 24h và đối chiếu với Percentile 40 (P40) của 90 ngày để Phase 1 có thể nhận diện trạng thái "Dynamic Sideway".

### 2.2. Dữ liệu Rủi ro Hệ thống và Vi cấu trúc (Microstructure & Risk Data)
- **Lịch kinh tế (Event Risk):** Thu thập thời gian công bố các tin tức quan trọng (FOMC, CPI, NFP). Tính toán thời gian đếm ngược (countdown) đến sự kiện.
- **Thanh lý (Liquidation Cascade):** Theo dõi khối lượng thanh lý đột biến trên thị trường.
- **Biến động bất thường:** Tính toán trung bình ATR(15m/1H) của 14 phiên và cung cấp tỷ lệ so sánh với ATR hiện tại (để Phase 1 kiểm tra ngưỡng rủi ro x3).
- **Thanh khoản:** Đo lường độ giãn Spread của Orderbook, độ lệch Basis giữa Spot và Futures.

### 2.3. Dữ liệu Dòng tiền và Vị thế (Flow & Positioning Data)
- **Open Interest (OI):** Theo dõi sự thay đổi của OI để đánh giá Build-up/Covering.
- **Funding Rate:** Thu thập Funding rate toàn thị trường để phát hiện các bẫy giao dịch đông đúc (Crowded Trade).
- **Dữ liệu Tổng hợp (Macro Indices):** 
  - Xu hướng Tỷ lệ thống trị của BTC (BTC.D).
  - Xu hướng tỷ lệ TOTAL3 / BTC (sức mạnh Altcoin).
  - Market Breadth: Tỷ lệ % Altcoin (Top 100) có giá nằm trên đường EMA50 & EMA200.

---

## 3. MÔ TẢ LUỒNG XỬ LÝ (PHASE 0 -> PHASE 1)

**Khi Phase 0 xuất ra Output, Phase 1 sẽ làm gì với dữ liệu đó?**

1. **Quét Rủi ro ưu tiên (Risk First):** Phase 1 nhận thông tin từ `macro_events`, `liquidation_surge_detected`, `atr_surge_ratio`. Nếu sắp ra tin FOMC, hoặc ATR x3 trung bình, hoặc có thanh lý lớn, Phase 1 lập tức gắn cờ `Event_Block` hoặc `Volatility_Alert` và ngắt hệ thống (không cấp phép cho Phase 2).
2. **Định hình Xu hướng Đa tầng:** 
   - Dùng khối dữ liệu `1D` (Close, EMA200, Structure) để gán nhãn `Macro_Bullish` hay `Macro_Bearish`. Định hình vị thế giao dịch (Bias).
   - Dùng khối dữ liệu `4H` (EMA50, EMA200, ADX, DI) để gán nhãn `Active_Bullish`, `Pullback`, hay `Dynamic_Sideway`. Quyết định xem đây có phải là "thời điểm" tốt để kích hoạt Scanner không.
3. **Phân tích Dòng tiền & Độ rộng:** Nhận `market_breadth_pct`, `btc_d_trend`, `total3_btc_trend` để xác nhận xem Altcoin có thực sự đang hút dòng tiền (Altcoin Season) hay không.
4. **Đánh giá Động lượng (Positioning):** Áp dụng ma trận giá và OI (so sánh `price_change_4h_pct` và `oi_change_4h_pct`) để ra điểm hệ số vị thế (Trend Healthy hay Squeeze/Deleveraging).
5. **Ra quyết định:** Phase 1 tổng hợp tất cả thành `market_score`. Nếu Điểm > 40 và Rủi ro an toàn, Phase 1 xuất tín hiệu `allow_alt_scan = true` để mở cửa cho Phase 2 (Altcoin Scanner).

---

## 4. OUTPUT SCHEMA (ĐẦU VÀO CỦA PHASE 1)
Dưới đây là cấu trúc JSON Data Payload chuẩn mực mà Phase 0 phải cung cấp liên tục (qua Stream hoặc Query) cho Phase 1:

```json
{
  "timestamp": 1716960000,
  "market_indices": {
    "btc_d_trend": "DOWN", 
    "total3_btc_trend": "UP",
    "market_breadth_pct_above_ema50": 65.5,
    "market_breadth_pct_above_ema200": 58.2
  },
  "btc_data": {
    "1D": {
      "close": 68000,
      "ema200": 60000,
      "close_above_ema200_count": 5,
      "structure": "HH_HL"
    },
    "4H": {
      "close": 68500,
      "ema50": 67000,
      "ema200": 64000,
      "adx_14": 28.5,
      "plus_di": 25.1,
      "minus_di": 15.0,
      "structure": "HH_HL",
      "range_24h_pct": 2.5,
      "range_p40_90d": 3.0
    },
    "15m": {
      "atr": 150,
      "atr_avg_14": 120,
      "atr_surge_ratio": 1.25
    }
  },
  "microstructure": {
    "oi_change_4h_pct": 5.2,
    "price_change_4h_pct": 2.1,
    "funding_rate_avg": 0.01,
    "liquidation_surge_detected": false,
    "spread_anomaly": false
  },
  "macro_events": {
    "next_event_name": "FOMC",
    "time_to_event_minutes": 120,
    "is_event_block_window": false
  }
}
```
