<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useMarketStore } from './stores/market';
import TheHeader from './components/layout/TheHeader.vue';
import BTCCard from './components/dashboard/BTCCard.vue';
import MarketBreadth from './components/dashboard/MarketBreadth.vue';
import MarketRegime from './components/dashboard/MarketRegime.vue';
import RiskMonitor from './components/dashboard/RiskMonitor.vue';
import LogStream from './components/dashboard/LogStream.vue';
import AltcoinScanner from './components/dashboard/AltcoinScanner.vue';
import GlossaryModal from './components/common/GlossaryModal.vue';
import InitialSyncOverlay from './components/common/InitialSyncOverlay.vue';
import AdminView from './components/admin/AdminView.vue';

const market = useMarketStore();
const isGlossaryOpen = ref(false);
const currentView = ref('dashboard');

onMounted(() => {
  market.init();
});
</script>

<template>
  <div class="min-h-screen bg-[#0b0e11] text-[#eaecef] font-sans p-6">
    <TheHeader :current-view="currentView" @navigate="currentView = $event" />

    <div v-if="currentView === 'dashboard'" class="grid grid-cols-12 gap-6">
      <!-- Left Column: BTC Benchmark & Breadth -->
      <div class="col-span-12 lg:col-span-8 space-y-6">
        
        <!-- Market Regime Center (NEW Phase 1) -->
        <MarketRegime 
          :regime="market.regime" 
          :is-loading="market.isRegimeLoading"
          :missing-timeframes="market.missingRegimeTimeframes"
          @show-glossary="isGlossaryOpen = true"
        />

        <!-- BTC Live Cards (Dynamic) -->
        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          <BTCCard 
            v-for="tf in market.timeframes" 
            :key="tf" 
            :tf="tf" 
            :data="market.btcData[tf]" 
            :is-loading="market.isSystemSyncing || !market.btcData[tf]"
          />
        </div>

        <MarketBreadth 
          :indices="market.marketIndices" 
          :is-loading="market.isSystemSyncing || !market.hasBreadthData"
        />

        <!-- Altcoin Scanner Results (Phase 2) -->
        <AltcoinScanner 
          :shortlist="market.shortlist" 
          :last-scan-time="market.lastScanTime"
          :is-loading="market.isScannerLoading"
        />
      </div>

      <!-- Right Column: Logs & Risk -->
      <div class="col-span-12 lg:col-span-4 space-y-6">
        <RiskMonitor
          next-event="FOMC Meeting"
          :microstructure="market.btcData['15m']?.microstructure || market.btcData['4h']?.microstructure || null"
          :is-loading="market.isSystemSyncing || !market.hasRiskData"
        />
        
        <LogStream 
          :logs="market.logs" 
          :checklist="market.regime.checklist"
        />
      </div>
    </div>

    <!-- Admin / DB Explorer View -->
    <div v-else-if="currentView === 'admin'">
      <AdminView />
    </div>

    <InitialSyncOverlay :sync="market.syncProgress" />

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
