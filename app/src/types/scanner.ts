export interface ScanMetrics {
    vol_growth_4h_pct: number;
    oi_growth_4h_pct: number;
    distance_to_ema50_4h_pct: number;
    funding_rate: number;
}

export interface ScanCandidate {
    symbol: string;
    rs_score: number;
    rs_rating: string;
    direction: string;
    rank_score: number;
    metrics: ScanMetrics;
    reason: string;
}

export interface ScannerPayload {
    scan_timestamp: number;
    shortlist: ScanCandidate[];
}
