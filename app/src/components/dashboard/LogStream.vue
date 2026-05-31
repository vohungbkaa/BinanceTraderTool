<script setup lang="ts">
defineProps<{
  logs: string[];
}>();
</script>

<template>
  <div class="bg-[#1e2329] rounded-xl border border-gray-800 flex flex-col h-[400px]">
    <div class="p-4 border-b border-gray-800 flex justify-between items-center bg-black/10 rounded-t-xl">
      <h2 class="text-xs font-bold uppercase tracking-wider text-gray-400">Confirmation Stream</h2>
      <span class="text-[10px] text-gray-600 font-mono">Live</span>
    </div>
    <div class="p-2 overflow-y-auto flex-1 font-mono text-[10px] space-y-1">
      <div v-for="(log, idx) in logs" :key="idx" 
           class="px-2 py-1 rounded hover:bg-white/5 transition-colors border-l-2 border-green-500/50 bg-green-500/5">
        <span class="text-green-500 mr-2">✓</span>
        <span class="text-gray-300">{{ log }}</span>
      </div>
      <div v-if="logs.length === 0" class="text-center py-20 text-gray-600 italic">
        Waiting for candle closures...
      </div>
    </div>
  </div>
</template>
