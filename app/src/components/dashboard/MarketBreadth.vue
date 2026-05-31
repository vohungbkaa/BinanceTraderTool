<script setup lang="ts">
import { BarChart3, TrendingUp, TrendingDown } from '@lucide/vue';
import type { MarketIndices } from '../../types/market';
import { formatNum } from '../../composables/useFormat';

defineProps<{
  indices: MarketIndices;
}>();
</script>

<template>
  <div class="bg-[#1e2329] p-6 rounded-xl border border-gray-800">
    <div class="flex items-center gap-2 mb-6">
      <BarChart3 class="w-5 h-5 text-blue-400" />
      <h2 class="text-sm font-bold uppercase tracking-wider">Market Breadth (Top 100 Altcoins)</h2>
    </div>
    
    <div class="grid grid-cols-2 gap-8">
      <div class="space-y-3">
        <div class="flex justify-between text-xs font-bold uppercase">
          <span class="text-gray-400">Above EMA50 (1D)</span>
          <span class="text-blue-400">{{ formatNum(indices.market_breadth_pct_above_ema50) }}%</span>
        </div>
        <div class="h-2 bg-gray-800 rounded-full overflow-hidden">
          <div class="h-full bg-blue-500 transition-all duration-500" 
               :style="{ width: `${indices.market_breadth_pct_above_ema50}%` }"></div>
        </div>
      </div>
      
      <div class="space-y-3">
        <div class="flex justify-between text-xs font-bold uppercase">
          <span class="text-gray-400">Above EMA200 (1D)</span>
          <span class="text-purple-400">{{ formatNum(indices.market_breadth_pct_above_ema200) }}%</span>
        </div>
        <div class="h-2 bg-gray-800 rounded-full overflow-hidden">
          <div class="h-full bg-purple-500 transition-all duration-500" 
               :style="{ width: `${indices.market_breadth_pct_above_ema200}%` }"></div>
        </div>
      </div>
    </div>

    <div class="mt-8 flex gap-4">
      <div class="flex-1 p-4 bg-black/20 rounded-lg border border-gray-800/50 flex items-center justify-between">
        <span class="text-[10px] font-bold text-gray-500 uppercase">TOTAL3 Trend</span>
        <div class="flex items-center gap-2">
          <TrendingUp v-if="indices.total3_btc_trend === 'UP'" class="w-4 h-4 text-green-500" />
          <TrendingDown v-else class="w-4 h-4 text-red-500" />
          <span :class="indices.total3_btc_trend === 'UP' ? 'text-green-500' : 'text-red-500'" 
                class="text-sm font-bold">{{ indices.total3_btc_trend }}</span>
        </div>
      </div>
      <div class="flex-1 p-4 bg-black/20 rounded-lg border border-gray-800/50 flex items-center justify-between">
        <span class="text-[10px] font-bold text-gray-500 uppercase">BTC Dominance</span>
        <div class="flex items-center gap-2">
          <TrendingUp v-if="indices.btc_d_trend === 'UP'" class="w-4 h-4 text-red-500" />
          <TrendingDown v-else class="w-4 h-4 text-green-500" />
          <span :class="indices.btc_d_trend === 'UP' ? 'text-red-500' : 'text-green-500'" 
                class="text-sm font-bold">{{ indices.btc_d_trend }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
