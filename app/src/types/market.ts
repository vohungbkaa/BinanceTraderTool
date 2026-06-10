export interface Candle {
    symbol: string;
    timeframe: string;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
    is_closed: boolean;
}

export interface Indicators {
    ema20?: number;
    ema50?: number;
    ema200?: number;
    atr14?: number;
    adx14?: number;
    plus_di?: number;
    minus_di?: number;
    structure: string;
    close_above_ema200_count: number;
    ema50_slope: number;
}

export interface MarketIndices {
    btc_d_trend: string;
    total3_btc_trend: string;
    market_breadth_pct_above_ema50: number;
    market_breadth_pct_above_ema200: number;
}

export interface Microstructure {
    oi_change_4h_pct: number;
    price_change_4h_pct: number;
    funding_rate_avg: number;
    cvd_4h: number;
    cvd_1d: number;
    liquidation_surge_detected: boolean;
    liquidation_upper_real: number;
    liquidation_lower_real: number;
    liquidation_upper_est: number;
    liquidation_lower_est: number;
    spread_anomaly: boolean;
}

export interface MacroEvents {
    next_event_name: string;
    time_to_event_minutes: number;
    is_event_block_window: boolean;
}

export interface NormalizedCandleData {
    timestamp: number;
    candle: Candle;
    indicators: Indicators;
    market_indices: MarketIndices;
    microstructure: Microstructure;
    macro_events: MacroEvents;
    range_24h_pct: number;
    range_p40_90d: number;
    atr_surge_ratio: number;
}

export enum StructuralTrend {
    MacroBullish = "MacroBullish",
    MacroBearish = "MacroBearish",
    MacroNeutral = "MacroNeutral",
}

export enum OperationalState {
    ActiveBullish = "Active_Bullish",
    ActiveBearish = "Active_Bearish",
    BullishPullback = "Bullish_Pullback",
    BearishPullback = "Bearish_Pullback",
    DynamicSideway = "Dynamic_Sideway",
}

export enum RiskStatus {
    Normal = "Normal",
    EventBlock = "Event_Block",
    VolatilityAlert = "Volatility_Alert",
    MicrostructureReset = "Microstructure_Reset",
}

export enum ActionMode {
    AggressiveLong = "Aggressive_Long",
    AggressiveShort = "Aggressive_Short",
    ScalpLong = "Scalp_Long",
    ScalpShort = "Scalp_Short",
    MeanReversion = "Mean_Reversion",
    OffSystem = "Off_System",
}

export enum VolatilityRegime {
    Compression = "Compression",
    Expansion = "Expansion",
    Extreme = "Extreme",
}

export enum OIState {
    LongBuildUp = "LongBuildUp",
    ShortBuildUp = "ShortBuildUp",
    LongLiquidation = "LongLiquidation",
    ShortCovering = "ShortCovering",
    Neutral = "Neutral",
}

export interface ChecklistItem {
    group: string;
    label: string;
    status: boolean;
}

export interface MarketRegimeContext {
    structural_trend: StructuralTrend;
    operational_state: OperationalState;
    volatility_regime: VolatilityRegime;
    oi_state: OIState;
    risk_status: RiskStatus;
    trend_score: number;
    flow_score: number;
    allow_alt_scan: boolean;
    action_mode: ActionMode;
    checklist: ChecklistItem[];
}
