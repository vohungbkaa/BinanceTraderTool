# Xác nhận điểm vào lệnh (Entry Trigger) dựa trên cấu trúc Vi mô và Động lượng ngắn hạn.

## 1. MỤC ĐÍCH CỦA MODULE (AGENT DIRECTIVE)
Là Agent thực thi Phase 3 (Bắn tỉa), nhiệm vụ của bạn là tiếp nhận `Shortlist` và `market_context` (Bối cảnh thị trường) từ Phase 2. Thay vì vào lệnh ngay lập tức (Market Order) dẫn đến tỷ lệ Risk/Reward (R:R) xấu, bạn phải kiên nhẫn theo dõi các Altcoin trong Shortlist trên các khung thời gian nhỏ (LTF - 15m, 1H) để tìm điểm vào lệnh tối ưu nhất (Pullback & Sweeps). 

Mục tiêu cốt lõi: **Không bao giờ FOMO đuổi giá. Chỉ bóp cò khi giá chiết khấu về vùng thanh khoản và có sự xác nhận đảo chiều cấu trúc vi mô.**

## 2. DỮ LIỆU ĐẦU VÀO (INPUT INGESTION)
Bạn sẽ nhận được một đối tượng JSON từ Phase 2 có cấu trúc như sau. Bạn PHẢI đọc tham số `action_mode` để áp dụng chiến thuật giao dịch tương ứng.

```json
{
  "market_context": {
    "action_mode": "Aggressive_Long"
  },
  "shortlist": [
    {
      "symbol": "SOLUSDT",
      "rs_rating": "A",
      "direction": "LONG",
      "rank_score": 92.5
    }
  ]
}
```

## 3. KHU VỰC CHỜ ĐỢI (PULLBACK ZONES - POI)
Với mỗi đồng coin trong Shortlist, Agent phải xác định các Điểm Quan Tâm (Point of Interest - POI) trên khung **1H** hoặc **4H** làm khu vực "giăng bẫy". Tuyệt đối không vào lệnh lơ lửng giữa không trung.

- **Vùng Động (Dynamic Zones):** Đường EMA20 hoặc EMA50 khung 1H/4H.
- **Khoảng Trống Thanh Khoản (Fair Value Gap - FVG):** Các vùng nến mất cân bằng (Imbalance) chưa được lấp đầy trên khung 1H/4H. Giá thường có xu hướng hút về đây để cân bằng trước khi đi tiếp.
- **Order Block (Khối Lệnh):** Cụm nến đi ngang trước khi xảy ra một cú bứt phá mạnh (Breakout/Breakdown) trên khung 1H/4H.

*Logic Kích Hoạt (Arming):* Khi giá chạm vào vùng biên (Tolerance = 0.5%) của một trong các POI trên, Agent chuyển trạng thái của đồng coin đó từ `Monitoring` sang `Armed` (Sẵn sàng bóp cò).

## 4. BỘ LỌC XÁC NHẬN TÍN HIỆU KHUNG NHỎ (LTF CONFIRMATION)
Khi giá đã vào vùng POI (trạng thái `Armed`), Agent bật kính lúp sang khung **15M** hoặc **5M** để tìm kiếm sự xác nhận của Dòng Tiền Thông Minh (Smart Money) trước khi vào lệnh. Yêu cầu **BẮT BUỘC CÓ 1 TRONG 2** tín hiệu sau:

### A. Quét Thanh Khoản (Liquidity Sweep / Stop Hunt)
- **Setup Long:** Giá nhúng nhanh (Spike) xuyên thủng một Đáy Cũ (Swing Low gần nhất) để quét Stoploss của phe mua non tay. Ngay sau đó rút chân mạnh và đóng nến trên mức Đáy Cũ.
- **Setup Short:** Giá phóng nhanh vượt qua một Đỉnh Cũ (Swing High gần nhất), sau đó rút râu/rớt mạnh đóng nến dưới Đỉnh Cũ.

### B. Đảo Chiều Cấu Trúc Bền Vững (Break of Structure - BOS)
- **Setup Long:** Sau khi tạo đáy tại vùng POI, giá vòng lên và phá vỡ (đóng nến trên) Đỉnh Dẫn Đến Đáy Thấp Nhất (Lower High) ở khung 15M. Đi kèm Khối lượng giao dịch (Volume) tăng vọt.
- **Setup Short:** Giá phá vỡ Đáy Dẫn Đến Đỉnh Cao Nhất (Higher Low) ở khung 15M kèm Volume xả lớn.

## 5. MA TRẬN ĐIỀU CHỈNH CHIẾN THUẬT THEO `ACTION_MODE`
Tùy thuộc vào chỉ thị `action_mode` truyền từ Phase 1 và Phase 2, Agent Phase 3 phải điều chỉnh độ nhạy (Sensitivity) của Trigger:

| Market Context (`action_mode`) | Điều chỉnh Chiến thuật Entry tại Phase 3 |
| :--- | :--- |
| **`Aggressive_Long` / `Aggressive_Short`** | **Nhanh nhẹn:** Không cần chờ BOS. Chỉ cần giá chạm EMA20/EMA50 và xuất hiện nến Pinbar (rút râu) hoặc Engulfing (nhấn chìm) trên khung 15M là có thể bóp cò ngay. |
| **`Scalp_Long` / `Scalp_Short`** | **Khắt khe:** Bắt buộc phải có FVG kết hợp Liquidity Sweep trên khung 5M. Đánh nhanh, Stoploss cực chặt sát ngay râu nến quét. |
| **`Mean_Reversion`** | **Bắt dao rơi:** Yêu cầu Hội tụ (Confluence). Phải có Phân kỳ (Divergence) RSI/MACD trên khung 15m kết hợp với Break of Structure (BOS). Cấm bắt đáy/đỉnh mù quáng. |

## 6. LỌC THỜI GIAN GIAO DỊCH (SESSION TIMING)
Agent cấp thêm điểm (+Bonus Point) cho các tín hiệu Trigger xuất hiện trong các khung giờ thanh khoản cao (Giờ Việt Nam - UTC+7):
- **Phiên Âu (London Open):** 14:00 - 16:00
- **Phiên Mỹ (New York Open):** 19:30 - 22:00
- *Tránh giao dịch (Penalty):* Cuối tuần (Thứ 7 - CN) hoặc lúc giao phiên Á thanh khoản mỏng (05:00 - 07:00 sáng).

## 7. CẤU TRÚC ĐẦU RA YÊU CẦU CHO PHASE 4 (JSON OUTPUT)
Nếu một Altcoin thỏa mãn đầy đủ các điều kiện Entry, Agent xuất ra một "Bản án" (Execution Ticket) chuyển sang Phase 4 (Quản trị Rủi ro & Đi lệnh).

```json
{
  "trigger_timestamp": 1716968500,
  "market_context": "Aggressive_Long",
  "symbol": "SOLUSDT",
  "direction": "LONG",
  "entry_data": {
    "entry_price": 168.50,
    "poi_type": "EMA50_1H",
    "confirmation_type": "Liquidity_Sweep_15m",
    "stoploss_technical": 165.20,
    "confluence_score": 9.5
  },
  "justification": "Price pulled back to 1H EMA50, swept Asian session lows, closed with 15M Bullish Engulfing and Volume spike."
}
```

### A. Chi tiết các trường cốt lõi:
- `entry_price`: Mức giá kích hoạt (Market/Limit tuỳ cấu hình).
- `stoploss_technical`: Điểm Stoploss Kỹ Thuật (dưới đáy râu nến quét/dưới vùng POI). Đây là cơ sở sống còn để Phase 4 tính toán Khối lượng vào lệnh (Position Size). Tuyệt đối không được bỏ trống.
- `confluence_score`: Điểm hội tụ (0-10). Càng nhiều tín hiệu (Chạm EMA + Quét thanh khoản + Đúng phiên Mỹ) thì điểm càng cao. Dùng để Phase 4 quyết định đánh Full Risk hay Half Risk.