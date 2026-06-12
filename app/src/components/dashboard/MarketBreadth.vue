<script setup lang="ts">
import { BarChart3, TrendingUp, TrendingDown, Loader2 } from '@lucide/vue';
import type { MarketIndices } from '../../types/market';
import { formatNum } from '../../composables/useFormat';

defineProps<{
  indices: MarketIndices;
  isLoading?: boolean;
}>();
</script>

<template>
  <div class="bg-[#1e2329] p-6 rounded-xl border border-gray-800">
    <div class="flex items-center gap-2 mb-6">
      <BarChart3 class="w-5 h-5 text-blue-400" />
      <h2 class="text-sm font-bold uppercase tracking-wider">Market Breadth (Top 100 Altcoins)</h2>
      <div v-if="isLoading" class="ml-auto flex items-center gap-2 text-[10px] font-bold uppercase text-yellow-500">
        <Loader2 class="w-3 h-3 animate-spin" />
        Syncing breadth
      </div>
    </div>
    
    <div v-if="isLoading" class="mb-5 rounded-lg border border-yellow-500/20 bg-yellow-500/10 px-4 py-3 text-xs font-semibold text-yellow-500">
      Breadth values will update after the universe cache and EMA context are available.
    </div>

    <div class="grid grid-cols-2 gap-8" :class="{ 'opacity-40': isLoading }">
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
