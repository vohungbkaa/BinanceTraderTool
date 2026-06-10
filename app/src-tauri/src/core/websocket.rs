use anyhow::Result;
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::time::Duration;
use std::collections::HashSet;

use super::models::{Candle, NormalizedCandleData, Indicators, MarketIndices, Microstructure, MacroEvents, CandleMetadata};
use super::events::MarketEvent;

// Base endpoint cho phép gửi payload SUBSCRIBE (Sử dụng Combined Stream thuộc /market)
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
        let config = crate::core::config::AppConfig::load();
        let timeframes = config.timeframes;

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

    fn get_all_streams(&self) -> Vec<String> {
        let mut streams = Vec::new();
        for symbol in &self.symbols {
            let sym_lower = symbol.to_lowercase();
            for tf in &self.timeframes {
                streams.push(format!("{}@kline_{}", sym_lower, tf));
            }
        }
        
        streams.push("btcdomusdt@markPrice".to_string());
        streams.push("!forceOrder@arr".to_string());
        streams.push("!markPrice@arr".to_string());

        streams
    }

    pub async fn run(&self) -> Result<()> {
        let all_streams = self.get_all_streams();
        
        // Mở kết nối với 1 stream mồi để Binance xác nhận đây là kết nối hợp lệ thuộc nhánh /market
        let initial_stream = "btcusdt@markPrice"; 
        let url = format!("{}{}", BINANCE_WS_BASE_URL, initial_stream);
        
        tracing::info!("[WS] ATTEMPTING CONNECTION TO BASE URL: {}", url);

        loop {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    tracing::info!("[WS] SUCCESS: Connected to Binance Market Stream!");
                    let (mut write, mut read) = ws_stream.split();

                    // Gửi payload SUBSCRIBE với danh sách stream đã chia nhỏ để tránh Limit của Binance
                    let all_streams = self.get_all_streams();
                    tracing::info!("[WS] Registering {} streams dynamically...", all_streams.len());

                    let mut req_id = 1;
                    for chunk in all_streams.chunks(200) {
                        let subscribe_msg = serde_json::json!({
                            "method": "SUBSCRIBE",
                            "params": chunk,
                            "id": req_id
                        });
                        
                        if let Err(e) = write.send(Message::Text(subscribe_msg.to_string().into())).await {
                            tracing::error!("[WS] Failed to send SUBSCRIBE payload: {}", e);
                        }
                        req_id += 1;
                        tokio::time::sleep(Duration::from_millis(500)).await; // Tránh rate limit của WebSocket
                    }

                    // 1. Giới hạn 24 giờ của Binance
                    let connection_time = chrono::Utc::now();
                    
                    loop {
                        // Restart kết nối trước khi chạm ngưỡng 24h (ở đây an toàn lấy 23.5h)
                        if chrono::Utc::now().signed_duration_since(connection_time).num_hours() >= 23 {
                            tracing::info!("[WS] 23 hours elapsed. Proactively reconnecting to avoid Binance 24h force-drop...");
                            break;
                        }

                        match tokio::time::timeout(Duration::from_secs(60), read.next()).await {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                self.handle_message(text.as_str()).await;
                            }
                            Ok(Some(Ok(Message::Ping(ping_data)))) => {
                                // Mặc dù tungstenite tự phản hồi Pong, nhưng ta explicit send Pong để debug và chắc chắn 100%
                                tracing::debug!("[WS] Received Ping from Binance, sending explicit Pong.");
                                if let Err(e) = write.send(Message::Pong(ping_data)).await {
                                    tracing::error!("[WS] Failed to send Pong: {}", e);
                                }
                            }
                            Ok(Some(Ok(Message::Pong(_)))) => {
                                // Bỏ qua
                            }
                            Ok(Some(Err(e))) => {
                                tracing::error!("[WS] Read error: {}", e);
                                break;
                            }
                            Ok(None) => {
                                tracing::warn!("[WS] Stream closed by server.");
                                break;
                            }
                            Err(_) => {
                                tracing::warn!("[WS] Connection timed out (No data for 60s). Likely OS sleep/Network drop.");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[WS] Connection failed: {}. Retrying in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            tracing::info!("[WS] Reconnecting...");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn handle_message(&self, payload: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(payload) {
            // Log response from SUBSCRIBE
            if v.get("result").is_some() && v.get("id").is_some() {
                tracing::info!("[WS] Received subscription confirmation: {}", payload);
                return;
            }

            // [QUAN TRỌNG] Combined Streams bọc dữ liệu trong "data"
            let data = v.get("data").unwrap_or(&v);

            if data["e"] == "kline" {
                if let Some(normalized) = self.parse_kline(data.clone()) {
                    let k = &data["k"];
                    let is_closed = k["x"].as_bool().unwrap_or(false);

                    // Log nhẹ để xác nhận giá nhảy
                    if is_closed {
                         tracing::info!("[WS KLINE CLOSED] {} | {} | Price: {} | Vol: {}", 
                            normalized.candle.symbol, 
                            normalized.candle.timeframe, 
                            normalized.candle.close,
                            normalized.candle.volume
                        );
                    } else {
                        // Bỏ comment dòng dưới nếu muốn thấy live tick từng giây
                        // println!("[WS KLINE UPDATE] {} | {} | Price: {}", normalized.candle.symbol, normalized.candle.timeframe, normalized.candle.close);
                    }
                    
                    let event = if is_closed {
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
                    symbol, is_liquidation: false, price: 0.0, value_usd: oi, 
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
                                    is_liquidation: true, price, value_usd: amount * price,
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
            taker_buy_volume: k["V"].as_str()?.parse().unwrap_or(0.0),
            is_closed: k["x"].as_bool().unwrap_or(false),
        };

        Some(NormalizedCandleData {
            timestamp: chrono::Utc::now().timestamp(),
            candle, ..Default::default()
        })
    }
}
