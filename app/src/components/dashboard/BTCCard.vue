<script setup lang="ts">
import { Activity } from '@lucide/vue';
import type { NormalizedCandleData } from '../../types/market';
import { formatNum, formatPrice } from '../../composables/useFormat';

defineProps<{
  tf: string;
  data?: NormalizedCandleData;
}>();
</script>

<template>
  <div class="bg-[#1e2329] p-5 rounded-xl border border-gray-800 hover:border-yellow-500/30 transition-colors">
    <div class="flex justify-between items-start mb-3">
      <span class="text-xs font-bold text-gray-400 uppercase">{{ tf }} BTCUSDT</span>
      <Activity class="w-4 h-4 text-yellow-500/50" />
    </div>
    <div class="text-2xl font-mono font-bold text-yellow-500">
      ${{ formatPrice(data?.candle.close) }}
    </div>
    <div class="mt-4 grid grid-cols-2 gap-2 text-[10px] text-gray-500 uppercase font-bold">
      <div>EMA50: <span class="text-gray-300 ml-1">{{ formatNum(data?.indicators.ema50) }}</span></div>
      <div>EMA200: <span class="text-gray-300 ml-1">{{ formatNum(data?.indicators.ema200) }}</span></div>
      <div>STR: <span class="text-yellow-500 ml-1">{{ data?.indicators.structure || 'NONE' }}</span></div>
      <div>ADX: <span class="text-gray-300 ml-1">{{ formatNum(data?.indicators.adx14) }}</span></div>
    </div>
  </div>
</template>
