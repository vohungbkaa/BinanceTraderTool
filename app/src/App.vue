<script setup lang="ts">
import { onMounted } from 'vue';
import { useMarketStore } from './stores/market';
import { 
  Activity, 
  BarChart3, 
  ShieldCheck, 
  TrendingUp, 
  TrendingDown, 
  Clock,
  LayoutDashboard
} from '@lucide/vue';

const market = useMarketStore();

onMounted(() => {
  market.init();
});

const formatNum = (val?: number) => val?.toFixed(2) || '---';
const formatPrice = (val?: number) => val?.toLocaleString(undefined, { minimumFractionDigits: 1 }) || '---';

const getTimeframeData = (tf: string) => market.btcData[tf];
</script>

<template>
  <div class="min-h-screen bg-[#0b0e11] text-[#eaecef] font-sans p-6">
    <!-- Header -->
    <header class="flex items-center justify-between mb-8 pb-4 border-b border-gray-800">
      <div class="flex items-center gap-3">
        <div class="p-2 bg-yellow-500/10 rounded-lg">
          <LayoutDashboard class="w-6 h-6 text-yellow-500" />
        </div>
        <div>
          <h1 class="text-xl font-bold tracking-tight">BinanceTraderTool <span class="text-yellow-500 text-sm ml-1">V2</span></h1>
          <p class="text-xs text-gray-500 uppercase tracking-widest">Phase 0: Data Pipeline Monitor</p>
        </div>
      </div>
      
      <div class="flex items-center gap-4">
        <div class="flex items-center gap-2 px-3 py-1.5 bg-green-500/10 rounded-full border border-green-500/20">
          <div class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
          <span class="text-xs font-medium text-green-500">WebSocket Connected</span>
        </div>
      </div>
    </header>

    <div class="grid grid-cols-12 gap-6">
      <!-- Left Column: BTC Benchmark -->
      <div class="col-span-12 lg:col-span-8 space-y-6">
        
        <!-- BTC Live Cards -->
        <div class="grid grid-cols-3 gap-4">
          <div v-for="tf in ['1m', '4h', '1d']" :key="tf" 
               class="bg-[#1e2329] p-5 rounded-xl border border-gray-800 hover:border-yellow-500/30 transition-colors">
            <div class="flex justify-between items-start mb-3">
              <span class="text-xs font-bold text-gray-400 uppercase">{{ tf }} BTCUSDT</span>
              <Activity class="w-4 h-4 text-yellow-500/50" />
            </div>
            <div class="text-2xl font-mono font-bold text-yellow-500">
              ${{ formatPrice(getTimeframeData(tf)?.candle.close) }}
            </div>
            <div class="mt-4 grid grid-cols-2 gap-2 text-[10px] text-gray-500 uppercase font-bold">
              <div>EMA50: <span class="text-gray-300 ml-1">{{ formatNum(getTimeframeData(tf)?.indicators.ema50) }}</span></div>
              <div>EMA200: <span class="text-gray-300 ml-1">{{ formatNum(getTimeframeData(tf)?.indicators.ema200) }}</span></div>
              <div>STR: <span class="text-yellow-500 ml-1">{{ getTimeframeData(tf)?.indicators.structure || 'NONE' }}</span></div>
              <div>ADX: <span class="text-gray-300 ml-1">{{ formatNum(getTimeframeData(tf)?.indicators.adx14) }}</span></div>
            </div>
          </div>
        </div>

        <!-- Market Breadth Section -->
        <div class="bg-[#1e2329] p-6 rounded-xl border border-gray-800">
          <div class="flex items-center gap-2 mb-6">
            <BarChart3 class="w-5 h-5 text-blue-400" />
            <h2 class="text-sm font-bold uppercase tracking-wider">Market Breadth (Top 100 Altcoins)</h2>
          </div>
          
          <div class="grid grid-cols-2 gap-8">
            <div class="space-y-3">
              <div class="flex justify-between text-xs font-bold uppercase">
                <span class="text-gray-400">Above EMA50 (1D)</span>
                <span class="text-blue-400">{{ formatNum(market.marketIndices.market_breadth_pct_above_ema50) }}%</span>
              </div>
              <div class="h-2 bg-gray-800 rounded-full overflow-hidden">
                <div class="h-full bg-blue-500 transition-all duration-500" 
                     :style="{ width: `${market.marketIndices.market_breadth_pct_above_ema50}%` }"></div>
              </div>
            </div>
            
            <div class="space-y-3">
              <div class="flex justify-between text-xs font-bold uppercase">
                <span class="text-gray-400">Above EMA200 (1D)</span>
                <span class="text-purple-400">{{ formatNum(market.marketIndices.market_breadth_pct_above_ema200) }}%</span>
              </div>
              <div class="h-2 bg-gray-800 rounded-full overflow-hidden">
                <div class="h-full bg-purple-500 transition-all duration-500" 
                     :style="{ width: `${market.marketIndices.market_breadth_pct_above_ema200}%` }"></div>
              </div>
            </div>
          </div>

          <div class="mt-8 flex gap-4">
            <div class="flex-1 p-4 bg-black/20 rounded-lg border border-gray-800/50 flex items-center justify-between">
              <span class="text-[10px] font-bold text-gray-500 uppercase">TOTAL3 Trend</span>
              <div class="flex items-center gap-2">
                <TrendingUp v-if="market.marketIndices.total3_btc_trend === 'UP'" class="w-4 h-4 text-green-500" />
                <TrendingDown v-else class="w-4 h-4 text-red-500" />
                <span :class="market.marketIndices.total3_btc_trend === 'UP' ? 'text-green-500' : 'text-red-500'" 
                      class="text-sm font-bold">{{ market.marketIndices.total3_btc_trend }}</span>
              </div>
            </div>
            <div class="flex-1 p-4 bg-black/20 rounded-lg border border-gray-800/50 flex items-center justify-between">
              <span class="text-[10px] font-bold text-gray-500 uppercase">BTC Dominance</span>
              <div class="flex items-center gap-2">
                <TrendingUp v-if="market.marketIndices.btc_d_trend === 'UP'" class="w-4 h-4 text-red-500" />
                <TrendingDown v-else class="w-4 h-4 text-green-500" />
                <span :class="market.marketIndices.btc_d_trend === 'UP' ? 'text-red-500' : 'text-green-500'" 
                      class="text-sm font-bold">{{ market.marketIndices.btc_d_trend }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Right Column: Logs & Risk -->
      <div class="col-span-12 lg:col-span-4 space-y-6">
        
        <!-- Macro Events & Risk -->
        <div class="bg-[#1e2329] p-5 rounded-xl border border-gray-800">
          <div class="flex items-center gap-2 mb-4">
            <ShieldCheck class="w-5 h-5 text-green-400" />
            <h2 class="text-sm font-bold uppercase tracking-wider">Risk Monitor</h2>
          </div>
          
          <div class="space-y-4">
            <div class="flex items-center justify-between p-3 bg-black/20 rounded-lg border border-gray-800">
              <div class="flex items-center gap-2">
                <Clock class="w-4 h-4 text-gray-500" />
                <span class="text-xs text-gray-400 font-medium">Next Event</span>
              </div>
              <span class="text-xs font-bold text-yellow-500">FOMC Meeting</span>
            </div>
            
            <div class="flex items-center justify-between p-3 bg-black/20 rounded-lg border border-gray-800">
              <span class="text-xs text-gray-400 font-medium">Liquidation Alert</span>
              <span class="text-[10px] px-2 py-0.5 bg-green-500/10 text-green-500 rounded border border-green-500/20 font-bold uppercase tracking-tighter">Normal</span>
            </div>
          </div>
        </div>

        <!-- Confirm Stream -->
        <div class="bg-[#1e2329] rounded-xl border border-gray-800 flex flex-col h-[400px]">
          <div class="p-4 border-b border-gray-800 flex justify-between items-center bg-black/10 rounded-t-xl">
            <h2 class="text-xs font-bold uppercase tracking-wider text-gray-400">Confirmation Stream</h2>
            <span class="text-[10px] text-gray-600 font-mono">Live</span>
          </div>
          <div class="p-2 overflow-y-auto flex-1 font-mono text-[10px] space-y-1">
            <div v-for="(log, idx) in market.logs" :key="idx" 
                 class="px-2 py-1 rounded hover:bg-white/5 transition-colors border-l-2 border-green-500/50 bg-green-500/5">
              <span class="text-green-500 mr-2">✓</span>
              <span class="text-gray-300">{{ log }}</span>
            </div>
            <div v-if="market.logs.length === 0" class="text-center py-20 text-gray-600 italic">
              Waiting for candle closures...
            </div>
          </div>
        </div>

      </div>
    </div>
  </div>
</template>

<style>
body {
  background-color: #0b0e11;
  margin: 0;
}

::-webkit-scrollbar {
  width: 4px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: #333;
  border-radius: 10px;
}

::-webkit-scrollbar-thumb:hover {
  background: #444;
}
</style>
