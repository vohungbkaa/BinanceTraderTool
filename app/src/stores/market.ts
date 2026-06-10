import { defineStore } from 'pinia';
import { ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { NormalizedCandleData, MarketIndices, MarketRegimeContext } from '../types/market';
import { StructuralTrend, OperationalState, RiskStatus, ActionMode, VolatilityRegime, OIState } from '../types/market';
import type { ScanCandidate } from '../types/scanner';

export const useMarketStore = defineStore('market', () => {
    const btcData = ref<Record<string, NormalizedCandleData>>({});
    const timeframes = ref<string[]>([]);
    const marketIndices = ref<MarketIndices>({
        btc_d_trend: 'SIDEWAY',
        total3_btc_trend: 'SIDEWAY',
        market_breadth_pct_above_ema50: 0,
        market_breadth_pct_above_ema200: 0,
    });
    const regime = ref<MarketRegimeContext>({
        structural_trend: StructuralTrend.MacroNeutral,
        operational_state: OperationalState.DynamicSideway,
        volatility_regime: VolatilityRegime.Compression,
        oi_state: OIState.Neutral,
        risk_status: RiskStatus.Normal,
        trend_score: 0,
        flow_score: 0,
        allow_alt_scan: false,
        action_mode: ActionMode.OffSystem,
        checklist: [],
    });
    const shortlist = ref<ScanCandidate[]>([]);
    const lastScanTime = ref<number>(0);
    const logs = ref<string[]>([]);

    async function init() {
        // Fetch config from backend
        try {
            const config: any = await invoke('get_config');
            timeframes.value = config.timeframes || ['15m', '4h', '1d'];
        } catch (e) {
            console.error('Failed to fetch config', e);
            timeframes.value = ['15m', '4h', '1d'];
        }

        await listen<any>('market-event', (event) => {
            const eventType = event.payload.event_type;

            if (eventType === 'RegimeUpdated') {
                regime.value = event.payload.payload as MarketRegimeContext;
                // [TỰ BẢO VỆ] Nếu Phase 1 báo đèn đỏ, xóa ngay danh sách quét cũ
                if (!regime.value.allow_alt_scan) {
                    shortlist.value = [];
                }
                return;
            }
            
            if (eventType === 'ScannerUpdated') {
                shortlist.value = event.payload.payload.shortlist;
                lastScanTime.value = event.payload.payload.scan_timestamp;
                return;
            }

            const data = event.payload.payload as NormalizedCandleData;

            if (data.candle.symbol.toUpperCase() === 'BTCUSDT') {
                btcData.value[data.candle.timeframe] = data;
                marketIndices.value = data.market_indices;
            }

            if (eventType === 'CandleClosed') {
                const time = new Date().toLocaleTimeString();
                const logMsg = `[${time}] ${data.candle.symbol} - ${data.candle.timeframe}: C: ${data.candle.close}`;
                logs.value.unshift(logMsg);
                if (logs.value.length > 50) logs.value.pop();
            }
        });
    }

    return { btcData, timeframes, marketIndices, regime, shortlist, lastScanTime, logs, init };
});
