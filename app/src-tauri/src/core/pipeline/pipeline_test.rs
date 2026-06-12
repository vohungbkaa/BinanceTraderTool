#[cfg(test)]
mod tests {
    use crate::core::models::NormalizedCandleData;
    use crate::core::websocket::BinanceWsClient;
    use crate::core::events::{MarketEvent, SystemEvent};
    use tokio::sync::{mpsc, broadcast};

    #[tokio::test]
    async fn test_pipeline_output_schema() {
        // 1. Dữ liệu mẫu thô từ Binance
        let raw_json = r#"{
            "e": "kline",
            "s": "BTCUSDT",
            "k": {
                "t": 1716960000,
                "T": 1716963600,
                "s": "BTCUSDT",
                "i": "15m",
                "o": "68000.0",
                "h": "68500.0",
                "l": "67500.0",
                "c": "68200.0",
                "v": "100.5",
                "q": "6800000.0",
                "V": "50.0",
                "x": true
            }
        }"#;

        // 3. Giả lập parse
        let (tx, _) = mpsc::channel(1);
        let (sys_tx, _) = mpsc::channel(1);
        let client = BinanceWsClient::new(tx, sys_tx);
        let v: serde_json::Value = serde_json::from_str(raw_json).unwrap();
        let normalized = client.parse_kline(v).await.unwrap();


        // 3. Assert dữ liệu
        assert_eq!(normalized.candle.symbol, "BTCUSDT");
        assert_eq!(normalized.candle.timeframe, "15m");
        assert_eq!(normalized.candle.close, 68200.0);
        assert_eq!(normalized.candle.is_closed, true);
    }
}
