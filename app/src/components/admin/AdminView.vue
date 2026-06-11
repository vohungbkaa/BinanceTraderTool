<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Types matching the Rust backend
interface AltcoinMetadata {
  symbol: string;
  quote_volume: number;
  price_change_percent: number;
  last_price: number;
}

interface NormalizedCandleData {
  timestamp: number;
  candle: {
    symbol: string;
    timeframe: string;
    open_time: number;
    close_time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
    quote_asset_volume: number;
    number_of_trades: number;
    taker_buy_volume: number;
    taker_buy_quote_volume: number;
    is_closed: boolean;
  };
  indicators: any;
  market_indices: any;
  microstructure: any;
  macro_events: any;
}

const activeTab = ref<'top100' | 'candles'>('top100');

// Data for Top 100
const topAltcoins = ref<AltcoinMetadata[]>([]);
const isLoadingTop100 = ref(false);
const searchTop100 = ref('');

// Computed filtered list for Top 100
const filteredTopAltcoins = computed(() => {
  if (!searchTop100.value) return topAltcoins.value;
  const q = searchTop100.value.toUpperCase();
  return topAltcoins.value.filter(coin => coin.symbol.includes(q));
});

// Data for Candles
const dbCandles = ref<NormalizedCandleData[]>([]);
const isLoadingCandles = ref(false);
const candleForm = ref({
  symbol: 'BTCUSDT',
  timeframe: '4h',
  limit: 100
});

const loadTopAltcoins = async () => {
  isLoadingTop100.value = true;
  try {
    const data = await invoke<AltcoinMetadata[]>('get_top_altcoins_metadata');
    topAltcoins.value = data;
  } catch (error) {
    console.error('Failed to load top altcoins metadata:', error);
  } finally {
    isLoadingTop100.value = false;
  }
};

const loadDbCandles = async () => {
  isLoadingCandles.value = true;
  try {
    const data = await invoke<NormalizedCandleData[]>('get_db_candles', {
      symbol: candleForm.value.symbol.toUpperCase(),
      timeframe: candleForm.value.timeframe,
      limit: candleForm.value.limit
    });
    dbCandles.value = data;
  } catch (error) {
    console.error('Failed to load DB candles:', error);
    dbCandles.value = [];
  } finally {
    isLoadingCandles.value = false;
  }
};

// Utilities
const formatCurrency = (val: number) => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0 }).format(val);
const formatNumber = (val: number) => new Intl.NumberFormat('en-US', { maximumFractionDigits: 4 }).format(val);
const formatDate = (ms: number) => new Date(ms).toLocaleString();

onMounted(() => {
  loadTopAltcoins();
  loadDbCandles();
});
</script>

<template>
  <div class="bg-[#12161a] rounded-xl border border-gray-800 p-6 min-h-[80vh]">
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-2xl font-bold text-white">System Administration</h2>
      
      <div class="flex gap-2 bg-gray-900 p-1 rounded-lg border border-gray-800">
        <button 
          @click="activeTab = 'top100'"
          :class="['px-4 py-2 rounded-md text-sm font-medium transition-colors', activeTab === 'top100' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-gray-200']"
        >
          Top 100 Altcoins Criteria
        </button>
        <button 
          @click="activeTab = 'candles'"
          :class="['px-4 py-2 rounded-md text-sm font-medium transition-colors', activeTab === 'candles' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-gray-200']"
        >
          Database Explorer
        </button>
      </div>
    </div>

    <!-- Tab: Top 100 Altcoins Criteria -->
    <div v-if="activeTab === 'top100'" class="space-y-4">
      <div class="flex justify-between items-end">
        <div>
          <h3 class="text-lg font-semibold text-white">Top 100 Scanning Universe</h3>
          <p class="text-sm text-gray-400">Verifying filtering criteria (Volume > 5M USDT, no stablecoins).</p>
        </div>
        <div class="flex gap-3">
          <input 
            v-model="searchTop100" 
            type="text" 
            placeholder="Search symbol..." 
            class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-48 p-1.5"
          >
          <button @click="loadTopAltcoins" class="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-sm rounded border border-gray-700 flex items-center gap-2">
            <span v-if="isLoadingTop100">Loading...</span>
            <span v-else>Refresh Data</span>
          </button>
        </div>
      </div>

      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[500px] relative">
        <table class="w-full text-sm text-left text-gray-300 relative">
          <thead class="text-xs text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-10">
            <tr>
              <th scope="col" class="px-6 py-3">Rank</th>
              <th scope="col" class="px-6 py-3">Symbol</th>
              <th scope="col" class="px-6 py-3">24h Quote Volume (USDT)</th>
              <th scope="col" class="px-6 py-3">Last Price</th>
              <th scope="col" class="px-6 py-3">24h Change</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(coin, index) in filteredTopAltcoins" :key="coin.symbol" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-6 py-3 font-medium text-gray-500">#{{ index + 1 }}</td>
              <td class="px-6 py-3 font-bold text-white">{{ coin.symbol }}</td>
              <td class="px-6 py-3 text-blue-400">{{ formatCurrency(coin.quote_volume) }}</td>
              <td class="px-6 py-3">{{ formatNumber(coin.last_price) }}</td>
              <td :class="['px-6 py-3 font-medium', coin.price_change_percent >= 0 ? 'text-green-500' : 'text-red-500']">
                {{ coin.price_change_percent > 0 ? '+' : '' }}{{ coin.price_change_percent.toFixed(2) }}%
              </td>
            </tr>
            <tr v-if="filteredTopAltcoins.length === 0 && !isLoadingTop100">
              <td colspan="5" class="px-6 py-8 text-center text-gray-500">No data available.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Tab: Database Explorer -->
    <div v-if="activeTab === 'candles'" class="space-y-4">
      <div class="flex justify-between items-end">
        <div>
          <h3 class="text-lg font-semibold text-white">Closed Candles (Local Database)</h3>
          <p class="text-sm text-gray-400">Querying historical data directly from SQLite.</p>
        </div>
      </div>

      <!-- Filters -->
      <div class="flex gap-4 bg-gray-900/50 p-4 rounded-lg border border-gray-800">
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Symbol</label>
          <input v-model="candleForm.symbol" type="text" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-full p-2" placeholder="BTCUSDT">
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Timeframe</label>
          <select v-model="candleForm.timeframe" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-full p-2">
            <option value="15m">15m</option>
            <option value="1h">1h</option>
            <option value="4h">4h</option>
            <option value="1d">1d</option>
          </select>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Limit</label>
          <input v-model.number="candleForm.limit" type="number" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-full p-2">
        </div>
        <div class="flex items-end">
          <button @click="loadDbCandles" class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors">
            <span v-if="isLoadingCandles">Querying...</span>
            <span v-else>Query DB</span>
          </button>
        </div>
      </div>

      <!-- Table -->
      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[500px] relative">
        <table class="w-full text-sm text-left text-gray-300 relative">
          <thead class="text-xs text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-10">
            <tr>
              <th scope="col" class="px-4 py-3">Open Time</th>
              <th scope="col" class="px-4 py-3">Open</th>
              <th scope="col" class="px-4 py-3">High</th>
              <th scope="col" class="px-4 py-3">Low</th>
              <th scope="col" class="px-4 py-3">Close</th>
              <th scope="col" class="px-4 py-3">Volume</th>
              <th scope="col" class="px-4 py-3">EMA 50</th>
              <th scope="col" class="px-4 py-3">ATR 14</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, index) in dbCandles" :key="row.candle.open_time" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-4 py-2 whitespace-nowrap">{{ formatDate(row.candle.open_time) }}</td>
              <td class="px-4 py-2 text-gray-400">{{ formatNumber(row.candle.open) }}</td>
              <td class="px-4 py-2 text-green-400">{{ formatNumber(row.candle.high) }}</td>
              <td class="px-4 py-2 text-red-400">{{ formatNumber(row.candle.low) }}</td>
              <td class="px-4 py-2 font-medium text-white">{{ formatNumber(row.candle.close) }}</td>
              <td class="px-4 py-2">{{ formatNumber(row.candle.volume) }}</td>
              <td class="px-4 py-2 text-yellow-500">{{ row.indicators?.ema50 ? formatNumber(row.indicators.ema50) : 'N/A' }}</td>
              <td class="px-4 py-2 text-purple-400">{{ row.indicators?.atr14 ? formatNumber(row.indicators.atr14) : 'N/A' }}</td>
            </tr>
            <tr v-if="dbCandles.length === 0 && !isLoadingCandles">
              <td colspan="8" class="px-6 py-8 text-center text-gray-500">No candles found in local DB for this symbol/timeframe.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
