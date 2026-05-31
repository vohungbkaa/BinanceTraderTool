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
    liquidation_surge_detected: boolean;
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
