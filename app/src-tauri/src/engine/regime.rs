use crate::core::events::MarketEvent;
use crate::core::models::NormalizedCandleData;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StructuralTrend {
    MacroBullish,
    MacroBearish,
    MacroNeutral,
}

impl std::fmt::Display for StructuralTrend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::MacroBullish => "Macro_Bullish",
            Self::MacroBearish => "Macro_Bearish",
            Self::MacroNeutral => "Macro_Neutral",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationalState {
    ActiveBullish,
    ActiveBearish,
    BullishPullback,
    BearishPullback,
    DynamicSideway,
}

impl std::fmt::Display for OperationalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ActiveBullish => "Active_Bullish",
            Self::ActiveBearish => "Active_Bearish",
            Self::BullishPullback => "Bullish_Pullback",
            Self::BearishPullback => "Bearish_Pullback",
            Self::DynamicSideway => "Dynamic_Sideway",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskStatus {
    Normal,
    EventBlock,
    VolatilityAlert,
    MicrostructureReset,
}

impl std::fmt::Display for RiskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Normal => "Normal",
            Self::EventBlock => "Event_Block",
            Self::VolatilityAlert => "Volatility_Alert",
            Self::MicrostructureReset => "Microstructure_Reset",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionMode {
    AggressiveLong,
    AggressiveShort,
    ScalpLong,
    ScalpShort,
    MeanReversion,
    OffSystem,
}

impl std::fmt::Display for ActionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::AggressiveLong => "Aggressive_Long",
            Self::AggressiveShort => "Aggressive_Short",
            Self::ScalpLong => "Scalp_Long",
            Self::ScalpShort => "Scalp_Short",
            Self::MeanReversion => "Mean_Reversion",
            Self::OffSystem => "Off_System",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VolatilityRegime {
    Compression,
    Expansion,
    Extreme,
}

impl std::fmt::Display for VolatilityRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OIState {
    LongBuildUp,
    ShortBuildUp,
    LongLiquidation,
    ShortCovering,
    Neutral,
}

impl std::fmt::Display for OIState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::LongBuildUp => "Long Build-up",
            Self::ShortBuildUp => "Short Build-up",
            Self::LongLiquidation => "Long Liquidation",
            Self::ShortCovering => "Short Covering",
            Self::Neutral => "Neutral",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub group: String,
    pub label: String,
    pub status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegimeContext {
    pub structural_trend: StructuralTrend,
    pub operational_state: OperationalState,
    pub volatility_regime: VolatilityRegime,
    pub oi_state: OIState,
    pub risk_status: RiskStatus,
    pub trend_score: i32,
    pub flow_score: i32,
    pub allow_alt_scan: bool,
    pub action_mode: ActionMode,
    pub checklist: Vec<ChecklistItem>,
}

pub struct MarketRegimeEngine {
    latest_1d: Option<NormalizedCandleData>,
    latest_4h: Option<NormalizedCandleData>,
}

impl MarketRegimeEngine {
    pub fn new() -> Self {
        Self {
            latest_1d: None,
            latest_4h: None,
        }
    }

    /// Khởi chạy Engine Phase 1 như một task độc lập
    pub async fn run(
        &mut self,
        mut event_rx: broadcast::Receiver<MarketEvent>,
        event_tx: broadcast::Sender<MarketEvent>,
    ) {
        info!("Phase 1: Market Regime Engine is running and listening for events...");

        loop {
            match event_rx.recv().await {
                Ok(event) => match event {
                    MarketEvent::CandleClosed(data) | MarketEvent::CandleUpdated(data) => {
                        if data.candle.symbol == "BTCUSDT" {
                            let mut trigger_analysis = false;

                            let alt_tf =
                                crate::core::config::AppConfig::load().altcoin_analysis_timeframe;
                            if data.candle.timeframe == alt_tf {
                                self.latest_1d = Some(data.clone());
                                trigger_analysis = true;
                            } else if data.candle.timeframe == "4h" {
                                self.latest_4h = Some(data.clone());
                                trigger_analysis = true;
                            } else if data.candle.timeframe == "15m" {
                                trigger_analysis = true;
                            }

                            if trigger_analysis {
                                let context = self.analyze(&data).await;
                                if let Err(e) = event_tx.send(MarketEvent::RegimeUpdated(context)) {
                                    error!("Failed to broadcast RegimeUpdated: {}", e);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    error!("MarketRegimeEngine lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("MarketRegimeEngine event channel closed. Exiting...");
                    break;
                }
            }
        }
    }

    /// Dành riêng cho Backtest/Simulator: Nhồi trực tiếp dữ liệu 1D và 4H để lấy kết quả
    pub async fn evaluate_historical(
        &mut self,
        data_1d: &NormalizedCandleData,
        data_4h: &NormalizedCandleData,
    ) -> MarketRegimeContext {
        self.latest_1d = Some(data_1d.clone());
        self.latest_4h = Some(data_4h.clone());
        // Giả lập nến trigger là nến 4H
        self.analyze(data_4h).await
    }

    async fn analyze(&self, current_data: &NormalizedCandleData) -> MarketRegimeContext {
        let mut risk_status = RiskStatus::Normal;
        let mut allow_alt_scan = false;
        let mut action_mode = ActionMode::OffSystem;
        let mut structural_trend = StructuralTrend::MacroNeutral;
        let mut operational_state = OperationalState::DynamicSideway;
        let mut oi_state = OIState::Neutral;
        let mut checklist = Vec::new();

        let data_1d = self.latest_1d.as_ref().unwrap_or(current_data);
        let data_4h = self.latest_4h.as_ref().unwrap_or(current_data);
        let risk_data = current_data;

        // ---------------------------------------------------------
        // BỘ LỌC 1: QUẢN TRỊ RỦI RO (RISK FIRST)
        // ---------------------------------------------------------
        if risk_data.macro_events.is_event_block_window {
            risk_status = RiskStatus::EventBlock;
        } else if risk_data.microstructure.liquidation_surge_detected {
            risk_status = RiskStatus::MicrostructureReset;
        } else if risk_data.atr_surge_ratio > 3.0 {
            risk_status = RiskStatus::VolatilityAlert;
        }

        checklist.push(ChecklistItem {
            group: "Risk Layer".to_string(),
            label: "No Event Block".to_string(),
            status: risk_status != RiskStatus::EventBlock,
        });
        checklist.push(ChecklistItem {
            group: "Risk Layer".to_string(),
            label: "Stability Normal".to_string(),
            status: risk_status == RiskStatus::Normal,
        });

        // ---------------------------------------------------------
        // BỘ LỌC 2: PHÂN TÍCH XU HƯỚNG ĐA TẦNG (TREND LAYER)
        // ---------------------------------------------------------
        // Tầng Vĩ Mô (1D)
        if let Some(ema50) = data_1d.indicators.ema50 {
            let is_above = data_1d.candle.close > ema50;
            checklist.push(ChecklistItem {
                group: "Macro Layer".to_string(),
                label: "BTC > EMA50 Daily".to_string(),
                status: is_above,
            });
        }
        if let Some(ema200) = data_1d.indicators.ema200 {
            let is_above = data_1d.candle.close > ema200;
            checklist.push(ChecklistItem {
                group: "Macro Layer".to_string(),
                label: "BTC > EMA200 Daily".to_string(),
                status: is_above,
            });

            let is_hh_hl =
                data_1d.indicators.structure == "HH" || data_1d.indicators.structure == "HL";

            if data_1d.candle.close > ema200
                && data_1d.indicators.close_above_ema200_count >= 3
                && is_hh_hl
            {
                structural_trend = StructuralTrend::MacroBullish;
            } else if data_1d.candle.close < ema200 {
                structural_trend = StructuralTrend::MacroBearish;
            }
        }

        // Tầng Vi Mô (4H)
        if let (Some(ema50), Some(ema200), Some(adx), Some(plus_di), Some(minus_di)) = (
            data_4h.indicators.ema50,
            data_4h.indicators.ema200,
            data_4h.indicators.adx14,
            data_4h.indicators.plus_di,
            data_4h.indicators.minus_di,
        ) {
            let is_hh_hl =
                data_4h.indicators.structure == "HH" || data_4h.indicators.structure == "HL";
            let is_ll_lh =
                data_4h.indicators.structure == "LL" || data_4h.indicators.structure == "LH";

            if data_4h.candle.close > ema50
                && ema50 > ema200
                && is_hh_hl
                && plus_di > minus_di
                && adx > 25.0
            {
                operational_state = OperationalState::ActiveBullish;
            } else if data_4h.candle.close < ema50
                && ema50 < ema200
                && is_ll_lh
                && minus_di > plus_di
                && adx > 25.0
            {
                operational_state = OperationalState::ActiveBearish;
            } else if data_4h.range_24h_pct < data_4h.range_p40_90d && adx < 20.0 {
                operational_state = OperationalState::DynamicSideway;
            } else {
                if structural_trend == StructuralTrend::MacroBullish {
                    operational_state = OperationalState::BullishPullback;
                } else {
                    operational_state = OperationalState::BearishPullback;
                }
            }
        }

        // Volatility Regime
        let (volatility_regime, is_expansion) = if risk_data.atr_surge_ratio > 2.5 {
            (VolatilityRegime::Extreme, true)
        } else if risk_data.range_24h_pct > risk_data.range_p40_90d * 1.2 {
            (VolatilityRegime::Expansion, true)
        } else {
            (VolatilityRegime::Compression, false)
        };

        checklist.push(ChecklistItem {
            group: "Risk Layer".to_string(),
            label: format!("Volatility: {}", volatility_regime),
            status: is_expansion,
        });

        // ---------------------------------------------------------
        // BỘ LỌC 3: ĐÁNH GIÁ DÒNG TIỀN & ĐỘNG LƯỢNG (FLOW LAYER)
        // ---------------------------------------------------------
        let breadth_ema50 = risk_data.market_indices.market_breadth_pct_above_ema50;
        let _breadth_ema200 = risk_data.market_indices.market_breadth_pct_above_ema200;

        let breadth_bearish = breadth_ema50 < 40.0;
        checklist.push(ChecklistItem {
            group: "Breadth Layer".to_string(),
            label: format!("Breadth EMA50 ({:.0}%) Low", breadth_ema50),
            status: breadth_bearish,
        });

        let btc_d_down =
            risk_data.market_indices.btc_d_trend == crate::core::models::TrendDirection::Down;
        checklist.push(ChecklistItem {
            group: "Flow Layer".to_string(),
            label: "BTC Dominance Falling".to_string(),
            status: btc_d_down,
        });

        // OI State Logic
        let price_change = (data_4h.candle.close - data_4h.candle.open) / data_4h.candle.open;
        let oi_change = risk_data.microstructure.oi_change_4h_pct;

        if price_change > 0.005 && oi_change > 2.0 {
            oi_state = OIState::LongBuildUp;
        } else if price_change < -0.005 && oi_change > 2.0 {
            oi_state = OIState::ShortBuildUp;
        } else if price_change < -0.01 && oi_change < -2.0 {
            oi_state = OIState::LongLiquidation;
        } else if price_change > 0.01 && oi_change < -2.0 {
            oi_state = OIState::ShortCovering;
        }

        checklist.push(ChecklistItem {
            group: "Flow Layer".to_string(),
            label: format!("OI State: {}", oi_state),
            status: oi_state != OIState::Neutral,
        });

        let cvd_1d = risk_data.microstructure.cvd_1d;
        let cvd_4h = risk_data.microstructure.cvd_4h;
        checklist.push(ChecklistItem {
            group: "Flow Layer".to_string(),
            label: "CVD 1D/4H Alignment".to_string(),
            status: (cvd_1d > 0.0 && cvd_4h > 0.0) || (cvd_1d < 0.0 && cvd_4h < 0.0),
        });

        // ---------------------------------------------------------
        // SCORING DECOUPLING
        // ---------------------------------------------------------
        // 1. TREND SCORE (0-100)
        let mut t_score = 0;
        if structural_trend == StructuralTrend::MacroBullish {
            t_score += 40;
        } else if structural_trend == StructuralTrend::MacroBearish {
            t_score += 40;
        }

        if operational_state == OperationalState::ActiveBullish
            || operational_state == OperationalState::ActiveBearish
        {
            t_score += 40;
        } else if operational_state == OperationalState::BullishPullback
            || operational_state == OperationalState::BearishPullback
        {
            t_score += 20;
        }

        if volatility_regime == VolatilityRegime::Expansion {
            t_score += 20;
        }
        let mut trend_score = t_score;

        // 2. FLOW SCORE (0-100)
        let mut f_score = 0;
        // Breadth alignment
        if structural_trend == StructuralTrend::MacroBullish && breadth_ema50 > 50.0 {
            f_score += 25;
        } else if structural_trend == StructuralTrend::MacroBearish && breadth_ema50 < 40.0 {
            f_score += 25;
        }

        // BTC Dominance alignment
        if structural_trend == StructuralTrend::MacroBullish && btc_d_down {
            f_score += 25;
        } else if structural_trend == StructuralTrend::MacroBearish && !btc_d_down {
            f_score += 25;
        }

        // CVD & OI alignment
        if (cvd_4h > 0.0 && oi_state == OIState::LongBuildUp)
            || (cvd_4h < 0.0 && oi_state == OIState::ShortBuildUp)
        {
            f_score += 50;
        } else if cvd_4h != 0.0 {
            f_score += 25;
        }

        let mut flow_score = f_score;

        if risk_status != RiskStatus::Normal {
            trend_score = 0;
            flow_score = 0;
        }

        // ---------------------------------------------------------
        // ACTION MODE (GATEWAY)
        // ---------------------------------------------------------
        let flow_alignment = flow_score >= 50;
        let trend_strong = trend_score >= 80;

        if risk_status == RiskStatus::Normal && trend_score >= 40 {
            if trend_strong && flow_alignment {
                // [CRITICAL] Chỉ Aggressive khi có Volatility Expansion (Phá vỡ)
                if is_expansion {
                    if structural_trend == StructuralTrend::MacroBullish {
                        action_mode = ActionMode::AggressiveLong;
                    } else {
                        action_mode = ActionMode::AggressiveShort;
                    }
                } else {
                    // Nếu đang Compression, hạ cấp xuống Scalp để tránh bị trap/squeeze
                    if structural_trend == StructuralTrend::MacroBullish {
                        action_mode = ActionMode::ScalpLong;
                    } else {
                        action_mode = ActionMode::ScalpShort;
                    }
                }
                allow_alt_scan = true;
            } else if flow_alignment {
                if structural_trend == StructuralTrend::MacroBullish {
                    action_mode = ActionMode::ScalpLong;
                } else {
                    action_mode = ActionMode::ScalpShort;
                }
                allow_alt_scan = true;
            } else if operational_state == OperationalState::DynamicSideway {
                action_mode = ActionMode::MeanReversion;
            }
        }

        MarketRegimeContext {
            structural_trend,
            operational_state,
            volatility_regime,
            oi_state,
            risk_status,
            trend_score,
            flow_score,
            allow_alt_scan,
            action_mode,
            checklist,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{
        Candle, Indicators, MacroEvents, MarketIndices, Microstructure, NormalizedCandleData,
    };

    fn default_mock_data() -> NormalizedCandleData {
        NormalizedCandleData {
            timestamp: 0,
            candle: Candle {
                symbol: "BTCUSDT".to_string(),
                timeframe: "4h".to_string(),
                open: 60000.0,
                close: 61000.0, // Tăng giá
                ..Default::default()
            },
            indicators: Indicators {
                ema50: Some(55000.0),
                ema200: Some(50000.0),
                adx14: Some(30.0),
                plus_di: Some(25.0),
                minus_di: Some(15.0),
                structure: "HH".to_string(),
                close_above_ema200_count: 5,
                ema50_slope: 1.5,
                ..Default::default()
            },
            market_indices: MarketIndices {
                btc_d_trend: crate::core::models::TrendDirection::Down,
                total3_btc_trend: crate::core::models::TrendDirection::Up,
                market_breadth_pct_above_ema50: 60.0,
                market_breadth_pct_above_ema200: 50.0,
            },
            microstructure: Microstructure {
                oi_change_4h_pct: 5.0,    // OI tăng
                funding_rate_avg: 0.0001, // Dưới mức phạt 0.05%
                liquidation_surge_detected: false,
                ..Default::default()
            },
            macro_events: MacroEvents {
                is_event_block_window: false,
                ..Default::default()
            },
            range_24h_pct: 5.0,
            range_p40_90d: 3.0,
            atr_surge_ratio: 1.0,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_ideal_bullish_scenario() {
        let engine = MarketRegimeEngine::new();
        let data = default_mock_data();
        let context = engine.analyze(&data).await;

        assert_eq!(context.risk_status, RiskStatus::Normal);
        assert_eq!(context.structural_trend, StructuralTrend::MacroBullish);
        assert_eq!(context.operational_state, OperationalState::ActiveBullish);
        assert!(context.trend_score + context.flow_score >= 150);
        assert_eq!(context.allow_alt_scan, true);
        assert_eq!(context.action_mode, ActionMode::AggressiveLong);
    }

    #[tokio::test]
    async fn test_event_risk_block() {
        let engine = MarketRegimeEngine::new();
        let mut data = default_mock_data();
        data.macro_events.is_event_block_window = true; // Sắp có FOMC

        let context = engine.analyze(&data).await;

        assert_eq!(context.risk_status, RiskStatus::EventBlock);
        assert_eq!(context.trend_score + context.flow_score, 0); // Bị ép về 0
        assert_eq!(context.allow_alt_scan, false); // Không cấp phép
        assert_eq!(context.action_mode, ActionMode::OffSystem);
    }

    #[tokio::test]
    async fn test_dynamic_sideway_rejection() {
        let engine = MarketRegimeEngine::new();
        let mut data = default_mock_data();
        data.indicators.adx14 = Some(15.0); // Dưới 20
        data.range_24h_pct = 2.0; // Nhỏ hơn P40 (3.0)
        data.indicators.ema50_slope = 0.02; // Gần 0

        let context = engine.analyze(&data).await;

        assert_eq!(context.operational_state, OperationalState::DynamicSideway);
        // Để test ra MeanReversion, điểm phải >= 40.
        // Trend = 0 (do sideway), Risk = 15, Pos = 18, Flow cần thêm điểm.
        // Set TOTAL3 = UP để lấy 20 điểm Flow -> Tổng = 53 điểm.
        data.market_indices.btc_d_trend = crate::core::models::TrendDirection::Up;
        data.market_indices.total3_btc_trend = crate::core::models::TrendDirection::Up;

        let context2 = engine.analyze(&data).await;
        assert_eq!(context2.allow_alt_scan, false);
        assert_eq!(context2.action_mode, ActionMode::MeanReversion);
    }

    #[tokio::test]
    async fn test_bearish_flow_alignment() {
        let engine = MarketRegimeEngine::new();
        let mut data = default_mock_data();
        // Setup Bearish
        data.candle.close = 40000.0;
        data.indicators.ema50 = Some(45000.0);
        data.indicators.ema200 = Some(50000.0);
        data.indicators.structure = "LL".to_string();
        data.indicators.plus_di = Some(15.0);
        data.indicators.minus_di = Some(25.0);

        // Flow thuận Bearish (Dòng tiền rút)
        data.market_indices.btc_d_trend = crate::core::models::TrendDirection::Up;
        data.market_indices.total3_btc_trend = crate::core::models::TrendDirection::Down;

        let context = engine.analyze(&data).await;

        assert_eq!(context.structural_trend, StructuralTrend::MacroBearish);
        assert_eq!(context.operational_state, OperationalState::ActiveBearish);
        assert_eq!(context.allow_alt_scan, true); // Cho phép short
    }
}
