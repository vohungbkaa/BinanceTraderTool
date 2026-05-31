<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useMarketStore } from './stores/market';
import TheHeader from './components/layout/TheHeader.vue';
import BTCCard from './components/dashboard/BTCCard.vue';
import MarketBreadth from './components/dashboard/MarketBreadth.vue';
import MarketRegime from './components/dashboard/MarketRegime.vue';
import RiskMonitor from './components/dashboard/RiskMonitor.vue';
import LogStream from './components/dashboard/LogStream.vue';
import GlossaryModal from './components/common/GlossaryModal.vue';

const market = useMarketStore();
const isGlossaryOpen = ref(false);

onMounted(() => {
  market.init();
});
</script>

<template>
  <div class="min-h-screen bg-[#0b0e11] text-[#eaecef] font-sans p-6">
    <TheHeader />

    <div class="grid grid-cols-12 gap-6">
      <!-- Left Column: BTC Benchmark & Breadth -->
      <div class="col-span-12 lg:col-span-8 space-y-6">
        
        <!-- Market Regime Center (NEW Phase 1) -->
        <MarketRegime 
          :regime="market.regime" 
          @show-glossary="isGlossaryOpen = true"
        />

        <!-- BTC Live Cards (Dynamic) -->
        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          <BTCCard 
            v-for="tf in market.timeframes" 
            :key="tf" 
            :tf="tf" 
            :data="market.btcData[tf]" 
          />
        </div>

        <MarketBreadth :indices="market.marketIndices" />
      </div>

      <!-- Right Column: Logs & Risk -->
      <div class="col-span-12 lg:col-span-4 space-y-6">
        <RiskMonitor 
          next-event="FOMC Meeting" 
          liquidation-status="Normal" 
        />
        
        <LogStream :logs="market.logs" />
      </div>
    </div>

    <!-- Modals -->
    <GlossaryModal 
      :show="isGlossaryOpen" 
      @close="isGlossaryOpen = false" 
    />
  </div>
</template>

<style>
body {
  background-color: #0b0e11;
  margin: 0;
}

::-webkit-scrollbar {
  width: 4px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: #333;
  border-radius: 10px;
}

::-webkit-scrollbar-thumb:hover {
  background: #444;
}
</style>
