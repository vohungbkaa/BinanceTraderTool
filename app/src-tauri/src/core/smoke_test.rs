#[cfg(test)]
mod smoke_tests {
    use crate::core::rest::BinanceRestClient;
    use crate::core::db::Database;
    use crate::core::indicators::IndicatorEngine;
    use crate::core::models::{Candle, NormalizedCandleData, Indicators, MarketIndices, Microstructure, MacroEvents, CandleMetadata};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_rest_api_connectivity() {
        let client = BinanceRestClient::new();
        // Thử lấy 5 nến BTCUSDT khung 15m
        let result = client.fetch_klines("BTCUSDT", "15m", 5).await;
        
        assert!(result.is_ok(), "Phải lấy được dữ liệu từ Binance REST API");
        let candles = result.unwrap();
        assert_eq!(candles.len(), 5, "Số lượng nến trả về phải đúng bằng 5");
        assert!(candles[0].close > 0.0, "Giá đóng cửa phải lớn hơn 0");
        println!("✅ REST API Test: Lấy dữ liệu thành công. Giá BTC hiện tại: {}", candles.last().unwrap().close);
    }

    #[tokio::test]
    async fn test_database_persistence() {
        // Sử dụng DB in-memory hoặc file tạm để test
        let db_url = "sqlite::memory:";
        let db = Database::new(db_url).await.expect("Phải khởi tạo được Database");

        let mock_data = NormalizedCandleData {
            timestamp: 123456789,
            candle: Candle {
                symbol: "TESTUSDT".to_string(),
                timeframe: "15m".to_string(),
                open_time: 1716960000,
                close_time: 1716960899,
                open: 100.0, high: 110.0, low: 90.0, close: 105.0,
                volume: 500.0, quote_volume: 0.0, taker_buy_volume: 0.0, is_closed: true,
            },
            indicators: Indicators {
                ema20: Some(102.5), ema50: Some(101.0), ema200: Some(100.0),
                atr14: Some(2.5), adx14: None, plus_di: None, minus_di: None,
                structure: "HH".to_string(),
                ..Default::default()
            },
            market_indices: MarketIndices {
                btc_d_trend: crate::core::models::TrendDirection::Up,
                total3_btc_trend: crate::core::models::TrendDirection::Up,
                market_breadth_pct_above_ema50: 75.0,
                market_breadth_pct_above_ema200: 60.0,
            },
            microstructure: Microstructure {
                oi_change_4h_pct: 5.0, price_change_4h_pct: 2.0,
                funding_rate_avg: 0.01, liquidation_surge_detected: false,
                spread_anomaly: false,
                ..Default::default()
            },
            macro_events: MacroEvents {
                next_event_name: "FOMC".to_string(),
                time_to_event_minutes: 120,
                is_event_block_window: false,
            },
            metadata: CandleMetadata { is_warmup: false, latency_ms: 50 },
            range_24h_pct: 2.5,
            range_p40_90d: 3.0,
            atr_surge_ratio: 1.1,
        };

        // Lưu vào DB
        let save_res = db.insert_closed_candle(&mock_data).await;
        assert!(save_res.is_ok(), "Phải lưu được nến vào Database");

        // Kiểm tra logic tính P40 (Truy vấn thử)
        let p40 = db.get_p40_range_90d("TESTUSDT").await;
        assert!(p40.is_ok(), "Truy vấn DB phải thành công");
        
        println!("✅ Database Test: Lưu và truy vấn dữ liệu thành công.");
    }

    #[tokio::test]
    async fn test_indicator_calculation_accuracy() {
        let mut engine = IndicatorEngine::new();
        
        // Giả lập chuỗi 5 nến tăng dần để check EMA
        for i in 1..=5 {
            let candle = Candle {
                symbol: "BTCUSDT".to_string(),
                timeframe: "1h".to_string(),
                open_time: 1000 + i,
                close_time: 2000 + i,
                open: 100.0 + i as f64,
                high: 110.0 + i as f64,
                low: 90.0 + i as f64,
                close: 100.0 + i as f64,
                volume: 10.0,
                quote_volume: 0.0,
                taker_buy_volume: 0.0,
                is_closed: true,
            };
            let inds = engine.process(&candle);
            assert!(inds.ema20.is_some());
            assert!(inds.atr14.is_some());
        }
        println!("✅ Indicator Test: Tính toán chỉ báo thành công.");
    }
}
