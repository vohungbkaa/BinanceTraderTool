use ta::indicators::{ExponentialMovingAverage, AverageTrueRange};
use ta::{Next, DataItem};
use crate::core::models::{Candle, Indicators};
use std::collections::HashMap;

pub struct SymbolIndicatorState {
    ema20: ExponentialMovingAverage,
    ema50: ExponentialMovingAverage,
    ema200: ExponentialMovingAverage,
    atr14: AverageTrueRange,
    atr20_avg: ExponentialMovingAverage,
    smoothed_tr: ExponentialMovingAverage,
    smoothed_plus_dm: ExponentialMovingAverage,
    smoothed_minus_dm: ExponentialMovingAverage,
    smoothed_dx: ExponentialMovingAverage,
    prev_candle: Option<Candle>,
    candle_buffer: Vec<Candle>,
    last_pivot_high: f64,
    last_pivot_low: f64,
}

impl SymbolIndicatorState {
    pub fn new() -> Self {
        Self {
            ema20: ExponentialMovingAverage::new(20).unwrap(),
            ema50: ExponentialMovingAverage::new(50).unwrap(),
            ema200: ExponentialMovingAverage::new(200).unwrap(),
            atr14: AverageTrueRange::new(14).unwrap(),
            atr20_avg: ExponentialMovingAverage::new(20).unwrap(),
            smoothed_tr: ExponentialMovingAverage::new(14).unwrap(),
            smoothed_plus_dm: ExponentialMovingAverage::new(14).unwrap(),
            smoothed_minus_dm: ExponentialMovingAverage::new(14).unwrap(),
            smoothed_dx: ExponentialMovingAverage::new(14).unwrap(),
            prev_candle: None,
            candle_buffer: Vec::with_capacity(7),
            last_pivot_high: 0.0,
            last_pivot_low: f64::MAX,
        }
    }

    fn detect_pivot_fractal(&mut self) -> String {
        if self.candle_buffer.len() < 7 { return "None".to_string(); }
        let mid_idx = 3;
        let mid_candle = &self.candle_buffer[mid_idx];
        let is_pivot_high = self.candle_buffer.iter().enumerate().all(|(i, c)| i == mid_idx || c.high < mid_candle.high);
        if is_pivot_high {
            let current_high = mid_candle.high;
            let label = if current_high > self.last_pivot_high { "HH" } else { "LH" };
            self.last_pivot_high = current_high;
            return label.to_string();
        }
        let is_pivot_low = self.candle_buffer.iter().enumerate().all(|(i, c)| i == mid_idx || c.low > mid_candle.low);
        if is_pivot_low {
            let current_low = mid_candle.low;
            let label = if current_low > self.last_pivot_low { "HL" } else { "LL" };
            self.last_pivot_low = current_low;
            return label.to_string();
        }
        "None".to_string()
    }

    fn compute_adx(&mut self, candle: &Candle) -> (Option<f64>, Option<f64>, Option<f64>) {
        if let Some(prev) = &self.prev_candle {
            let h_diff = candle.high - prev.high;
            let l_diff = prev.low - candle.low;
            let plus_dm = if h_diff > l_diff && h_diff > 0.0 { h_diff } else { 0.0 };
            let minus_dm = if l_diff > h_diff && l_diff > 0.0 { l_diff } else { 0.0 };
            let tr = (candle.high - candle.low).max((candle.high - prev.close).abs()).max((candle.low - prev.close).abs());
            let s_tr = self.smoothed_tr.next(tr);
            let s_plus_dm = self.smoothed_plus_dm.next(plus_dm);
            let s_minus_dm = self.smoothed_minus_dm.next(minus_dm);
            if s_tr > 0.0 {
                let plus_di = 100.0 * s_plus_dm / s_tr;
                let minus_di = 100.0 * s_minus_dm / s_tr;
                let dx = 100.0 * (plus_di - minus_di).abs() / (plus_di + minus_di);
                let adx = self.smoothed_dx.next(dx);
                return (Some(adx), Some(plus_di), Some(minus_di));
            }
        }
        (None, None, None)
    }

    pub fn next(&mut self, candle: &Candle) -> Indicators {
        let ema20 = self.ema20.next(candle.close);
        let ema50 = self.ema50.next(candle.close);
        let ema200 = self.ema200.next(candle.close);

        // [FIX] Kiểm tra nến hợp lệ trước khi builder DataItem để tránh panic
        let mut atr = 0.0;
        if candle.high >= candle.low && candle.high > 0.0 {
            if let Ok(data_item) = DataItem::builder()
                .open(candle.open).high(candle.high).low(candle.low).close(candle.close).volume(candle.volume)
                .build() {
                atr = self.atr14.next(&data_item);
                self.atr20_avg.next(atr);
            }
        }

        let (adx, plus_di, minus_di) = self.compute_adx(candle);
        self.prev_candle = Some(candle.clone());
        if self.candle_buffer.len() >= 7 { self.candle_buffer.remove(0); }
        self.candle_buffer.push(candle.clone());
        let structure = self.detect_pivot_fractal();

        Indicators {
            ema20: Some(ema20), ema50: Some(ema50), ema200: Some(ema200),
            atr14: Some(atr), adx14: adx, plus_di, minus_di, structure,
        }
    }
}

pub struct IndicatorEngine {
    states: HashMap<String, SymbolIndicatorState>,
}

impl IndicatorEngine {
    pub fn new() -> Self {
        Self { states: HashMap::new() }
    }

    pub fn process(&mut self, candle: &Candle) -> Indicators {
        let key = format!("{}:{}", candle.symbol, candle.timeframe);
        let state = self.states.entry(key).or_insert_with(SymbolIndicatorState::new);
        state.next(candle)
    }
}
