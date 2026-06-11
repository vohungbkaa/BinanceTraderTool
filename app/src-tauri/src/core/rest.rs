use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use crate::core::models::Candle;
use tracing::info;

#[derive(Clone)]
pub struct BinanceRestClient {
    client: Client,
    base_url: String,
}

impl BinanceRestClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://fapi.binance.com".to_string(),
        }
    }

    /// Lấy danh sách toàn bộ Symbol và thông tin niêm yết
    pub async fn fetch_exchange_info(&self) -> Result<Value> {
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);
        info!("Fetching Binance Exchange Info...");
        let res = self.client.get(&url).send().await?.json::<Value>().await?;
        Ok(res)
    }

    /// Lấy biến động 24h của toàn bộ thị trường để lọc theo Volume
    pub async fn fetch_24h_tickers(&self) -> Result<Vec<Value>> {
        let url = format!("{}/fapi/v1/ticker/24hr", self.base_url);
        let res = self.client.get(&url).send().await?.json::<Vec<Value>>().await?;
        Ok(res)
    }

    /// Lấy nến lịch sử để warm-up các chỉ báo
    pub async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
    ) -> Result<Vec<Candle>> {
        // Build URL manually to avoid the .query() method issue for now
        let url = format!(
            "{}/fapi/v1/klines?symbol={}&interval={}&limit={}", 
            self.base_url, symbol, interval, limit
        );
        
        info!("Fetching historical klines: {}", url);

        let res: Value = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let mut candles = Vec::new();

        if let Some(arr) = res.as_array() {
            for item in arr {
                if let Some(candle_arr) = item.as_array() {
                    let candle = Candle {
                        symbol: symbol.to_string(),
                        timeframe: interval.to_string(),
                        open_time: candle_arr[0].as_i64().context("Invalid open_time")?,
                        close_time: candle_arr[6].as_i64().context("Invalid close_time")?,
                        open: candle_arr[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        high: candle_arr[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        low: candle_arr[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        close: candle_arr[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        volume: candle_arr[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        quote_volume: candle_arr[7].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        taker_buy_volume: candle_arr[9].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        is_closed: true,
                    };
                    candles.push(candle);
                }
            }
        }

        info!("Successfully fetched {} klines for {}", candles.len(), symbol);
        Ok(candles)
    }

    /// Lấy thông tin Funding Rate hiện tại (Premium Index)
    pub async fn fetch_premium_index(&self) -> Result<Vec<Value>> {
        let url = format!("{}/fapi/v1/premiumIndex", self.base_url);
        info!("Fetching Premium Index (Funding Rates)...");
        let res = self.client.get(&url).send().await?.json::<Vec<Value>>().await?;
        Ok(res)
    }

    /// Lấy thông tin Open Interest của một symbol
    pub async fn fetch_open_interest(&self, symbol: &str) -> Result<Value> {
        let url = format!("{}/fapi/v1/openInterest?symbol={}", self.base_url, symbol);
        let res = self.client.get(&url).send().await?.json::<Value>().await?;
        Ok(res)
    }

    /// Lấy Open Interest cho nhiều symbols đồng thời với giới hạn concurrency
    pub async fn fetch_open_interest_bulk<F>(&self, symbols: &[String], on_progress: F) -> Result<std::collections::HashMap<String, f64>> 
    where F: Fn(f64, String) + Send + Sync + 'static {
        use std::sync::Arc;
        use tokio::sync::Semaphore;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let total = symbols.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let on_progress = Arc::new(on_progress);
        let semaphore = Arc::new(Semaphore::new(20)); // Max 20 concurrent requests
        let mut tasks = Vec::new();

        for symbol in symbols {
            let sym = symbol.clone();
            let client = self.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let completed = completed.clone();
            let on_progress = on_progress.clone();
            
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let result = match client.fetch_open_interest(&sym).await {
                    Ok(val) => {
                        let oi = val["openInterest"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        Some((sym.clone(), oi))
                    }
                    Err(_) => None,
                };
                
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress((done as f64 / total as f64) * 100.0, format!("Fetching OI: {}/{}", done, total));
                result
            }));
        }

        let mut results = std::collections::HashMap::new();
        for task in futures_util::future::join_all(tasks).await {
            if let Ok(Some((sym, oi))) = task {
                results.insert(sym, oi);
            }
        }

        Ok(results)
    }

    /// Lấy Open Interest 24h trước cho nhiều symbols đồng thời
    pub async fn fetch_oi_hist_24h_bulk<F>(&self, symbols: &[String], on_progress: F) -> Result<std::collections::HashMap<String, f64>> 
    where F: Fn(f64, String) + Send + Sync + 'static {
        use std::sync::Arc;
        use tokio::sync::Semaphore;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let total = symbols.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let on_progress = Arc::new(on_progress);
        let semaphore = Arc::new(Semaphore::new(10)); // Giới hạn 10 requests concurrent
        let mut tasks = Vec::new();
        let end_time = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;

        for symbol in symbols {
            let sym = symbol.clone();
            let client = self.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let completed = completed.clone();
            let on_progress = on_progress.clone();
            
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let url = format!("{}/futures/data/openInterestHist?symbol={}&period=5m&limit=1&endTime={}", client.base_url, sym, end_time);
                
                let result = match client.client.get(&url).send().await {
                    Ok(res) => {
                        if let Ok(data) = res.json::<Vec<Value>>().await {
                            if let Some(first) = data.first() {
                                let oi = first["sumOpenInterest"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                Some((sym.clone(), oi))
                            } else { None }
                        } else { None }
                    }
                    Err(_) => None,
                };

                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress((done as f64 / total as f64) * 100.0, format!("Fetching Hist OI: {}/{}", done, total));
                result
            }));
        }

        let mut results = std::collections::HashMap::new();
        for task in futures_util::future::join_all(tasks).await {
            if let Ok(Some((sym, oi))) = task {
                results.insert(sym, oi);
            }
        }

        Ok(results)
    }

    /// Lấy nến lịch sử cho nhiều symbols đồng thời (Dùng để tính ATR chuẩn)
    pub async fn fetch_klines_bulk<F>(&self, symbols: &[String], interval: &str, limit: u32, on_progress: F) -> Result<std::collections::HashMap<String, Vec<Candle>>> 
    where F: Fn(f64, String) + Send + Sync + 'static {
        use std::sync::Arc;
        use tokio::sync::Semaphore;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let total = symbols.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let on_progress = Arc::new(on_progress);
        let semaphore = Arc::new(Semaphore::new(10)); 
        let mut tasks = Vec::new();

        for symbol in symbols {
            let sym = symbol.clone();
            let interval_clone = interval.to_string();
            let client = self.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let completed = completed.clone();
            let on_progress = on_progress.clone();
            
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let result = match client.fetch_klines(&sym, &interval_clone, limit).await {
                    Ok(candles) => Some((sym.clone(), candles)),
                    Err(e) => {
                        tracing::warn!("Failed to fetch klines for {}: {}", sym, e);
                        None
                    }
                };

                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress((done as f64 / total as f64) * 100.0, format!("Fetching ATR: {}/{}", done, total));
                result
            }));
        }

        let mut results = std::collections::HashMap::new();
        for task in futures_util::future::join_all(tasks).await {
            if let Ok(Some((sym, candles))) = task {
                results.insert(sym, candles);
            }
        }

        Ok(results)
    }
}
