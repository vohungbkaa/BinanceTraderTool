<script setup lang="ts">
import { Loader2 } from '@lucide/vue';
import type { SyncProgress } from '../../types/market';

defineProps<{
  sync: SyncProgress | null;
}>();
</script>

<template>
  <Transition name="fade">
    <div v-if="sync" class="fixed inset-0 z-[100] flex items-center justify-center bg-[#0b0e11]/90 backdrop-blur-sm">
      <div class="max-w-md w-full p-8 bg-[#1e2329] rounded-2xl border border-gray-800 shadow-2xl text-center space-y-6">
        <div class="relative flex justify-center">
          <Loader2 class="w-16 h-16 text-yellow-500 animate-spin" />
          <div class="absolute inset-0 flex items-center justify-center">
            <span class="text-[10px] font-black text-white">{{ Math.round(sync.progress) }}%</span>
          </div>
        </div>

        <div class="space-y-2">
          <h2 class="text-xl font-black uppercase tracking-tighter text-white">System Initializing</h2>
          <p class="text-sm text-gray-400 font-medium h-4">{{ sync.message }}</p>
        </div>

        <div class="w-full h-1.5 bg-gray-800 rounded-full overflow-hidden">
          <div 
            class="h-full bg-yellow-500 transition-all duration-300 ease-out shadow-[0_0_10px_rgba(234,179,8,0.3)]"
            :style="{ width: `${sync.progress}%` }"
          ></div>
        </div>

        <div class="grid grid-cols-4 gap-2">
          <div v-for="step in ['METADATA', 'WEBSOCKET', 'BREADTH', 'WARMUP']" :key="step"
               class="flex flex-col items-center gap-1">
            <div :class="[
                   'w-2 h-2 rounded-full',
                   sync.step === step || (sync.step === 'WARMUP' && step !== 'WARMUP') || sync.step === 'WARMUP_DONE' || sync.step === 'BREADTH_DONE' ? 'bg-green-500' : 'bg-gray-700'
                 ]"
                 class="transition-colors duration-500">
            </div>
            <span class="text-[8px] font-bold text-gray-500 uppercase">{{ step }}</span>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
