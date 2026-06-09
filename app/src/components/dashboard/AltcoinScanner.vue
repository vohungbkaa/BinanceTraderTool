<script setup lang="ts">
import { Search, TrendingUp, TrendingDown, Info, ListFilter } from '@lucide/vue';
import type { ScanCandidate } from '../../types/scanner';
import { formatNum } from '../../composables/useFormat';

defineProps<{
  shortlist: ScanCandidate[];
  lastScanTime: number;
}>();

const getRatingClass = (rating: string) => {
  switch (rating) {
    case 'A': return 'bg-green-500/20 text-green-500 border-green-500/30';
    case 'B': return 'bg-blue-500/20 text-blue-500 border-blue-500/30';
    case 'C': return 'bg-gray-500/20 text-gray-500 border-gray-500/30';
    case 'D': return 'bg-red-500/20 text-red-500 border-red-500/30';
    default: return 'bg-gray-800 text-gray-400';
  }
};

const formatTime = (ts: number) => {
  if (ts === 0) return 'Never';
  return new Date(ts * 1000).toLocaleTimeString();
};
</script>

<template>
  <div class="bg-[#1e2329] rounded-xl border border-gray-800 overflow-hidden flex flex-col h-full">
    <!-- Header -->
    <div class="p-4 border-b border-gray-800 bg-black/10 flex justify-between items-center">
      <div class="flex items-center gap-2">
        <div class="p-1.5 bg-blue-500/10 rounded-lg">
          <Search class="w-4 h-4 text-blue-400" />
        </div>
        <h2 class="text-sm font-bold uppercase tracking-wider text-gray-300">Altcoin Scanner Shortlist</h2>
      </div>
      <div class="flex items-center gap-3">
        <span class="text-[10px] text-gray-500 font-medium italic">Last scan: {{ formatTime(lastScanTime) }}</span>
        <ListFilter class="w-4 h-4 text-gray-600" />
      </div>
    </div>

    <!-- Table -->
    <div class="flex-1 overflow-auto custom-scrollbar">
      <table class="w-full text-left border-collapse">
        <thead>
          <tr class="text-[10px] text-gray-500 uppercase tracking-tighter border-b border-gray-800/50 sticky top-0 bg-[#1e2329] z-10">
            <th class="px-4 py-3 font-black">Symbol</th>
            <th class="px-4 py-3 font-black text-center">Rating</th>
            <th class="px-4 py-3 font-black text-center">Direction</th>
            <th class="px-4 py-3 font-black text-right">Funding</th>
            <th class="px-4 py-3 font-black text-right">OI 4H</th>
            <th class="px-4 py-3 font-black text-right">RS Score</th>
            <th class="px-4 py-3 font-black text-right">Rank</th>
            <th class="px-4 py-3 font-black">Reason</th>
          </tr>
        </thead>
        <tbody class="text-xs font-medium">
          <tr v-for="alt in shortlist" :key="alt.symbol" 
              class="border-b border-gray-800/30 hover:bg-white/5 transition-colors group">
            <td class="px-4 py-4">
              <span class="font-bold text-gray-200 group-hover:text-yellow-500 transition-colors">{{ alt.symbol }}</span>
            </td>
            <td class="px-4 py-4 text-center">
              <span :class="getRatingClass(alt.rs_rating)" 
                    class="px-2 py-0.5 rounded border text-[10px] font-black">
                {{ alt.rs_rating }}
              </span>
            </td>
            <td class="px-4 py-4">
              <div class="flex items-center justify-center gap-1">
                <TrendingUp v-if="alt.direction === 'LONG'" class="w-3 h-3 text-green-500" />
                <TrendingDown v-else class="w-3 h-3 text-red-500" />
                <span :class="alt.direction === 'LONG' ? 'text-green-500' : 'text-red-500'" class="font-black text-[10px]">
                  {{ alt.direction }}
                </span>
              </div>
            </td>
            <td class="px-4 py-4 text-right font-mono" :class="alt.metrics.funding_rate < -0.05 ? 'text-red-400' : (alt.metrics.funding_rate > 0.05 ? 'text-green-400' : 'text-gray-400')">
              {{ formatNum(alt.metrics.funding_rate * 100) }}%
            </td>
            <td class="px-4 py-4 text-right font-mono" :class="alt.metrics.oi_growth_4h_pct > 0 ? 'text-green-400' : 'text-red-400'">
              <span v-if="alt.metrics.oi_growth_4h_pct > 0">+</span>{{ formatNum(alt.metrics.oi_growth_4h_pct) }}%
            </td>
            <td class="px-4 py-4 text-right font-mono text-gray-300">
              {{ formatNum(alt.rs_score) }}
            </td>
            <td class="px-4 py-4 text-right">
              <div class="inline-block px-2 py-0.5 bg-yellow-500/10 text-yellow-500 rounded font-mono font-bold">
                {{ formatNum(alt.rank_score) }}
              </div>
            </td>
            <td class="px-4 py-4 text-[10px] text-gray-500 max-w-[200px] truncate italic">
              {{ alt.reason }}
            </td>
          </tr>
          
          <!-- Empty State -->
          <tr v-if="shortlist.length === 0">
            <td colspan="8" class="py-20 text-center">
              <div class="flex flex-col items-center gap-3 opacity-20">
                <Search class="w-12 h-12" />
                <p class="text-sm font-bold uppercase tracking-widest">No active signals found</p>
                <p class="text-[10px]">Scanner is waiting for Phase 1 Green Light</p>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Footer Summary -->
    <div v-if="shortlist.length > 0" class="p-3 bg-black/10 border-t border-gray-800 flex items-center gap-2">
      <Info class="w-3.5 h-3.5 text-blue-500" />
      <p class="text-[9px] text-gray-500 font-bold uppercase tracking-tighter">
        Top {{ shortlist.length }} candidates selected based on Z-Score Relative Strength and context alignment.
      </p>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #333;
  border-radius: 10px;
}
</style>
