<script setup lang="ts">
import { LayoutDashboard } from '@lucide/vue';
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
    
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-2 px-3 py-1.5 bg-green-500/10 rounded-full border border-green-500/20">
        <div class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
        <span class="text-xs font-medium text-green-500">WebSocket Connected</span>
      </div>
    </div>
  </header>
</template>
