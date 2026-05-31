#[cfg(test)]
mod tests {
    use crate::core::indicators::IndicatorEngine;
    use crate::core::models::Candle;

    #[test]
    fn test_indicator_engine_basics() {
        let mut engine = IndicatorEngine::new();
        
        // Tạo dữ liệu nến giả lập
        let candle = Candle {
            symbol: "BTCUSDT".to_string(),
            timeframe: "1h".to_string(),
            open_time: 1716960000,
            close_time: 1716963600,
            open: 60000.0,
            high: 60500.0,
            low: 59500.0,
            close: 60200.0,
            volume: 100.0,
            is_closed: true,
        };

        // Chạy engine tính toán (Giờ chỉ trả về 4 giá trị)
        let (ema20, ema50, ema200, atr) = engine.next(&candle);

        // Kiểm tra xem các giá trị có được trả về hợp lệ (không phải NaN)
        assert!(ema20.is_finite());
        assert!(ema50.is_finite());
        assert!(ema200.is_finite());
        assert!(atr.is_finite());
    }
}
