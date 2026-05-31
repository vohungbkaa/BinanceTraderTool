import { defineStore } from 'pinia';
import { ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { NormalizedCandleData, MarketIndices, MarketRegimeContext } from '../types/market';
import { StructuralTrend, OperationalState, RiskStatus, ActionMode } from '../types/market';

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
        operational_state: OperationalState.Pullback,
        risk_status: RiskStatus.Normal,
        market_score: 0,
        allow_alt_scan: false,
        action_mode: ActionMode.OffSystem,
    });
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

    return { btcData, timeframes, marketIndices, regime, logs, init };
});
