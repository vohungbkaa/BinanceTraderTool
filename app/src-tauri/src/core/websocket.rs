use anyhow::Result;
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::time::Duration;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::models::{Candle, NormalizedCandleData};
use super::events::MarketEvent;

// Base endpoint cho phép gửi payload SUBSCRIBE (Sử dụng Combined Stream thuộc /market)
const BINANCE_WS_BASE_URL: &str = "wss://fstream.binance.com/market/stream?streams=";

pub struct BinanceWsClient {
    symbols: Arc<RwLock<HashSet<String>>>,
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
            symbols: Arc::new(RwLock::new(HashSet::new())),
            timeframes,
            event_tx 
        }
    }

    /// Cập nhật danh sách symbols mà không cần Mutex bên ngoài
    pub async fn update_symbols(&self, symbols: Vec<String>) {
        let mut syms = self.symbols.write().await;
        *syms = symbols.into_iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
    }

    async fn get_all_streams(&self) -> Vec<String> {
        let mut streams = Vec::new();
        let syms = self.symbols.read().await;
        for symbol in syms.iter() {
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
        let mut prev_stream_count = 0;
        let mut connection_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        let mut cancel_tx = tokio::sync::broadcast::channel::<()>(1).0;

        loop {
            let all_streams = self.get_all_streams().await;
            if all_streams.len() != prev_stream_count && !all_streams.is_empty() {
                tracing::info!("[WS] Stream count changed to {}. Reconnecting all WS...", all_streams.len());
                
                let _ = cancel_tx.send(());
                for handle in connection_handles.drain(..) {
                    let _ = handle.await;
                }
                
                let (new_cancel_tx, _) = tokio::sync::broadcast::channel::<()>(1);
                cancel_tx = new_cancel_tx;
                
                // Binance cho phép 200 streams/connection. Chia thành chunk 150 để an toàn.
                let chunks: Vec<Vec<String>> = all_streams.chunks(150).map(|c| c.to_vec()).collect();
                
                for (idx, chunk) in chunks.into_iter().enumerate() {
                    let mut rx = cancel_tx.subscribe();
                    let tx = self.event_tx.clone();
                    
                    // Endpoint kết hợp: Gắn trực tiếp params vào URL. Binance Futures dùng dấu '/' để phân cách các stream.
                    let stream_params = chunk.join("/");
                    let url = format!("wss://fstream.binance.com/stream?streams={}", stream_params);
                    
                    let handle = tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                _ = rx.recv() => {
                                    break; // Nhận tín hiệu hủy từ vòng lặp chính
                                }
                                _ = async {
                                    match connect_async(&url).await {
                                        Ok((ws_stream, _)) => {
                                            tracing::info!("[WS-{}] Connected to {} streams via URL.", idx, chunk.len());
                                            let (mut write, mut read) = ws_stream.split();
                                            
                                            // Không cần gửi message SUBSCRIBE vì đã khai báo trên URL.

                                            let connection_time = chrono::Utc::now();
                                            loop {
                                                if chrono::Utc::now().signed_duration_since(connection_time).num_hours() >= 23 {
                                                    tracing::info!("[WS-{}] 23h elapsed. Proactively reconnecting...", idx);
                                                    break;
                                                }

                                                match tokio::time::timeout(Duration::from_secs(60), read.next()).await {
                                                    Ok(Some(Ok(Message::Text(text)))) => {
                                                        Self::process_message(text.as_str(), &tx).await;
                                                    }
                                                    Ok(Some(Ok(Message::Ping(ping_data)))) => {
                                                        // Tungstenite split() không tự pong. Phải gửi thủ công.
                                                        if let Err(e) = write.send(Message::Pong(ping_data)).await {
                                                            tracing::error!("[WS-{}] Failed to send Pong: {}", idx, e);
                                                            break;
                                                        }
                                                    }
                                                    Ok(Some(Err(e))) => {
                                                        tracing::error!("[WS-{}] Read error: {}", idx, e);
                                                        break;
                                                    }
                                                    Ok(None) => {
                                                        tracing::warn!("[WS-{}] Stream closed by server.", idx);
                                                        break;
                                                    }
                                                    Err(_) => {
                                                        tracing::warn!("[WS-{}] Connection timed out (60s no data).", idx);
                                                        break;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("[WS-{}] Connection failed: {}", idx, e);
                                            tokio::time::sleep(Duration::from_secs(5)).await;
                                        }
                                    }
                                } => {}
                            }
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    });
                    connection_handles.push(handle);
                }
                
                prev_stream_count = all_streams.len();
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn process_message(payload: &str, tx: &mpsc::Sender<MarketEvent>) {
        if let Ok(v) = serde_json::from_str::<Value>(payload) {
            if v.get("result").is_some() && v.get("id").is_some() {
                tracing::info!("[WS] Received subscription confirmation");
                return;
            }

            // Với endpoint /ws và SUBSCRIBE, message không có "data" wrapper unless we use /stream
            // Nên ta linh hoạt lấy từ "data" hoặc dùng root
            let data = v.get("data").unwrap_or(&v);

            if data["e"] == "kline" {
                if let Some(normalized) = Self::parse_kline_static(data.clone()) {
                    let k = &data["k"];
                    let is_closed = k["x"].as_bool().unwrap_or(false);

                    let event = if is_closed {
                        MarketEvent::CandleClosed(normalized)
                    } else {
                        MarketEvent::CandleUpdated(normalized)
                    };
                    let _ = tx.send(event).await;
                }
            }
            else if data["e"] == "openInterestUpdate" {
                let symbol = data["s"].as_str().unwrap_or("").to_string();
                let oi = data["o"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                let _ = tx.send(MarketEvent::DepthUpdated {
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
                                let _ = tx.send(MarketEvent::DepthUpdated {
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
                                    let _ = tx.send(MarketEvent::FundingUpdated {
                                        symbol, funding_rate: mark_price, 
                                        timestamp: item["E"].as_i64().unwrap_or(0),
                                    }).await;
                                } else {
                                    let _ = tx.send(MarketEvent::FundingUpdated {
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

    pub async fn parse_kline(&self, v: Value) -> Option<NormalizedCandleData> {
        Self::parse_kline_static(v)
    }

    fn parse_kline_static(v: Value) -> Option<NormalizedCandleData> {
        let symbol = v["s"].as_str()?.to_string();
        let k = &v["k"];
        let candle = super::models::Candle {
            symbol, timeframe: k["i"].as_str()?.to_string(),
            open_time: k["t"].as_i64()?, close_time: k["T"].as_i64()?,
            open: k["o"].as_str()?.parse().unwrap_or(0.0),
            high: k["h"].as_str()?.parse().unwrap_or(0.0),
            low: k["l"].as_str()?.parse().unwrap_or(0.0),
            close: k["c"].as_str()?.parse().unwrap_or(0.0),
            volume: k["v"].as_str()?.parse().unwrap_or(0.0),
            quote_volume: k["q"].as_str()?.parse().unwrap_or(0.0),
            taker_buy_volume: k["V"].as_str()?.parse().unwrap_or(0.0),
            is_closed: k["x"].as_bool().unwrap_or(false),
        };

        Some(NormalizedCandleData {
            timestamp: chrono::Utc::now().timestamp(),
            candle, ..Default::default()
        })
    }
}
