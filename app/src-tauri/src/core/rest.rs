use crate::core::models::Candle;
use crate::core::rate_limit::BinanceRateLimiter;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct OrderBookLiquidity {
    pub spread_pct: f64,
    pub depth_50k_slippage_pct: f64,
}

#[derive(Clone)]
pub struct BinanceRestClient {
    client: Client,
    base_url: String,
    rate_limiter: Arc<BinanceRateLimiter>,
}

impl BinanceRestClient {
    /// Client cho live market data (scan, real-time).
    /// Budget cao (65% / 2400wpm), concurrency 8.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://fapi.binance.com".to_string(),
            rate_limiter: Arc::new(BinanceRateLimiter::default()),
        }
    }

    /// Client cho heavy bootstrap operations (warmup, metadata, breadth).
    /// Budget thấp (40% / 2400wpm = 960wpm), concurrency 4 để nhường bandwidth cho live.
    /// Dùng riêng biệt để tránh bootstrap lấn át live market events.
    pub fn new_bootstrap() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://fapi.binance.com".to_string(),
            rate_limiter: Arc::new(BinanceRateLimiter::new(2400, 0.40, 4)),
        }
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        url: String,
        weight: u32,
    ) -> Result<T> {
        let _permit = self.rate_limiter.acquire(endpoint, weight.max(1)).await;
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let used_weight = response
            .headers()
            .get("x-mbx-used-weight-1m")
            .or_else(|| response.headers().get("X-MBX-USED-WEIGHT-1M"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());

        self.rate_limiter.observe_headers(used_weight).await;
        self.rate_limiter.observe_status(status).await;

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("Binance REST {} failed with {}: {}", endpoint, status, body);
        }

        Ok(response.json::<T>().await?)
    }

    /// Lấy danh sách toàn bộ Symbol và thông tin niêm yết
    pub async fn fetch_exchange_info(&self) -> Result<Value> {
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);
        info!("Fetching Binance Exchange Info...");
        self.get_json("exchangeInfo", url, 1).await
    }

    /// Lấy biến động 24h của toàn bộ thị trường để lọc theo Volume
    pub async fn fetch_24h_tickers(&self) -> Result<Vec<Value>> {
        let url = format!("{}/fapi/v1/ticker/24hr", self.base_url);
        self.get_json("ticker24hr_all", url, 40).await
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

        let res: Value = self.get_json("klines", url, kline_weight(limit)).await?;

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
                        taker_buy_volume: candle_arr[9]
                            .as_str()
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0.0),
                        is_closed: true,
                    };
                    candles.push(candle);
                }
            }
        }

        info!(
            "Successfully fetched {} klines for {}",
            candles.len(),
            symbol
        );
        Ok(candles)
    }

    /// Lấy thông tin Funding Rate hiện tại (Premium Index)
    pub async fn fetch_premium_index(&self) -> Result<Vec<Value>> {
        let url = format!("{}/fapi/v1/premiumIndex", self.base_url);
        info!("Fetching Premium Index (Funding Rates)...");
        self.get_json("premiumIndex_all", url, 10).await
    }

    /// Lấy thông tin Open Interest của một symbol
    pub async fn fetch_open_interest(&self, symbol: &str) -> Result<Value> {
        let url = format!("{}/fapi/v1/openInterest?symbol={}", self.base_url, symbol);
        self.get_json("openInterest", url, 1).await
    }

    /// Đo thanh khoản thực dụng từ orderbook: spread và slippage ước tính cho lệnh market 50k USDT.
    pub async fn fetch_order_book_liquidity(
        &self,
        symbol: &str,
        notional_usdt: f64,
    ) -> Result<OrderBookLiquidity> {
        let url = format!(
            "{}/fapi/v1/depth?symbol={}&limit=100",
            self.base_url, symbol
        );
        let data: Value = self.get_json("depth", url, 5).await?;
        let bids = data["bids"].as_array().context("Missing bids")?;
        let asks = data["asks"].as_array().context("Missing asks")?;

        let best_bid = bids
            .first()
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let best_ask = asks
            .first()
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if best_bid <= 0.0 || best_ask <= 0.0 {
            bail!("Invalid top of book for {}", symbol);
        }

        let mid = (best_bid + best_ask) / 2.0;
        let spread_pct = (best_ask - best_bid) / mid * 100.0;
        let buy_slippage = Self::simulate_market_slippage(asks, notional_usdt, mid);
        let sell_slippage = Self::simulate_market_slippage(bids, notional_usdt, mid);

        Ok(OrderBookLiquidity {
            spread_pct,
            depth_50k_slippage_pct: buy_slippage.max(sell_slippage),
        })
    }

    fn simulate_market_slippage(levels: &[Value], notional_usdt: f64, mid: f64) -> f64 {
        if mid <= 0.0 || notional_usdt <= 0.0 {
            return f64::INFINITY;
        }

        let mut remaining = notional_usdt;
        let mut filled_qty = 0.0;
        let mut spent = 0.0;

        for level in levels {
            let price = level
                .get(0)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let qty = level
                .get(1)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            if price <= 0.0 || qty <= 0.0 {
                continue;
            }

            let level_notional = price * qty;
            let fill_notional = remaining.min(level_notional);
            let fill_qty = fill_notional / price;
            filled_qty += fill_qty;
            spent += fill_notional;
            remaining -= fill_notional;

            if remaining <= 0.0 {
                break;
            }
        }

        if remaining > 0.0 || filled_qty <= 0.0 {
            return f64::INFINITY;
        }

        let avg_price = spent / filled_qty;
        ((avg_price - mid).abs() / mid) * 100.0
    }

    pub async fn fetch_order_book_liquidity_bulk<F>(
        &self,
        symbols: &[String],
        on_progress: F,
    ) -> Result<std::collections::HashMap<String, OrderBookLiquidity>>
    where
        F: Fn(f64, String) + Send + Sync + 'static,
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let total = symbols.len().max(1);
        let completed = Arc::new(AtomicUsize::new(0));
        let on_progress = Arc::new(on_progress);
        let semaphore = Arc::new(Semaphore::new(8));
        let mut tasks = Vec::new();

        for symbol in symbols {
            let sym = symbol.clone();
            let client = self.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let completed = completed.clone();
            let on_progress = on_progress.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let result = match client.fetch_order_book_liquidity(&sym, 50_000.0).await {
                    Ok(liquidity) => Some((sym.clone(), liquidity)),
                    Err(e) => {
                        tracing::warn!("Failed to fetch orderbook liquidity for {}: {}", sym, e);
                        None
                    }
                };

                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(
                    (done as f64 / total as f64) * 100.0,
                    format!("Checking orderbook liquidity: {}/{}", done, total),
                );
                result
            }));
        }

        let mut results = std::collections::HashMap::new();
        for task in futures_util::future::join_all(tasks).await {
            if let Ok(Some((sym, liquidity))) = task {
                results.insert(sym, liquidity);
            }
        }

        Ok(results)
    }

    /// Lấy Open Interest cho nhiều symbols đồng thời với giới hạn concurrency
    pub async fn fetch_open_interest_bulk<F>(
        &self,
        symbols: &[String],
        on_progress: F,
    ) -> Result<std::collections::HashMap<String, f64>>
    where
        F: Fn(f64, String) + Send + Sync + 'static,
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::sync::Semaphore;

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
                        let oi = val["openInterest"]
                            .as_str()
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        Some((sym.clone(), oi))
                    }
                    Err(_) => None,
                };

                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(
                    (done as f64 / total as f64) * 100.0,
                    format!("Fetching OI: {}/{}", done, total),
                );
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
    pub async fn fetch_oi_hist_24h_bulk<F>(
        &self,
        symbols: &[String],
        on_progress: F,
    ) -> Result<std::collections::HashMap<String, f64>>
    where
        F: Fn(f64, String) + Send + Sync + 'static,
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::sync::Semaphore;

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
                let url = format!(
                    "{}/futures/data/openInterestHist?symbol={}&period=5m&limit=1&endTime={}",
                    client.base_url, sym, end_time
                );

                let result = match client
                    .get_json::<Vec<Value>>("openInterestHist", url, 1)
                    .await
                {
                    Ok(data) => {
                        if let Some(first) = data.first() {
                            let oi = first["sumOpenInterest"]
                                .as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            Some((sym.clone(), oi))
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                };

                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(
                    (done as f64 / total as f64) * 100.0,
                    format!("Fetching Hist OI: {}/{}", done, total),
                );
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
    pub async fn fetch_klines_bulk<F>(
        &self,
        symbols: &[String],
        interval: &str,
        limit: u32,
        on_progress: F,
    ) -> Result<std::collections::HashMap<String, Vec<Candle>>>
    where
        F: Fn(f64, String) + Send + Sync + 'static,
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::sync::Semaphore;

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
                on_progress(
                    (done as f64 / total as f64) * 100.0,
                    format!("Fetching ATR: {}/{}", done, total),
                );
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

fn kline_weight(limit: u32) -> u32 {
    match limit {
        0..=99 => 1,
        100..=499 => 2,
        500..=1000 => 5,
        _ => 10,
    }
}
