<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Types matching the Rust backend
interface UniverseCandidate {
  symbol: string;
  quote_volume: number;
  volume_change_24h_pct: number;
  open_interest: number;
  oi_change_24h_pct: number;
  volatility: number;
  funding_rate: number;
  vol_score: number;
  vol_change_score: number;
  oi_score: number;
  oi_change_score: number;
  atr_score: number;
  fund_score: number;
  composite_score: number;
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
    quote_volume: number;
    taker_buy_volume: number;
    is_closed: boolean;
  };
  indicators: {
    ema20: number | null;
    ema50: number | null;
    ema200: number | null;
    atr14: number | null;
    adx14: number | null;
    plus_di: number | null;
    minus_di: number | null;
    structure: string;
    close_above_ema200_count: number;
    ema50_slope: number;
  };
  market_indices: {
    btc_d_trend: string;
    total3_btc_trend: string;
    market_breadth_pct_above_ema50: number;
    market_breadth_pct_above_ema200: number;
  };
  microstructure: {
    oi_change_4h_pct: number;
    price_change_4h_pct: number;
    funding_rate_avg: number;
    cvd_4h: number;
    cvd_1d: number;
    liquidation_surge_detected: boolean;
  };
  macro_events: any;
  range_24h_pct: number;
  range_p40_90d: number;
  atr_surge_ratio: number;
}

const activeTab = ref<'top100' | 'candles'>('top100');

// Data for Top 100
const topAltcoins = ref<UniverseCandidate[]>([]);
const isLoadingTop100 = ref(false);
const searchTop100 = ref(''); // Global search

// Column-specific filters for Top 100
const filterTop100 = ref({
  rank: '',
  symbol: '',
  score: '',
  volume: '',
  vol_growth: '',
  oi: '',
  oi_change: '',
  volatility: '',
  funding: '',
  price: '',
  change: ''
});

// Helper for numerical advanced filtering (supports >, <, >=, <=, =)
const numFilter = (val: number | null | undefined, filterStr: string) => {
  if (!filterStr) return true;
  if (val === null || val === undefined) return false;
  
  const str = filterStr.trim();
  const lowerStr = str.toLowerCase();
  
  try {
    if (str.startsWith('>=')) return val >= parseFloat(str.slice(2));
    if (str.startsWith('<=')) return val <= parseFloat(str.slice(2));
    if (str.startsWith('>')) return val > parseFloat(str.slice(1));
    if (str.startsWith('<')) return val < parseFloat(str.slice(1));
    if (str.startsWith('=')) return val === parseFloat(str.slice(1));
  } catch (e) {}
  
  return String(val).toLowerCase().includes(lowerStr);
};

// Computed filtered list for Top 100
const filteredTopAltcoins = computed(() => {
  return topAltcoins.value.filter((coin, index) => {
    const rankNum = index + 1;
    if (searchTop100.value && !coin.symbol.toLowerCase().includes(searchTop100.value.toLowerCase())) return false;
    
    if (!numFilter(rankNum, filterTop100.value.rank)) return false;
    if (filterTop100.value.symbol && !coin.symbol.toLowerCase().includes(filterTop100.value.symbol.toLowerCase())) return false;
    if (!numFilter(coin.composite_score, filterTop100.value.score)) return false;
    if (!numFilter(coin.quote_volume, filterTop100.value.volume)) return false;
    if (!numFilter(coin.volume_change_24h_pct, filterTop100.value.vol_growth)) return false;
    if (!numFilter(coin.open_interest, filterTop100.value.oi)) return false;
    if (!numFilter(coin.oi_change_24h_pct, filterTop100.value.oi_change)) return false;
    if (!numFilter(coin.volatility, filterTop100.value.volatility)) return false;
    if (!numFilter(coin.funding_rate, filterTop100.value.funding)) return false;
    if (!numFilter(coin.last_price, filterTop100.value.price)) return false;
    if (!numFilter(coin.price_change_percent, filterTop100.value.change)) return false;
    
    return true;
  });
});

// Data for Candles
interface DbCandlesResponse {
  data: NormalizedCandleData[];
  total: number;
}
const dbCandles = ref<NormalizedCandleData[]>([]);
const dbCandlesTotal = ref(0);
const isLoadingCandles = ref(false);
const candleForm = ref({
  symbol: 'BTCUSDT',
  timeframe: '4h',
  limit: 100
});

// Column-specific filters for Candles
const filterCandles = ref({
  symbol: '', openTime: '', open: '', high: '', low: '', close: '', volume: '', takerBuy: '',
  ema20: '', ema50: '', ema200: '', atr14: '', adx14: '', structure: '',
  oi: '', funding: '', cvd: '', liq: '',
  bEma50: '', bEma200: '', range: ''
});

const filteredDbCandles = computed(() => {
  return dbCandles.value.filter(row => {
    if (filterCandles.value.symbol && !row.candle.symbol.toLowerCase().includes(filterCandles.value.symbol.toLowerCase())) return false;
    if (filterCandles.value.openTime && !formatDate(row.candle.open_time).includes(filterCandles.value.openTime)) return false;
    if (!numFilter(row.candle.open, filterCandles.value.open)) return false;
    if (!numFilter(row.candle.high, filterCandles.value.high)) return false;
    if (!numFilter(row.candle.low, filterCandles.value.low)) return false;
    if (!numFilter(row.candle.close, filterCandles.value.close)) return false;
    if (!numFilter(row.candle.volume, filterCandles.value.volume)) return false;
    if (!numFilter(row.indicators?.ema20, filterCandles.value.ema20)) return false;
    if (!numFilter(row.indicators?.ema50, filterCandles.value.ema50)) return false;
    if (!numFilter(row.indicators?.ema200, filterCandles.value.ema200)) return false;
    if (!numFilter(row.indicators?.atr14, filterCandles.value.atr14)) return false;
    if (!numFilter(row.indicators?.adx14, filterCandles.value.adx14)) return false;
    if (filterCandles.value.structure && !(row.indicators?.structure || 'None').toLowerCase().includes(filterCandles.value.structure.toLowerCase())) return false;
    return true;
  });
});

const loadTopAltcoins = async (forceRefresh = false) => {
  isLoadingTop100.value = true;
  try {
    if (forceRefresh) {
      const data = await invoke<UniverseCandidate[]>("get_top_altcoins_metadata");
      topAltcoins.value = data;
    } else {
      const data = await invoke<UniverseCandidate[]>("get_stored_universe");
      if (data && data.length > 0) {
        topAltcoins.value = data;
      } else {
        const freshData = await invoke<UniverseCandidate[]>("get_top_altcoins_metadata");
        topAltcoins.value = freshData;
      }
    }
  } catch (error) {
    console.error("Failed to load top altcoins metadata:", error);
  } finally {
    isLoadingTop100.value = false;
  }
};

const loadDbCandles = async () => {
  if (!candleForm.value.symbol) return;
  isLoadingCandles.value = true;
  try {
    const response = await invoke<DbCandlesResponse>('get_db_candles', {
      symbol: candleForm.value.symbol.toUpperCase(),
      timeframe: candleForm.value.timeframe,
      limit: candleForm.value.limit
    });
    dbCandles.value = response.data;
    dbCandlesTotal.value = response.total;
  } catch (error) {
    console.error('Failed to load DB candles:', error);
    dbCandles.value = [];
    dbCandlesTotal.value = 0;
  } finally {
    isLoadingCandles.value = false;
  }
};

// Utilities
const formatCurrency = (val: number) => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0 }).format(val);
const formatNumber = (val: number | null | undefined) => {
  if (val === null || val === undefined) return 'N/A';
  return new Intl.NumberFormat('en-US', { maximumFractionDigits: 4 }).format(val);
};
const formatPct = (val: number | null | undefined) => {
  if (val === null || val === undefined || isNaN(val)) return '0.00%';
  return (val * 100).toFixed(2) + '%';
};
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
        <button @click="activeTab = 'top100'" :class="['px-4 py-2 rounded-md text-sm font-medium transition-colors', activeTab === 'top100' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-gray-200']">Top 100 Altcoins Criteria</button>
        <button @click="activeTab = 'candles'" :class="['px-4 py-2 rounded-md text-sm font-medium transition-colors', activeTab === 'candles' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-gray-200']">Database Explorer</button>
      </div>
    </div>

    <!-- Tab: Top 100 Altcoins Criteria -->
    <div v-if="activeTab === 'top100'" class="space-y-4">
      <div class="flex justify-between items-end">
        <div>
          <h3 class="text-lg font-semibold text-white">Top 100 Scanning Universe <span class="text-sm font-normal text-gray-500 ml-2">({{ filteredTopAltcoins.length }} / {{ topAltcoins.length }} records)</span></h3>
          <p class="text-sm text-gray-400">Verifying filtering criteria (Weights: 25% Vol, 10% Vol Growth, 20% OI, 15% OI Growth, 20% ATR, 10% Funding).</p>
        </div>
        <div class="flex gap-3">
          <input v-model="searchTop100" type="text" placeholder="Global search..." class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-48 p-1.5">
          <button @click="loadTopAltcoins(true)" class="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-sm rounded border border-gray-700 flex items-center gap-2">
            <span v-if="isLoadingTop100">Loading...</span>
            <span v-else>Force Refresh API</span>
          </button>
        </div>
      </div>

      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[650px] relative">
        <table class="w-full text-sm text-left text-gray-300 relative whitespace-nowrap table-fixed">
          <thead class="text-xs text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-40">
            <!-- Header Names -->
            <tr>
              <th scope="col" class="w-[60px] px-3 py-3 sticky top-0 left-0 z-50 bg-gray-900 border-b border-gray-800">Rank</th>
              <th scope="col" class="w-[130px] px-3 py-3 sticky top-0 left-[60px] z-50 bg-gray-900 border-b border-gray-800 shadow-[1px_0_0_#1f2937]">Symbol</th>
              <th scope="col" class="w-[90px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800 text-yellow-500 font-bold text-center">Score</th>
              <th scope="col" class="w-[160px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800">24h Vol (USDT)</th>
              <th scope="col" class="w-[110px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800 text-blue-400">Vol Growth</th>
              <th scope="col" class="w-[160px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800">Open Interest</th>
              <th scope="col" class="w-[110px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800 text-purple-400">OI Change</th>
              <th scope="col" class="w-[110px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800">Volatility</th>
              <th scope="col" class="w-[110px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800">Funding</th>
              <th scope="col" class="w-[120px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800">Last Price</th>
              <th scope="col" class="w-[100px] px-3 py-3 sticky top-0 z-40 bg-gray-900 border-b border-gray-800 text-right">24h Change</th>
            </tr>
            <!-- Filter Inputs -->
            <tr class="bg-gray-800 sticky top-[44px] z-40 border-b border-gray-700">
              <th class="px-1 py-1 sticky left-0 z-50 bg-gray-800"><input v-model="filterTop100.rank" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder="<10"></th>
              <th class="px-1 py-1 sticky left-[60px] z-50 bg-gray-800 shadow-[1px_0_0_#1f2937]"><input v-model="filterTop100.symbol" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder="Filter..."></th>
              <th class="px-1 py-1"><input v-model="filterTop100.score" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none text-center" placeholder=">80"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.volume" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder=">10M"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.vol_growth" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder=">20"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.oi" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder=">20M"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.oi_change" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder=">10"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.volatility" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder="<0.05"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.funding" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder="Filter"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.price" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder="<1"></th>
              <th class="px-1 py-1"><input v-model="filterTop100.change" class="w-full bg-gray-900/50 border border-gray-700 text-gray-300 text-[10px] rounded px-1 py-1 outline-none" placeholder=">5"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(coin, index) in filteredTopAltcoins" :key="coin.symbol" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-3 py-2 font-medium text-gray-500 sticky left-0 bg-[#12161a] z-10">#{{ topAltcoins.indexOf(coin) + 1 }}</td>
              <td class="px-3 py-2 font-bold text-white sticky left-[60px] bg-[#12161a] z-10 shadow-[1px_0_0_#1f2937]">{{ coin.symbol }}</td>
              <td class="px-3 py-2 text-lg font-bold text-yellow-500 bg-yellow-500/5 text-center">{{ coin.composite_score.toFixed(1) }}</td>
              <td class="px-3 py-2 text-blue-400"><div class="truncate">{{ formatCurrency(coin.quote_volume) }}</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.vol_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-blue-300"><div :class="coin.volume_change_24h_pct >= 0 ? 'text-green-400' : 'text-red-400'">{{ coin.volume_change_24h_pct > 0 ? '+' : '' }}{{ coin.volume_change_24h_pct.toFixed(1) }}%</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.vol_change_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-purple-400"><div class="truncate">{{ formatCurrency(coin.open_interest) }}</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.oi_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-purple-300"><div :class="coin.oi_change_24h_pct >= 0 ? 'text-green-400' : 'text-red-400'">{{ coin.oi_change_24h_pct > 0 ? '+' : '' }}{{ coin.oi_change_24h_pct.toFixed(1) }}%</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.oi_change_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-indigo-400"><div>{{ (coin.volatility * 100).toFixed(2) }}%</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.atr_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-pink-400"><div>{{ formatPct(coin.funding_rate) }}</div><div class="text-[10px] text-gray-500 italic">Score: {{ coin.fund_score.toFixed(1) }}</div></td>
              <td class="px-3 py-2 text-gray-300">{{ formatNumber(coin.last_price) }}</td>
              <td :class="['px-3 py-2 font-medium text-right', coin.price_change_percent >= 0 ? 'text-green-500' : 'text-red-500']">{{ coin.price_change_percent > 0 ? '+' : '' }}{{ coin.price_change_percent.toFixed(2) }}%</td>
            </tr>
            <tr v-if="filteredTopAltcoins.length === 0 && !isLoadingTop100"><td colspan="11" class="px-6 py-8 text-center text-gray-500">No data available.</td></tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Tab: Database Explorer -->
    <div v-if="activeTab === 'candles'" class="space-y-4">
       <!-- Giữ nguyên code Database Explorer cũ nhưng thêm sticky tương tự nếu cần -->
       <div class="flex justify-between items-end">
        <div>
          <h3 class="text-lg font-semibold text-white">Closed Candles (Local Database) <span class="text-sm font-normal text-gray-500 ml-2">({{ filteredDbCandles.length }} / {{ dbCandlesTotal }} records)</span></h3>
          <p class="text-sm text-gray-400">Querying historical data directly from SQLite.</p>
        </div>
      </div>
      <div class="flex gap-4 bg-gray-900/50 p-4 rounded-lg border border-gray-800 items-end">
        <div><label class="block text-xs font-medium text-gray-400 mb-1">Symbol</label><input v-model="candleForm.symbol" type="text" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md block w-40 p-2"></div>
        <div><label class="block text-xs font-medium text-gray-400 mb-1">Timeframe</label><select v-model="candleForm.timeframe" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md block w-24 p-2"><option value="15m">15m</option><option value="4h">4h</option><option value="1d">1d</option></select></div>
        <div><label class="block text-xs font-medium text-gray-400 mb-1">Limit</label><input v-model.number="candleForm.limit" type="number" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md block w-24 p-2"></div>
        <button @click="loadDbCandles" class="px-4 py-2 bg-gray-800 hover:bg-gray-700 text-white text-sm rounded-md border border-gray-700">Refresh</button>
      </div>
      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[600px] relative">
        <table class="w-full text-xs text-left text-gray-300 relative whitespace-nowrap table-fixed">
          <thead class="text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-40">
            <tr>
              <th scope="col" class="w-[100px] px-3 py-2 sticky top-0 left-0 z-50 bg-gray-900">Symbol</th>
              <th scope="col" class="w-[180px] px-3 py-2 sticky top-0 left-[100px] z-50 bg-gray-900 shadow-[1px_0_0_#1f2937]">Open Time</th>
              <th scope="col" class="w-[100px] px-3 py-2 sticky top-0 z-40 bg-gray-900 text-right">Close</th>
              <th scope="col" class="w-[120px] px-3 py-2 sticky top-0 z-40 bg-gray-900 text-right">Volume</th>
              <th scope="col" class="w-[100px] px-3 py-2 sticky top-0 z-40 bg-gray-900 text-right">EMA50</th>
              <th scope="col" class="w-[100px] px-3 py-2 sticky top-0 z-40 bg-gray-900 text-right">EMA200</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in filteredDbCandles" :key="row.candle.open_time + row.candle.symbol" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-3 py-2 font-bold text-white bg-[#12161a] sticky left-0 z-10">{{ row.candle.symbol }}</td>
              <td class="px-3 py-2 font-mono bg-[#12161a] sticky left-[100px] z-10 shadow-[1px_0_0_#1f2937] text-[10px]">{{ formatDate(row.candle.open_time) }}</td>
              <td class="px-3 py-2 text-right font-bold text-white">{{ formatNumber(row.candle.close) }}</td>
              <td class="px-3 py-2 text-right">{{ formatNumber(row.candle.volume) }}</td>
              <td class="px-3 py-2 text-right text-yellow-500">{{ formatNumber(row.indicators?.ema50) }}</td>
              <td class="px-3 py-2 text-right text-yellow-300">{{ formatNumber(row.indicators?.ema200) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
