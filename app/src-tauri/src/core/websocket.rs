use anyhow::Result;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::time::Duration;
use std::collections::HashSet;

use super::models::{Candle, NormalizedCandleData, Indicators, MarketIndices, Microstructure, MacroEvents, CandleMetadata};
use super::events::MarketEvent;

// [FIX] Sử dụng định dạng /market/stream?streams= theo đúng kiểm chứng thực tế
const BINANCE_WS_BASE_URL: &str = "wss://fstream.binance.com/market/stream?streams=";

pub struct BinanceWsClient {
    symbols: HashSet<String>,
    timeframes: Vec<String>,
    event_tx: mpsc::Sender<MarketEvent>,
}

impl BinanceWsClient {
    pub fn new(
        event_tx: mpsc::Sender<MarketEvent>, 
        _system_tx: mpsc::Sender<crate::core::events::SystemEvent>
    ) -> Self {
        let config_file = std::fs::read_to_string("config.json").unwrap_or_else(|_| "{\"timeframes\": [\"15m\", \"4h\", \"1d\"]}".to_string());
        let config: serde_json::Value = serde_json::from_str(&config_file).unwrap_or_default();
        let timeframes: Vec<String> = config["timeframes"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(|| vec!["15m".to_string(), "4h".to_string(), "1d".to_string()]);

        Self { 
            symbols: HashSet::new(),
            timeframes,
            event_tx 
        }
    }

    pub fn update_symbols(&mut self, symbols: Vec<String>) {
        self.symbols = symbols.into_iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
    }

    fn build_stream_url(&self) -> String {
        let mut streams = Vec::new();
        for symbol in &self.symbols {
            for tf in &self.timeframes {
                streams.push(format!("{}@kline_{}", symbol, tf));
            }
            streams.push(format!("{}@openInterest", symbol));
        }
        
        streams.push("btcdomusdt@markPrice".to_string());
        streams.push("!forceOrder@arr".to_string());
        streams.push("!markPrice@arr".to_string());

        let combined_streams = streams.join("/");
        format!("{}{}", BINANCE_WS_BASE_URL, combined_streams).replace(" ", "")
    }

    pub async fn run(&self) -> Result<()> {
        let url = self.build_stream_url();
        
        println!("\n[WS] ATTEMPTING CONNECTION TO: {}", url);

        loop {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    println!("[WS] SUCCESS: Connected to Binance Market Stream!");
                    let (_, mut read) = ws_stream.split();

                    // Sử dụng timeout để phát hiện mất kết nối (do khóa máy/Sleep)
                    // Binance gửi Ping mỗi 3 phút, dữ liệu BTC cập nhật mỗi giây.
                    // Nếu quá 60s không nhận được bất kỳ packet nào -> Chết connection.
                    loop {
                        match tokio::time::timeout(Duration::from_secs(60), read.next()).await {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                self.handle_message(&text).await;
                            }
                            Ok(Some(Ok(Message::Ping(_)))) | Ok(Some(Ok(Message::Pong(_)))) => {
                                // Tungstenite tự động trả lời Pong, ta chỉ cần nó để reset timeout.
                            }
                            Ok(Some(Err(e))) => {
                                println!("[WS] Read error: {}", e);
                                break;
                            }
                            Ok(None) => {
                                println!("[WS] Stream closed by server.");
                                break;
                            }
                            Err(_) => {
                                println!("[WS] Connection timed out (No data for 60s). Likely OS sleep/Network drop.");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    println!("[WS] Connection failed: {}. Retrying in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            println!("[WS] Reconnecting...");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn handle_message(&self, payload: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(payload) {
            // [QUAN TRỌNG] Combined Streams bọc dữ liệu trong "data"
            let data = v.get("data").unwrap_or(&v);

            if data["e"] == "kline" {
                if let Some(normalized) = self.parse_kline(data.clone()) {
                    // Log nhẹ để xác nhận giá nhảy
                    // println!("[LIVE TICK] {}: {}", normalized.candle.symbol, normalized.candle.close);
                    
                    let event = if normalized.candle.is_closed {
                        MarketEvent::CandleClosed(normalized)
                    } else {
                        MarketEvent::CandleUpdated(normalized)
                    };
                    let _ = self.event_tx.send(event).await;
                }
            }
            else if data["e"] == "openInterestUpdate" {
                let symbol = data["s"].as_str().unwrap_or("").to_string();
                let oi = data["o"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                let _ = self.event_tx.send(MarketEvent::DepthUpdated {
                    symbol, spread_bps: 1.0, liquidity_score: oi, 
                    timestamp: data["E"].as_i64().unwrap_or(0),
                }).await;
            }
            else if data.is_array() {
                if let Some(arr) = data.as_array() {
                    for item in arr {
                        let event_type = item["e"].as_str().unwrap_or("");
                        match event_type {
                            "forceOrder" => {
                                let amount = item["o"]["q"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                let price = item["o"]["p"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                let _ = self.event_tx.send(MarketEvent::DepthUpdated {
                                    symbol: item["o"]["s"].as_str().unwrap_or("").to_string(),
                                    spread_bps: 0.0, liquidity_score: amount * price,
                                    timestamp: item["E"].as_i64().unwrap_or(0),
                                }).await;
                            },
                            "markPriceUpdate" => {
                                let symbol = item["s"].as_str().unwrap_or("").to_string();
                                let funding_rate = item["r"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                let mark_price = item["p"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                if symbol == "BTCDOMUSDT" {
                                    let _ = self.event_tx.send(MarketEvent::FundingUpdated {
                                        symbol, funding_rate: mark_price, 
                                        timestamp: item["E"].as_i64().unwrap_or(0),
                                    }).await;
                                } else {
                                    let _ = self.event_tx.send(MarketEvent::FundingUpdated {
                                        symbol, funding_rate, 
                                        timestamp: item["E"].as_i64().unwrap_or(0),
                                    }).await;
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub fn parse_kline(&self, v: Value) -> Option<NormalizedCandleData> {
        let symbol = v["s"].as_str()?.to_string();
        let k = &v["k"];
        let candle = Candle {
            symbol, timeframe: k["i"].as_str()?.to_string(),
            open_time: k["t"].as_i64()?, close_time: k["T"].as_i64()?,
            open: k["o"].as_str()?.parse().unwrap_or(0.0),
            high: k["h"].as_str()?.parse().unwrap_or(0.0),
            low: k["l"].as_str()?.parse().unwrap_or(0.0),
            close: k["c"].as_str()?.parse().unwrap_or(0.0),
            volume: k["v"].as_str()?.parse().unwrap_or(0.0),
            is_closed: k["x"].as_bool().unwrap_or(false),
        };

        Some(NormalizedCandleData {
            timestamp: chrono::Utc::now().timestamp(),
            candle, ..Default::default()
        })
    }
}
