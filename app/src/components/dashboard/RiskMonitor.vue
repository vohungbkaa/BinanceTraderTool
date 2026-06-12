<script setup lang="ts">
import { ShieldCheck, Clock, Activity, Target, Loader2 } from '@lucide/vue';
import type { Microstructure } from '../../types/market';
import { formatNum } from '../../composables/useFormat';

defineProps<{
  nextEvent: string;
  microstructure: Microstructure | null;
  isLoading?: boolean;
}>();
</script>

<template>
  <div class="bg-[#1e2329] p-5 rounded-xl border border-gray-800">
    <div class="flex items-center gap-2 mb-4">
      <ShieldCheck class="w-5 h-5 text-green-400" />
      <h2 class="text-sm font-bold uppercase tracking-wider">Risk & Order Flow</h2>
      <div v-if="isLoading" class="ml-auto flex items-center gap-2 text-[10px] font-bold uppercase text-yellow-500">
        <Loader2 class="w-3 h-3 animate-spin" />
        Waiting
      </div>
    </div>
    
    <div class="space-y-4">
      <div class="flex items-center justify-between p-3 bg-black/20 rounded-lg border border-gray-800">
        <div class="flex items-center gap-2">
          <Clock class="w-4 h-4 text-gray-500" />
          <span class="text-xs text-gray-400 font-medium">Next Event</span>
        </div>
        <span class="text-xs font-bold text-yellow-500">{{ nextEvent }}</span>
      </div>
      
      <!-- Order Flow Metrics -->
      <div v-if="isLoading" class="rounded-lg border border-yellow-500/20 bg-yellow-500/10 px-4 py-3 text-xs font-semibold text-yellow-500">
        Waiting for BTC order-flow context before showing liquidation and CVD metrics.
      </div>

      <div v-else-if="microstructure" class="space-y-2">
        <div class="grid grid-cols-2 gap-2">
          <div class="p-3 bg-black/20 rounded-lg border border-gray-800">
            <div class="flex items-center gap-2 mb-1">
              <Activity class="w-3 h-3 text-gray-500" />
              <span class="text-[10px] text-gray-400 font-bold uppercase">CVD Delta 4H</span>
            </div>
            <div :class="microstructure.cvd_4h > 0 ? 'text-green-500' : 'text-red-500'" class="text-xs font-bold font-mono">
              {{ microstructure.cvd_4h > 0 ? '+' : '' }}{{ formatNum(microstructure.cvd_4h) }}
            </div>
          </div>
          
          <div class="p-3 bg-black/20 rounded-lg border border-gray-800">
            <div class="flex items-center gap-2 mb-1">
              <Activity class="w-3 h-3 text-gray-500" />
              <span class="text-[10px] text-gray-400 font-bold uppercase">CVD Delta 1D</span>
            </div>
            <div :class="microstructure.cvd_1d > 0 ? 'text-green-500' : 'text-red-500'" class="text-xs font-bold font-mono">
              {{ microstructure.cvd_1d > 0 ? '+' : '' }}{{ formatNum(microstructure.cvd_1d) }}
            </div>
          </div>
        </div>

        <div class="p-3 bg-black/20 rounded-lg border border-gray-800 space-y-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <Target class="w-4 h-4 text-gray-500" />
              <span class="text-xs text-gray-400 font-medium">Liquidation Heatmap</span>
            </div>
            <span class="text-[9px] text-gray-500 uppercase font-bold">Real / Est</span>
          </div>
          
          <div class="space-y-1">
            <div class="flex justify-between items-center text-[10px]">
              <span class="text-red-400 uppercase font-bold tracking-wider">Upper Cluster</span>
              <div class="flex gap-2">
                <span class="font-mono text-gray-500 font-medium">${{ formatNum(microstructure.liquidation_upper_real) }}</span>
                <span class="font-mono text-gray-300 font-bold">${{ formatNum(microstructure.liquidation_upper_est) }}</span>
              </div>
            </div>
            <div class="flex justify-between items-center text-[10px]">
              <span class="text-green-400 uppercase font-bold tracking-wider">Lower Cluster</span>
              <div class="flex gap-2">
                <span class="font-mono text-gray-500 font-medium">${{ formatNum(microstructure.liquidation_lower_real) }}</span>
                <span class="font-mono text-gray-300 font-bold">${{ formatNum(microstructure.liquidation_lower_est) }}</span>
              </div>
            </div>
          </div>
          
          <div class="mt-2 pt-2 border-t border-gray-800/50 flex justify-between items-center text-[10px]">
             <span class="text-gray-500 uppercase font-bold tracking-wider">Cascade Risk</span>
             <span :class="microstructure.liquidation_surge_detected ? 'text-red-500' : 'text-green-500'" class="font-bold uppercase">
               {{ microstructure.liquidation_surge_detected ? 'CRITICAL' : 'NORMAL' }}
             </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
