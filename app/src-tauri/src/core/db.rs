use anyhow::{Context, Result};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use tracing::info;

use super::models::{NormalizedCandleData};

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

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");

        // Bảng lưu cấu hình/danh sách Symbol
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS symbol_config (
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
                id INTEGER PRIMARY KEY AUTOINCREMENT,
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
                
                -- Indicators
                ema20 REAL,
                ema50 REAL,
                ema200 REAL,
                atr14 REAL,
                adx14 REAL,
                plus_di REAL,
                minus_di REAL,
                structure TEXT,

                -- Risk & Context
                oi_change_pct REAL,
                range_24h_pct REAL,
                range_p40_90d REAL,
                atr_surge_ratio REAL,
                is_warmup BOOLEAN NOT NULL,

                UNIQUE(symbol, timeframe, open_time)
            );",
        )
        .execute(&self.pool)
        .await?;

        // Thêm cột nếu chưa tồn tại (cho các DB cũ)
        let _ = sqlx::query("ALTER TABLE closed_candles ADD COLUMN open_time_str TEXT;").execute(&self.pool).await;
        let _ = sqlx::query("ALTER TABLE closed_candles ADD COLUMN close_time_str TEXT;").execute(&self.pool).await;

        Ok(())
    }

    pub async fn insert_closed_candle(&self, data: &NormalizedCandleData) -> Result<()> {
        let open_time_str = chrono::DateTime::from_timestamp_millis(data.candle.open_time)
            .map(|dt| dt.format("%d:%m:%Y %H:%M:%S").to_string())
            .unwrap_or_default();
        let close_time_str = chrono::DateTime::from_timestamp_millis(data.candle.close_time)
            .map(|dt| dt.format("%d:%m:%Y %H:%M:%S").to_string())
            .unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO closed_candles (
                symbol, timeframe, open_time, close_time, open_time_str, close_time_str,
                open, high, low, close, volume,
                ema20, ema50, ema200, atr14, adx14, plus_di, minus_di, structure,
                oi_change_pct, range_24h_pct, range_p40_90d, atr_surge_ratio,
                is_warmup
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
            ON CONFLICT(symbol, timeframe, open_time) 
            DO UPDATE SET 
                close=excluded.close, 
                structure=excluded.structure,
                ema20=excluded.ema20,
                ema50=excluded.ema50,
                ema200=excluded.ema200,
                atr14=excluded.atr14,
                adx14=excluded.adx14,
                plus_di=excluded.plus_di,
                minus_di=excluded.minus_di,
                oi_change_pct=excluded.oi_change_pct,
                range_24h_pct=excluded.range_24h_pct,
                range_p40_90d=excluded.range_p40_90d,
                atr_surge_ratio=excluded.atr_surge_ratio,
                open_time_str=excluded.open_time_str,
                close_time_str=excluded.close_time_str
            "#
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
        
        data.reverse(); // Đảo lại theo thứ tự thời gian tăng dần
        Ok(data)
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
