<script setup lang="ts">
import { ShieldCheck, Clock, Activity, Target } from '@lucide/vue';
import type { Microstructure } from '../../types/market';
import { formatNum } from '../../composables/useFormat';

defineProps<{
  nextEvent: string;
  microstructure: Microstructure | null;
}>();
</script>

<template>
  <div class="bg-[#1e2329] p-5 rounded-xl border border-gray-800">
    <div class="flex items-center gap-2 mb-4">
      <ShieldCheck class="w-5 h-5 text-green-400" />
      <h2 class="text-sm font-bold uppercase tracking-wider">Risk & Order Flow</h2>
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
      <div v-if="microstructure" class="space-y-2">
        <div class="flex items-center justify-between p-3 bg-black/20 rounded-lg border border-gray-800">
          <div class="flex items-center gap-2">
            <Activity class="w-4 h-4 text-gray-500" />
            <span class="text-xs text-gray-400 font-medium">CVD (Aggression)</span>
          </div>
          <span :class="microstructure.cvd > 0 ? 'text-green-500' : 'text-red-500'" class="text-xs font-bold font-mono">
            {{ formatNum(microstructure.cvd) }}
          </span>
        </div>

        <div class="p-3 bg-black/20 rounded-lg border border-gray-800 space-y-3">
          <div class="flex items-center gap-2">
            <Target class="w-4 h-4 text-gray-500" />
            <span class="text-xs text-gray-400 font-medium">Liquidation Heatmap</span>
          </div>
          <div class="flex justify-between items-center text-[10px]">
            <span class="text-red-400 uppercase font-bold tracking-wider">Upper Cluster</span>
            <span class="font-mono text-gray-300 font-bold">${{ formatNum(microstructure.liquidation_upper_cluster) }}</span>
          </div>
          <div class="flex justify-between items-center text-[10px]">
            <span class="text-green-400 uppercase font-bold tracking-wider">Lower Cluster</span>
            <span class="font-mono text-gray-300 font-bold">${{ formatNum(microstructure.liquidation_lower_cluster) }}</span>
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
