<script setup lang="ts">
import { Brain, Zap, Activity, Info } from '@lucide/vue';
import type { MarketRegimeContext } from '../../types/market';
import { VolatilityRegime } from '../../types/market';

defineProps<{
  regime: MarketRegimeContext;
}>();

defineEmits(['show-glossary']);

const getTrendColor = (score: number) => {
  if (score >= 80) return 'text-blue-500';
  if (score >= 40) return 'text-blue-400';
  return 'text-gray-500';
};

const getFlowColor = (score: number) => {
  if (score >= 80) return 'text-green-500';
  if (score >= 50) return 'text-green-400';
  return 'text-red-500';
};

const getVolatilityColor = (regime: VolatilityRegime) => {
  if (regime === VolatilityRegime.Compression) return 'text-blue-400';
  if (regime === VolatilityRegime.Expansion) return 'text-yellow-500';
  return 'text-red-500 animate-pulse';
};
</script>

<template>
  <div class="bg-[#1e2329] p-6 rounded-xl border border-gray-800 relative overflow-hidden">
    <!-- Background Icon Decoration -->
    <Brain class="absolute -right-4 -bottom-4 w-32 h-32 text-white/5 rotate-12" />

    <div class="flex items-center justify-between mb-6">
      <div class="flex items-center gap-2">
        <div class="p-2 bg-purple-500/10 rounded-lg">
          <Brain class="w-5 h-5 text-purple-400" />
        </div>
        <h2 class="text-sm font-bold uppercase tracking-wider text-gray-300">Market Regime Engine</h2>
        <button @click="$emit('show-glossary')" class="p-1 hover:bg-white/5 rounded transition-colors text-gray-500 hover:text-yellow-500">
          <Info class="w-4 h-4" />
        </button>
      </div>
      
      <div class="flex items-center gap-3">
        <span class="text-[10px] font-bold uppercase text-gray-500">Scanner Gateway:</span>
        <div :class="regime.allow_alt_scan ? 'bg-green-500 shadow-[0_0_10px_rgba(34,197,94,0.5)]' : 'bg-red-500'" 
             class="w-3 h-3 rounded-full transition-all duration-500"></div>
        <span :class="regime.allow_alt_scan ? 'text-green-500' : 'text-red-500'" class="text-[10px] font-bold uppercase">
          {{ regime.allow_alt_scan ? 'Enabled' : 'Blocked' }}
        </span>
      </div>
    </div>

    <div class="grid grid-cols-12 gap-6 relative z-10">
      <!-- Market Score -->
      <div class="col-span-12 md:col-span-5 grid grid-cols-2 gap-4">
        <div class="flex flex-col items-center justify-center p-4 bg-black/20 rounded-xl border border-gray-800/50">
          <span class="text-[10px] font-bold text-gray-500 uppercase mb-2">Trend Score</span>
          <div :class="getTrendColor(regime.trend_score)" class="text-4xl font-black font-mono">
            {{ regime.trend_score }}
          </div>
          <div class="w-full h-1 bg-gray-800 rounded-full mt-4 overflow-hidden">
            <div class="h-full transition-all duration-1000 bg-blue-500" 
                 :style="{ width: `${regime.trend_score}%` }"></div>
          </div>
        </div>
        <div class="flex flex-col items-center justify-center p-4 bg-black/20 rounded-xl border border-gray-800/50">
          <span class="text-[10px] font-bold text-gray-500 uppercase mb-2">Flow Score</span>
          <div :class="getFlowColor(regime.flow_score)" class="text-4xl font-black font-mono">
            {{ regime.flow_score }}
          </div>
          <div class="w-full h-1 bg-gray-800 rounded-full mt-4 overflow-hidden">
            <div class="h-full transition-all duration-1000 bg-green-500" 
                 :style="{ width: `${regime.flow_score}%` }"></div>
          </div>
        </div>
      </div>

      <!-- Analysis Details -->
      <div class="col-span-12 md:col-span-7 grid grid-cols-2 gap-4">
        <div class="p-3 bg-black/20 rounded-lg border border-gray-800/30">
          <p class="text-[9px] font-bold text-gray-500 uppercase mb-1">Macro Trend (1D)</p>
          <p class="text-sm font-bold text-gray-200">{{ regime.structural_trend }}</p>
        </div>
        <div class="p-3 bg-black/20 rounded-lg border border-gray-800/30">
          <p class="text-[9px] font-bold text-gray-500 uppercase mb-1">Micro State (4H)</p>
          <p class="text-sm font-bold text-gray-200">{{ regime.operational_state }}</p>
        </div>
        <div class="p-3 bg-black/20 rounded-lg border border-gray-800/30">
          <p class="text-[9px] font-bold text-gray-500 uppercase mb-1 flex items-center gap-1">
            <Activity class="w-2.5 h-2.5" /> Volatility
          </p>
          <p class="text-sm font-bold" :class="getVolatilityColor(regime.volatility_regime)">{{ regime.volatility_regime }}</p>
        </div>
        <div class="p-3 bg-black/20 rounded-lg border border-gray-800/30">
          <p class="text-[9px] font-bold text-gray-500 uppercase mb-1 flex items-center gap-1">
            <Zap class="w-2.5 h-2.5" /> OI State
          </p>
          <p class="text-sm font-bold text-orange-400 uppercase italic">{{ regime.oi_state }}</p>
        </div>
      </div>
    </div>

    <!-- Action Mode Banner -->
    <div class="mt-6 p-4 rounded-xl border flex items-center justify-between group transition-all"
         :class="{
           'bg-green-500/10 border-green-500/30 text-green-500': (regime.action_mode || '').includes('Long'),
           'bg-red-500/10 border-red-500/30 text-red-500': (regime.action_mode || '').includes('Short'),
           'bg-yellow-500/10 border-yellow-500/30 text-yellow-500': regime.action_mode === 'Mean_Reversion',
           'bg-gray-500/10 border-gray-800 text-gray-500': !regime.action_mode || regime.action_mode === 'Off_System'
         }">
      <div class="flex items-center gap-3">
        <Zap class="w-5 h-5 fill-current" />
        <div>
          <p class="text-[9px] font-black uppercase tracking-tighter opacity-70">Current Action Mode</p>
          <h3 class="text-lg font-black italic uppercase leading-none">{{ regime.action_mode }}</h3>
        </div>
      </div>
      <div v-if="regime.allow_alt_scan" class="flex items-center gap-1 px-3 py-1 bg-green-500 text-black rounded text-[10px] font-black animate-bounce">
        AUTO-SCAN ACTIVE
      </div>
    </div>
  </div>
</template>
