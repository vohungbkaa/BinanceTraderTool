#[cfg(test)]
mod integration_tests {
    use crate::core::pipeline::DataPipeline;
    use crate::core::db::Database;
    use crate::core::events::MarketEvent;
    use crate::core::models::{Candle, NormalizedCandleData};
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tracing::info;

    #[tokio::test]
    async fn test_full_phase0_pipeline_flow() {
        // 1. Setup Infrastructure
        let db_url = "sqlite::memory:";
        let db = Arc::new(Database::new(db_url).await.unwrap());
        let (global_tx, mut global_rx) = broadcast::channel::<MarketEvent>(100);
        
        // 2. Initialize Pipeline
        let symbols = vec!["BTCUSDT".to_string()];
        let mut pipeline = DataPipeline::new(symbols, db.clone(), global_tx.clone());

        // 3. Simulate a sequence of candles to trigger indicators and structure
        // We'll feed 10 candles to ensure indicators have some data
        for i in 0..10 {
            let candle = Candle {
                symbol: "BTCUSDT".to_string(),
                timeframe: "15m".to_string(),
                open_time: 1716960000 + (i * 900000),
                close_time: 1716960899 + (i * 900000),
                open: 60000.0 + (i as f64 * 100.0),
                high: 60500.0 + (i as f64 * 100.0),
                low: 59500.0 + (i as f64 * 100.0),
                close: 60200.0 + (i as f64 * 100.0),
                volume: 100.0,
                is_closed: true,
            };

            // Wrap in NormalizedCandleData as if it came from WebSocket.parse_kline
            let data = NormalizedCandleData {
                timestamp: chrono::Utc::now().timestamp(),
                candle,
                indicators: Default::default(), // Will be filled by pipeline
                market_indices: Default::default(),
                microstructure: Default::default(),
                macro_events: Default::default(),
                metadata: Default::default(),
                range_24h_pct: 0.0,
                range_p40_90d: 0.0,
                atr_surge_ratio: 1.0,
            };

            // Directly call the handler to simulate receiving from WebSocket channel
            pipeline.handle_market_event(MarketEvent::CandleClosed(data)).await;
        }

        // 4. Capture the last broadcasted event
        let mut last_output: Option<NormalizedCandleData> = None;
        while let Ok(event) = global_rx.try_recv() {
            if let MarketEvent::CandleClosed(data) = event {
                last_output = Some(data);
            }
        }

        // 5. VERIFY OUTPUT AGAINST SPEC
        assert!(last_output.is_some(), "Pipeline phải phát ra sự kiện CandleClosed");
        let output = last_output.unwrap();

        println!("\n--- PHASE 0 FINAL OUTPUT INSPECTION ---");
        println!("Symbol: {}", output.candle.symbol);
        println!("Close Price: {}", output.candle.close);
        
        // Check Indicators [SPEC 2.1]
        println!("Indicators:");
        println!("  EMA50: {:?}", output.indicators.ema50);
        println!("  EMA200: {:?}", output.indicators.ema200);
        println!("  Pivot Structure: {}", output.indicators.structure);
        assert!(output.indicators.ema50.is_some());
        
        // Check Risk & Microstructure [SPEC 2.2]
        println!("Microstructure:");
        println!("  OI Change %: {}", output.microstructure.oi_change_4h_pct);
        println!("  Liquidation Alert: {}", output.microstructure.liquidation_surge_detected);
        
        // Check News [SPEC 2.2]
        println!("Macro Events:");
        println!("  Next Event: {}", output.macro_events.next_event_name);
        println!("  Block Window: {}", output.macro_events.is_event_block_window);

        // Check Indices [SPEC 2.3]
        println!("Market Indices:");
        println!("  BTC.D Trend: {}", output.market_indices.btc_d_trend);
        println!("  Market Breadth (EMA50): {}%", output.market_indices.market_breadth_pct_above_ema50);

        println!("--- END OF INSPECTION ---\n");

        // Final Assertion: Ensure essential fields for Phase 1 are present
        assert!(!output.indicators.structure.is_empty(), "Phải có dữ liệu cấu trúc đỉnh đáy");
        assert!(output.timestamp > 0, "Phải có timestamp");
    }
}
