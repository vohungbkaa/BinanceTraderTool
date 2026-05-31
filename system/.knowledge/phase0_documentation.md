# Phase 0: Data Pipeline & Preprocessing Documentation

## 1. Modules Summary

### `breadth.rs`
- **Struct:** `BreadthEngine`
- **Function `update_breadth(top_altcoins: &[String]) -> Result<()>`**
    - **Mục đích:** Tính toán tỷ lệ phần trăm các Altcoin nằm trên EMA50 và EMA200 để đo lường độ rộng thị trường (Market Breadth).
    - **Hoạt động:** Sử dụng cơ chế cache trong DB (dữ liệu mới trong 1h) hoặc fetch từ Binance REST API, sau đó tính toán chỉ báo và cập nhật `market_breadth_ema50` và `market_breadth_ema200`.

### `db.rs`
- **Struct:** `Database`
- **Hàm chính:**
    - `new()`: Kết nối SQLite và chạy migrations (tạo bảng `closed_candles`, `symbol_config`).
    - `insert_closed_candle(data: &NormalizedCandleData)`: Lưu nến đã đóng kèm toàn bộ bối cảnh (indicators, risk, market indices) vào DB, hỗ trợ `ON CONFLICT` để update.
    - `get_candles(symbol, timeframe, limit)`: Lấy dữ liệu nến lịch sử từ DB để tính toán chỉ báo.
    - `get_last_update_time(...)`: Kiểm tra thời điểm dữ liệu cuối cùng để quản lý cache.
    - `get_p40_range_90d(...)`: Tính toán biên độ P40 của 90 ngày cho trạng thái Sideway.

### `indicators.rs`
- **Struct:** `SymbolIndicatorState` (tính toán cho 1 cặp symbol/tf), `IndicatorEngine` (quản lý trạng thái của nhiều symbol).
- **Hàm `SymbolIndicatorState::next(&Candle) -> Indicators`**
    - **Mục đích:** Tính toán các chỉ báo kỹ thuật (EMA20, 50, 200, ATR, ADX, DI+, DI-, Structure Pivot HH/HL/LH/LL) cho một nến mới.
    - **Kết quả:** Trả về struct `Indicators` chứa dữ liệu đã tính.

### `metadata.rs`
- **Struct:** `MetadataManager`
- **Hàm `get_top_altcoins() -> Result<Vec<String>>`**
    - **Mục đích:** Lọc và lấy danh sách Top 100 Altcoin chất lượng cao nhất dựa trên Volume 24h (>5M USDT) để làm đầu vào tính Breadth.

### `pipeline.rs`
- **Struct:** `DataPipeline`
- **Chức năng chính:** Điều phối toàn bộ Phase 0.
    - `start()`: Khởi chạy luồng pipeline, sync metadata/breadth, warmup dữ liệu từ DB, chạy các loop định kỳ (News/Risk, Metadata sync) và WS client.
    - `handle_market_event(event: MarketEvent)`: Xử lý các sự kiện từ WebSocket (`CandleClosed`, `CandleUpdated`, `Depth`, `Funding`), tính toán chỉ báo, rủi ro, và phát ra sự kiện tổng hợp (`NormalizedCandleData`) lên `global_event_tx`.
    - `fill_gaps(...)`: Bù dữ liệu nến bị thiếu khi phát hiện mất kết nối.

### `websocket.rs`
- **Struct:** `BinanceWsClient`
- **Chức năng:** Kết nối WebSocket của Binance, parse các tin nhắn kline và các loại event khác, sau đó chuyển đổi thành `NormalizedCandleData` và gửi vào kênh pipeline.

## 2. Mapping Output Phase 0 to Phase 1 Specification

Dưới đây là ánh xạ từ `NormalizedCandleData` (Output Phase 0) sang các trường yêu cầu trong `phase1_market_regime_spec.md`:

| Trường trong Phase 1 Spec | Ánh xạ từ `NormalizedCandleData` (Phase 0) |
| :--- | :--- |
| **btc_data.close** | `candle.close` |
| **btc_data.ema50** | `indicators.ema50` |
| **btc_data.ema200** | `indicators.ema200` |
| **btc_data.adx_14** | `indicators.adx14` |
| **btc_data.structure** | `indicators.structure` |
| **macro_events.is_event_block_window** | `macro_events.is_event_block_window` |
| **microstructure.liquidation_surge_detected** | `microstructure.liquidation_surge_detected` |
| **btc_data.15m.atr_surge_ratio** | `atr_surge_ratio` |
| **market_indices.btc_d_trend** | `market_indices.btc_d_trend` |
| **market_indices.total3_btc_trend** | `market_indices.total3_btc_trend` |
| **market_indices.market_breadth_pct_above_ema50/200** | `market_indices.market_breadth_pct_above_ema50/200` |
