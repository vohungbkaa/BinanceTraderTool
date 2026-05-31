<script setup lang="ts">
import { ShieldCheck, Clock } from '@lucide/vue';

defineProps<{
  nextEvent: string;
  liquidationStatus: 'Normal' | 'Warning' | 'Critical';
}>();
</script>

<template>
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
        <span class="text-xs font-bold text-yellow-500">{{ nextEvent }}</span>
      </div>
      
      <div class="flex items-center justify-between p-3 bg-black/20 rounded-lg border border-gray-800">
        <span class="text-xs text-gray-400 font-medium">Liquidation Alert</span>
        <span :class="{
          'bg-green-500/10 text-green-500 border-green-500/20': liquidationStatus === 'Normal',
          'bg-yellow-500/10 text-yellow-500 border-yellow-500/20': liquidationStatus === 'Warning',
          'bg-red-500/10 text-red-500 border-red-500/20': liquidationStatus === 'Critical'
        }" class="text-[10px] px-2 py-0.5 rounded border font-bold uppercase tracking-tighter">
          {{ liquidationStatus }}
        </span>
      </div>
    </div>
  </div>
</template>
