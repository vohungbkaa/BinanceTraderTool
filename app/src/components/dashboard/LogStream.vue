<script setup lang="ts">
import type { ChecklistItem } from '../../types/market';
import { computed } from 'vue';

const props = defineProps<{
  logs: string[];
  checklist?: ChecklistItem[];
}>();

const groupedChecklist = computed(() => {
  if (!props.checklist) return {};
  return props.checklist.reduce((acc: any, item) => {
    if (!acc[item.group]) acc[item.group] = [];
    acc[item.group].push(item);
    return acc;
  }, {});
});
</script>

<template>
  <div class="bg-[#1e2329] rounded-xl border border-gray-800 flex flex-col h-[400px]">
    <div class="p-4 border-b border-gray-800 flex justify-between items-center bg-black/10 rounded-t-xl">
      <h2 class="text-xs font-bold uppercase tracking-wider text-gray-400">Confirmation Stream</h2>
      <span class="text-[10px] text-gray-600 font-mono">Live</span>
    </div>
    
    <!-- Checklist Section -->
    <div v-if="checklist && checklist.length > 0" class="p-4 border-b border-gray-800/50 space-y-4 bg-black/5 overflow-y-auto max-h-[220px]">
      <div v-for="(items, group) in groupedChecklist" :key="group" class="space-y-1">
        <h3 class="text-[8px] font-black text-gray-600 uppercase tracking-widest mb-2 px-1 border-l border-gray-700">{{ group }}</h3>
        <div v-for="(item, idx) in items" :key="idx" 
             class="flex items-center justify-between text-[10px] font-mono px-1">
          <span :class="item.status ? 'text-gray-300' : 'text-gray-500'">{{ item.label }}</span>
          <span :class="item.status ? 'text-green-500' : 'text-red-500'" class="font-bold">
            {{ item.status ? '[✓]' : '[✗]' }}
          </span>
        </div>
      </div>
    </div>

    <!-- Logs Section -->
    <div class="p-2 overflow-y-auto flex-1 font-mono text-[9px] space-y-1">
      <div v-for="(log, idx) in logs" :key="idx" 
           class="px-2 py-1 rounded hover:bg-white/5 transition-colors border-l-2 border-blue-500/30 bg-blue-500/5">
        <span class="text-blue-500/50 mr-2">></span>
        <span class="text-gray-400">{{ log }}</span>
      </div>
      <div v-if="logs.length === 0 && (!checklist || checklist.length === 0)" class="text-center py-20 text-gray-600 italic text-[10px]">
        Waiting for candle closures...
      </div>
    </div>
  </div>
</template>
