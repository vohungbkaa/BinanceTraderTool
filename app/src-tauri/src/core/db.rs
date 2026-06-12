use anyhow::{Result, Context};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use crate::core::models::{Candle, NormalizedCandleData};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(db_url)
            .await
            .context("Failed to connect to SQLite")?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<()> {
        // Bảng lưu thông tin Altcoin cơ bản
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS altcoins (
                symbol TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                listed_at INTEGER NOT NULL,
                volume_24h REAL NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )
        .execute(&self.pool)
        .await?;

        // Bảng lưu nến đã đóng và toàn bộ bối cảnh (Confirmed Candles + Context)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS closed_candles (
                symbol TEXT NOT NULL,
                timeframe TEXT NOT NULL,
                open_time INTEGER NOT NULL,
                close_time INTEGER NOT NULL,
                open_time_str TEXT,
                close_time_str TEXT,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL,
                ema20 REAL,
                ema50 REAL,
                ema200 REAL,
                atr14 REAL,
                adx14 REAL,
                plus_di REAL,
                minus_di REAL,
                structure TEXT,
                oi_change_pct REAL,
                range_24h_pct REAL,
                range_p40_90d REAL,
                atr_surge_ratio REAL,
                is_warmup BOOLEAN NOT NULL,
                PRIMARY KEY (symbol, timeframe, open_time)
            );",
        )
        .execute(&self.pool)
        .await?;

        // Thêm cột nếu chưa tồn tại (cho các DB cũ)
        let _ = sqlx::query("ALTER TABLE closed_candles ADD COLUMN open_time_str TEXT;").execute(&self.pool).await;
        let _ = sqlx::query("ALTER TABLE closed_candles ADD COLUMN close_time_str TEXT;").execute(&self.pool).await;

        // Bảng lưu danh sách 100 Altcoins tiềm năng nhất (Universe) đã qua bộ lọc Composite Score
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS universe_candidates (
                symbol TEXT PRIMARY KEY,
                quote_volume REAL NOT NULL,
                volume_change_24h_pct REAL NOT NULL,
                open_interest REAL NOT NULL,
                oi_change_24h_pct REAL NOT NULL,
                volatility REAL NOT NULL,
                funding_rate REAL NOT NULL,
                vol_score REAL NOT NULL,
                vol_change_score REAL NOT NULL,
                oi_score REAL NOT NULL,
                oi_change_score REAL NOT NULL,
                atr_score REAL NOT NULL,
                fund_score REAL NOT NULL,
                composite_score REAL NOT NULL,
                price_change_percent REAL NOT NULL,
                last_price REAL NOT NULL,
                updated_at INTEGER NOT NULL
            );"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Lưu trữ danh sách Universe Candidates vào DB
    pub async fn save_universe_candidates(&self, candidates: &[crate::core::metadata::UniverseCandidate]) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        
        // Xóa dữ liệu cũ trước khi nạp mới
        sqlx::query("DELETE FROM universe_candidates").execute(&self.pool).await?;

        for c in candidates {
            sqlx::query(
                r#"
                INSERT INTO universe_candidates (
                    symbol, quote_volume, volume_change_24h_pct, open_interest, oi_change_24h_pct,
                    volatility, funding_rate, vol_score, vol_change_score, oi_score,
                    oi_change_score, atr_score, fund_score, composite_score,
                    price_change_percent, last_price, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                "#
            )
            .bind(&c.symbol)
            .bind(c.quote_volume)
            .bind(c.volume_change_24h_pct)
            .bind(c.open_interest)
            .bind(c.oi_change_24h_pct)
            .bind(c.volatility)
            .bind(c.funding_rate)
            .bind(c.vol_score)
            .bind(c.vol_change_score)
            .bind(c.oi_score)
            .bind(c.oi_change_score)
            .bind(c.atr_score)
            .bind(c.fund_score)
            .bind(c.composite_score)
            .bind(c.price_change_percent)
            .bind(c.last_price)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Lấy danh sách Universe Candidates mới nhất từ DB
    pub async fn get_stored_universe_candidates(&self) -> Result<Vec<crate::core::metadata::UniverseCandidate>> {
        let rows = sqlx::query(
            "SELECT symbol, quote_volume, volume_change_24h_pct, open_interest, oi_change_24h_pct,
                    volatility, funding_rate, vol_score, vol_change_score, oi_score,
                    oi_change_score, atr_score, fund_score, composite_score,
                    price_change_percent, last_price 
             FROM universe_candidates 
             ORDER BY composite_score DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let candidates = rows.into_iter().map(|r| {
            crate::core::metadata::UniverseCandidate {
                symbol: r.get(0),
                quote_volume: r.get(1),
                volume_change_24h_pct: r.get(2),
                open_interest: r.get(3),
                oi_change_24h_pct: r.get(4),
                volatility: r.get(5),
                funding_rate: r.get(6),
                vol_score: r.get(7),
                vol_change_score: r.get(8),
                oi_score: r.get(9),
                oi_change_score: r.get(10),
                atr_score: r.get(11),
                fund_score: r.get(12),
                composite_score: r.get(13),
                price_change_percent: r.get(14),
                last_price: r.get(15),
            }
        }).collect();

        Ok(candidates)
    }

    /// Lưu nến đã đóng
    pub async fn insert_closed_candle(&self, data: &NormalizedCandleData) -> Result<()> {
        let open_time_str = chrono::DateTime::from_timestamp_millis(data.candle.open_time)
            .map(|dt| dt.with_timezone(&chrono::Local).format("%d:%m:%Y %H:%M:%S").to_string())
            .unwrap_or_default();
        let close_time_str = chrono::DateTime::from_timestamp_millis(data.candle.close_time)
            .map(|dt| dt.with_timezone(&chrono::Local).format("%d:%m:%Y %H:%M:%S").to_string())
            .unwrap_or_default();

        sqlx::query(
            "INSERT OR REPLACE INTO closed_candles (
                symbol, timeframe, open_time, close_time, open_time_str, close_time_str,
                open, high, low, close, volume,
                ema20, ema50, ema200, atr14, adx14, plus_di, minus_di, structure,
                oi_change_pct, range_24h_pct, range_p40_90d, atr_surge_ratio, is_warmup
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"
        )
        .bind(&data.candle.symbol)
        .bind(&data.candle.timeframe)
        .bind(data.candle.open_time)
        .bind(data.candle.close_time)
        .bind(open_time_str)
        .bind(close_time_str)
        .bind(data.candle.open)
        .bind(data.candle.high)
        .bind(data.candle.low)
        .bind(data.candle.close)
        .bind(data.candle.volume)
        .bind(data.indicators.ema20)
        .bind(data.indicators.ema50)
        .bind(data.indicators.ema200)
        .bind(data.indicators.atr14)
        .bind(data.indicators.adx14)
        .bind(data.indicators.plus_di)
        .bind(data.indicators.minus_di)
        .bind(&data.indicators.structure)
        .bind(data.microstructure.oi_change_4h_pct)
        .bind(data.range_24h_pct)
        .bind(data.range_p40_90d)
        .bind(data.atr_surge_ratio)
        .bind(data.metadata.is_warmup)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Lưu danh sách nến (Batch) vào DB
    pub async fn insert_closed_candles_batch(&self, batch: &[NormalizedCandleData]) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for data in batch {
            let open_time_str = chrono::DateTime::from_timestamp_millis(data.candle.open_time)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%d:%m:%Y %H:%M:%S").to_string())
                .unwrap_or_default();
            let close_time_str = chrono::DateTime::from_timestamp_millis(data.candle.close_time)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%d:%m:%Y %H:%M:%S").to_string())
                .unwrap_or_default();

            sqlx::query(
                "INSERT OR REPLACE INTO closed_candles (
                    symbol, timeframe, open_time, close_time, open_time_str, close_time_str,
                    open, high, low, close, volume,
                    ema20, ema50, ema200, atr14, adx14, plus_di, minus_di, structure,
                    oi_change_pct, range_24h_pct, range_p40_90d, atr_surge_ratio, is_warmup
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"
            )
            .bind(&data.candle.symbol)
            .bind(&data.candle.timeframe)
            .bind(data.candle.open_time)
            .bind(data.candle.close_time)
            .bind(open_time_str)
            .bind(close_time_str)
            .bind(data.candle.open)
            .bind(data.candle.high)
            .bind(data.candle.low)
            .bind(data.candle.close)
            .bind(data.candle.volume)
            .bind(data.indicators.ema20)
            .bind(data.indicators.ema50)
            .bind(data.indicators.ema200)
            .bind(data.indicators.atr14)
            .bind(data.indicators.adx14)
            .bind(data.indicators.plus_di)
            .bind(data.indicators.minus_di)
            .bind(&data.indicators.structure)
            .bind(data.microstructure.oi_change_4h_pct)
            .bind(data.range_24h_pct)
            .bind(data.range_p40_90d)
            .bind(data.atr_surge_ratio)
            .bind(data.metadata.is_warmup)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    /// Lấy danh sách nến từ DB để tính toán chỉ báo mà không cần gọi API
    pub async fn get_candles_with_indicators(&self, symbol: &str, timeframe: &str, limit: usize) -> Result<Vec<crate::core::models::NormalizedCandleData>> {
        let rows = sqlx::query(
            "SELECT symbol, timeframe, open_time, close_time, open, high, low, close, volume,
                    ema20, ema50, ema200, atr14, adx14, plus_di, minus_di, structure
             FROM closed_candles 
             WHERE symbol = ?1 AND timeframe = ?2 
             ORDER BY open_time DESC LIMIT ?3"
        )
        .bind(symbol)
        .bind(timeframe)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut data: Vec<crate::core::models::NormalizedCandleData> = rows.iter().map(|r| {
            crate::core::models::NormalizedCandleData {
                timestamp: r.get(2),
                candle: crate::core::models::Candle {
                    symbol: r.get(0),
                    timeframe: r.get(1),
                    open_time: r.get(2),
                    close_time: r.get(3),
                    open: r.get(4),
                    high: r.get(5),
                    low: r.get(6),
                    close: r.get(7),
                    volume: r.get(8),
                    quote_volume: 0.0,
                    taker_buy_volume: 0.0,
                    is_closed: true,
                },
                indicators: crate::core::models::Indicators {
                    ema20: r.get(9),
                    ema50: r.get(10),
                    ema200: r.get(11),
                    atr14: r.get(12),
                    adx14: r.get(13),
                    plus_di: r.get(14),
                    minus_di: r.get(15),
                    structure: r.get(16),
                    ..Default::default()
                },
                ..Default::default()
            }
        }).collect();
        
        data.reverse(); 
        Ok(data)
    }

    /// Tìm kiếm nến kèm chỉ báo với cơ chế fuzzy match
    pub async fn search_candles_with_indicators(&self, search_term: &str, timeframe: &str, limit: usize) -> Result<Vec<crate::core::models::NormalizedCandleData>> {
        let pattern = format!("%{}%", search_term.to_uppercase());
        let rows = sqlx::query(
            r#"SELECT symbol, timeframe, open_time, close_time, open, high, low, close, volume,
                    ema20, ema50, ema200, atr14, adx14, plus_di, minus_di, structure
             FROM closed_candles 
             WHERE symbol LIKE ?1 AND (?2 = "" OR timeframe = ?2) 
             ORDER BY open_time DESC LIMIT ?3"#
        )
        .bind(pattern)
        .bind(timeframe)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut data: Vec<crate::core::models::NormalizedCandleData> = rows.iter().map(|r| {
            crate::core::models::NormalizedCandleData {
                timestamp: r.get(2),
                candle: crate::core::models::Candle {
                    symbol: r.get(0),
                    timeframe: r.get(1),
                    open_time: r.get(2),
                    close_time: r.get(3),
                    open: r.get(4),
                    high: r.get(5),
                    low: r.get(6),
                    close: r.get(7),
                    volume: r.get(8),
                    quote_volume: 0.0,
                    taker_buy_volume: 0.0,
                    is_closed: true,
                },
                indicators: crate::core::models::Indicators {
                    ema20: r.get(9),
                    ema50: r.get(10),
                    ema200: r.get(11),
                    atr14: r.get(12),
                    adx14: r.get(13),
                    plus_di: r.get(14),
                    minus_di: r.get(15),
                    structure: r.get(16),
                    ..Default::default()
                },
                ..Default::default()
            }
        }).collect();
        
        data.reverse(); 
        Ok(data)
    }

    /// Đếm tổng số nến thô kèm chỉ báo kỹ thuật theo symbol (cho phân trang/hiển thị)
    pub async fn count_search_candles(&self, search_term: &str, timeframe: &str) -> Result<i64> {
        let pattern = format!("%{}%", search_term.to_uppercase());
        let (count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM closed_candles WHERE symbol LIKE ?1 AND (?2 = "" OR timeframe = ?2)"#
        )
        .bind(pattern)
        .bind(timeframe)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Lấy danh sách nến thô
    pub async fn get_candles(&self, symbol: &str, timeframe: &str, limit: usize) -> Result<Vec<crate::core::models::Candle>> {
        let rows = sqlx::query(
            "SELECT symbol, timeframe, open_time, close_time, open, high, low, close, volume 
             FROM closed_candles 
             WHERE symbol = ?1 AND timeframe = ?2 
             ORDER BY open_time DESC LIMIT ?3"
        )
        .bind(symbol)
        .bind(timeframe)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut candles: Vec<crate::core::models::Candle> = rows.iter().map(|r| {
            crate::core::models::Candle {
                symbol: r.get(0),
                timeframe: r.get(1),
                open_time: r.get(2),
                close_time: r.get(3),
                open: r.get(4),
                high: r.get(5),
                low: r.get(6),
                close: r.get(7),
                volume: r.get(8),
                quote_volume: 0.0,
                taker_buy_volume: 0.0,
                is_closed: true,
            }
        }).collect();
        
        candles.reverse();
        Ok(candles)
    }

    /// Kiểm tra xem dữ liệu trong máy còn mới không
    pub async fn get_last_update_time(&self, symbol: &str, timeframe: &str) -> Result<i64> {
        let row = sqlx::query("SELECT MAX(close_time) FROM closed_candles WHERE symbol = ?1 AND timeframe = ?2")
            .bind(symbol).bind(timeframe).fetch_optional(&self.pool).await?;
        
        Ok(row.and_then(|r| r.get::<Option<i64>, _>(0)).unwrap_or(0))
    }

    pub async fn get_p40_range_90d(&self, symbol: &str) -> Result<f64> {
        let config = crate::core::config::AppConfig::load();
        let tf = config.altcoin_analysis_timeframe;
        let rows = sqlx::query("SELECT range_24h_pct FROM closed_candles WHERE symbol = ?1 AND timeframe = ?2 ORDER BY open_time DESC LIMIT 90")
            .bind(symbol).bind(&tf).fetch_all(&self.pool).await?;
        if rows.is_empty() { return Ok(0.0); }
        let mut ranges: Vec<f64> = rows.iter().map(|r| r.get::<f64, _>(0)).collect();
        ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = (ranges.len() as f64 * 0.4) as usize;
        Ok(*ranges.get(idx).unwrap_or(&0.0))
    }
}
