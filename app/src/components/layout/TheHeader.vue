<script setup lang="ts">
import { LayoutDashboard, Database } from '@lucide/vue';

defineProps<{
  currentView: string;
}>();

const emit = defineEmits<{
  (e: 'navigate', view: string): void;
}>();
</script>

<template>
  <header class="flex items-center justify-between mb-8 pb-4 border-b border-gray-800">
    <div class="flex items-center gap-3">
      <div class="p-2 bg-yellow-500/10 rounded-lg">
        <LayoutDashboard class="w-6 h-6 text-yellow-500" />
      </div>
      <div>
        <h1 class="text-xl font-bold tracking-tight">BinanceTraderTool <span class="text-yellow-500 text-sm ml-1">V2</span></h1>
        <p class="text-xs text-gray-500 uppercase tracking-widest">Phase 0: Data Pipeline Monitor</p>
      </div>
    </div>
    
    <div class="flex items-center gap-6">
      <nav class="flex items-center gap-4 bg-gray-900/50 p-1 rounded-lg border border-gray-800">
        <button 
          @click="emit('navigate', 'dashboard')"
          :class="['px-4 py-1.5 rounded-md text-sm font-medium transition-colors flex items-center gap-2', 
                   currentView === 'dashboard' ? 'bg-gray-800 text-white shadow-sm' : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/50']"
        >
          <LayoutDashboard class="w-4 h-4" />
          Dashboard
        </button>
        <button 
          @click="emit('navigate', 'admin')"
          :class="['px-4 py-1.5 rounded-md text-sm font-medium transition-colors flex items-center gap-2', 
                   currentView === 'admin' ? 'bg-gray-800 text-white shadow-sm' : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800/50']"
        >
          <Database class="w-4 h-4" />
          Admin & DB
        </button>
      </nav>

      <div class="flex items-center gap-2 px-3 py-1.5 bg-green-500/10 rounded-full border border-green-500/20">
        <div class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
        <span class="text-xs font-medium text-green-500">WebSocket Connected</span>
      </div>
    </div>
  </header>
</template>
