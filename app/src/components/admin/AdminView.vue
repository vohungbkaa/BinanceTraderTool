<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Types matching the Rust backend
interface UniverseCandidate {
  symbol: string;
  quote_volume: number;
  open_interest: number;
  volatility: number;
  funding_rate_abs: number;
  vol_score: number;
  oi_score: number;
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
  oi: '',
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
  } catch (e) {
    // If parsing fails, fallback to string includes
  }
  
  return String(val).toLowerCase().includes(lowerStr);
};

// Computed filtered list for Top 100
const filteredTopAltcoins = computed(() => {
  return topAltcoins.value.filter((coin, index) => {
    // Global search
    if (searchTop100.value && !coin.symbol.toLowerCase().includes(searchTop100.value.toLowerCase())) return false;
    
    // Column filters
    if (filterTop100.value.rank && !String(index + 1).includes(filterTop100.value.rank)) return false;
    if (filterTop100.value.symbol && !coin.symbol.toLowerCase().includes(filterTop100.value.symbol.toLowerCase())) return false;
    if (!numFilter(coin.composite_score, filterTop100.value.score)) return false;
    if (!numFilter(coin.quote_volume, filterTop100.value.volume)) return false;
    if (!numFilter(coin.open_interest, filterTop100.value.oi)) return false;
    if (!numFilter(coin.volatility, filterTop100.value.volatility)) return false;
    if (!numFilter(coin.funding_rate_abs, filterTop100.value.funding)) return false;
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
    if (!numFilter(row.candle.taker_buy_volume, filterCandles.value.takerBuy)) return false;
    
    if (!numFilter(row.indicators?.ema20, filterCandles.value.ema20)) return false;
    if (!numFilter(row.indicators?.ema50, filterCandles.value.ema50)) return false;
    if (!numFilter(row.indicators?.ema200, filterCandles.value.ema200)) return false;
    if (!numFilter(row.indicators?.atr14, filterCandles.value.atr14)) return false;
    if (!numFilter(row.indicators?.adx14, filterCandles.value.adx14)) return false;
    
    if (filterCandles.value.structure && !(row.indicators?.structure || 'None').toLowerCase().includes(filterCandles.value.structure.toLowerCase())) return false;

    if (!numFilter(row.microstructure?.oi_change_4h_pct, filterCandles.value.oi)) return false;
    if (!numFilter(row.microstructure?.funding_rate_avg, filterCandles.value.funding)) return false;
    if (!numFilter(row.microstructure?.cvd_4h, filterCandles.value.cvd)) return false;
    
    if (filterCandles.value.liq) {
      const hasSurge = row.microstructure?.liquidation_surge_detected ? 'yes' : '-';
      if (!hasSurge.includes(filterCandles.value.liq.toLowerCase())) return false;
    }

    if (!numFilter(row.market_indices?.market_breadth_pct_above_ema50, filterCandles.value.bEma50)) return false;
    if (!numFilter(row.market_indices?.market_breadth_pct_above_ema200, filterCandles.value.bEma200)) return false;
    if (!numFilter(row.range_24h_pct * 100, filterCandles.value.range)) return false;

    return true;
  });
});

const loadTopAltcoins = async () => {
  isLoadingTop100.value = true;
  try {
    const data = await invoke<UniverseCandidate[]>('get_top_altcoins_metadata');
    topAltcoins.value = data;
  } catch (error) {
    console.error('Failed to load top altcoins metadata:', error);
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

// Debounce helper for text change search (DB fetch)
let debounceTimeout: number | undefined;
watch(() => candleForm.value.symbol, () => {
  clearTimeout(debounceTimeout);
  debounceTimeout = window.setTimeout(() => {
    loadDbCandles();
  }, 500);
});

watch(() => candleForm.value.timeframe, () => {
  loadDbCandles();
});

watch(() => candleForm.value.limit, () => {
  clearTimeout(debounceTimeout);
  debounceTimeout = window.setTimeout(() => {
    loadDbCandles();
  }, 500);
});

// Utilities
const formatCurrency = (val: number) => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0 }).format(val);
const formatNumber = (val: number | null | undefined) => {
  if (val === null || val === undefined) return 'N/A';
  return new Intl.NumberFormat('en-US', { maximumFractionDigits: 4 }).format(val);
};
const formatPct = (val: number | null | undefined) => {
  if (val === null || val === undefined) return 'N/A';
  return (val).toFixed(2) + '%';
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
          <h3 class="text-lg font-semibold text-white">
            Top 100 Scanning Universe 
            <span class="text-sm font-normal text-gray-500 ml-2">({{ filteredTopAltcoins.length }} / {{ topAltcoins.length }} records)</span>
          </h3>
          <p class="text-sm text-gray-400">Verifying filtering criteria (Min-Max Normalization 0-100 scale. Sweet Spot = High Vol/OI, Low ATR/Funding).</p>
        </div>
        <div class="flex gap-3">
          <input 
            v-model="searchTop100" 
            type="text" 
            placeholder="Global search..." 
            class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-48 p-1.5"
          >
          <button @click="loadTopAltcoins" class="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 text-sm rounded border border-gray-700 flex items-center gap-2">
            <span v-if="isLoadingTop100">Loading...</span>
            <span v-else>Refresh Data</span>
          </button>
        </div>
      </div>

      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[600px] relative">
        <table class="w-full text-sm text-left text-gray-300 relative whitespace-nowrap">
          <thead class="text-xs text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-10">
            <!-- Header Names -->
            <tr>
              <th scope="col" class="px-4 py-2 border-b border-gray-800 sticky left-0 z-20 bg-gray-900">Rank</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800 sticky left-[60px] z-20 bg-gray-900 shadow-[1px_0_0_#1f2937]">Symbol</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800 text-yellow-500 font-bold">Composite Score</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">24h Quote Vol (USDT)</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">Open Interest (USDT)</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">Volatility (Proxy ATR)</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">Funding Rate (Abs)</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">Last Price</th>
              <th scope="col" class="px-4 py-2 border-b border-gray-800">24h Change</th>
            </tr>
            <!-- Filter Inputs -->
            <tr class="bg-gray-800/80 border-b border-gray-800">
              <th class="p-1 sticky left-0 z-20 bg-gray-800"><input v-model="filterTop100.rank" class="w-12 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="<10" /></th>
              <th class="p-1 sticky left-[60px] z-20 bg-gray-800 shadow-[1px_0_0_#1f2937]"><input v-model="filterTop100.symbol" class="w-20 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="Filter..." /></th>
              <th class="p-1"><input v-model="filterTop100.score" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder=">80" /></th>
              <th class="p-1"><input v-model="filterTop100.volume" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="e.g. >10000000" /></th>
              <th class="p-1"><input v-model="filterTop100.oi" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder=">20000000" /></th>
              <th class="p-1"><input v-model="filterTop100.volatility" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="<0.05" /></th>
              <th class="p-1"><input v-model="filterTop100.funding" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterTop100.price" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="<1" /></th>
              <th class="p-1"><input v-model="filterTop100.change" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder=">5" /></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(coin, index) in filteredTopAltcoins" :key="coin.symbol" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-4 py-2 font-medium text-gray-500 sticky left-0 bg-[#12161a] z-10">#{{ index + 1 }}</td>
              <td class="px-4 py-2 font-bold text-white sticky left-[60px] bg-[#12161a] z-10 shadow-[1px_0_0_#1f2937]">{{ coin.symbol }}</td>
              <td class="px-4 py-2 text-xl font-bold text-yellow-500 bg-yellow-500/5 rounded-md text-center">{{ coin.composite_score.toFixed(1) }}</td>
              <td class="px-4 py-2 text-blue-400">
                <div>{{ formatCurrency(coin.quote_volume) }}</div>
                <div class="text-[10px] text-gray-500 mt-0.5">Score: <span class="text-blue-500">{{ coin.vol_score.toFixed(1) }}</span></div>
              </td>
              <td class="px-4 py-2 text-purple-400">
                <div>{{ formatCurrency(coin.open_interest) }}</div>
                <div class="text-[10px] text-gray-500 mt-0.5">Score: <span class="text-purple-500">{{ coin.oi_score.toFixed(1) }}</span></div>
              </td>
              <td class="px-4 py-2 text-indigo-400">
                <div>{{ (coin.volatility * 100).toFixed(2) }}%</div>
                <div class="text-[10px] text-gray-500 mt-0.5" title="Inverse scale: High score = Low volatility (Compression)">Score: <span class="text-indigo-500">{{ coin.atr_score.toFixed(1) }}</span></div>
              </td>
              <td class="px-4 py-2 text-pink-400">
                <div>{{ (coin.funding_rate_abs * 100).toFixed(4) }}%</div>
                <div class="text-[10px] text-gray-500 mt-0.5" title="Inverse scale: High score = Near zero (Balanced)">Score: <span class="text-pink-500">{{ coin.fund_score.toFixed(1) }}</span></div>
              </td>
              <td class="px-4 py-2">{{ formatNumber(coin.last_price) }}</td>
              <td :class="['px-4 py-2 font-medium', coin.price_change_percent >= 0 ? 'text-green-500' : 'text-red-500']">
                {{ coin.price_change_percent > 0 ? '+' : '' }}{{ coin.price_change_percent.toFixed(2) }}%
              </td>
            </tr>
            <tr v-if="filteredTopAltcoins.length === 0 && !isLoadingTop100">
              <td colspan="9" class="px-6 py-8 text-center text-gray-500">No data available.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Tab: Database Explorer -->
    <div v-if="activeTab === 'candles'" class="space-y-4">
      <div class="flex justify-between items-end">
        <div>
          <h3 class="text-lg font-semibold text-white">
            Closed Candles (Local Database)
            <span class="text-sm font-normal text-gray-500 ml-2">({{ filteredDbCandles.length }} / {{ dbCandlesTotal }} records)</span>
          </h3>
          <p class="text-sm text-gray-400">Querying historical data directly from SQLite. Data updates automatically as you type.</p>
        </div>
      </div>

      <!-- Filters -->
      <div class="flex gap-4 bg-gray-900/50 p-4 rounded-lg border border-gray-800 items-end">
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Symbol (Auto-search DB)</label>
          <input v-model="candleForm.symbol" type="text" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-40 p-2" placeholder="e.g. BTCUSDT">
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Timeframe</label>
          <select v-model="candleForm.timeframe" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-24 p-2">
            <option value="15m">15m</option>
            <option value="1h">1h</option>
            <option value="4h">4h</option>
            <option value="1d">1d</option>
          </select>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1">Limit (DB)</label>
          <input v-model.number="candleForm.limit" type="number" class="bg-gray-800 border border-gray-700 text-white text-sm rounded-md focus:ring-blue-500 focus:border-blue-500 block w-24 p-2">
        </div>
        <div class="ml-2 text-sm text-gray-500" v-if="isLoadingCandles">
          <span class="animate-pulse">Loading...</span>
        </div>
        <div class="ml-auto flex items-center">
           <button @click="loadDbCandles" class="px-4 py-2 bg-gray-800 hover:bg-gray-700 text-white text-sm font-medium rounded-md border border-gray-700 transition-colors">
            Force Refresh
          </button>
        </div>
      </div>

      <!-- Table with Extended Columns -->
      <div class="overflow-x-auto rounded-lg border border-gray-800 h-[600px] relative">
        <table class="w-full text-xs text-left text-gray-300 relative whitespace-nowrap">
          <thead class="text-gray-400 uppercase bg-gray-900 border-b border-gray-800 sticky top-0 z-30">
            <!-- Header Names -->
            <tr class="bg-gray-900 border-b border-gray-800">
              <th scope="col" class="px-3 py-2 bg-gray-900 sticky left-0 z-20">Symbol</th>
              <th scope="col" class="px-3 py-2 bg-gray-900 sticky left-[80px] z-20 shadow-[1px_0_0_#1f2937]">Open Time</th>
              
              <!-- Price Action -->
              <th scope="col" class="px-3 py-2 border-l border-gray-800">Open</th>
              <th scope="col" class="px-3 py-2">High</th>
              <th scope="col" class="px-3 py-2">Low</th>
              <th scope="col" class="px-3 py-2">Close</th>
              <th scope="col" class="px-3 py-2">Volume</th>
              <th scope="col" class="px-3 py-2">Taker Buy Vol</th>
              
              <!-- Indicators -->
              <th scope="col" class="px-3 py-2 border-l border-gray-800">EMA20</th>
              <th scope="col" class="px-3 py-2">EMA50</th>
              <th scope="col" class="px-3 py-2">EMA200</th>
              <th scope="col" class="px-3 py-2">ATR14</th>
              <th scope="col" class="px-3 py-2">ADX14</th>
              <th scope="col" class="px-3 py-2">Structure</th>
              
              <!-- Microstructure -->
              <th scope="col" class="px-3 py-2 border-l border-gray-800">OI Chg 4H</th>
              <th scope="col" class="px-3 py-2">Funding Rate</th>
              <th scope="col" class="px-3 py-2">CVD 4H</th>
              <th scope="col" class="px-3 py-2">Liq Surge</th>
              
              <!-- Market/Ranges -->
              <th scope="col" class="px-3 py-2 border-l border-gray-800">Breadth EMA50</th>
              <th scope="col" class="px-3 py-2">Breadth EMA200</th>
              <th scope="col" class="px-3 py-2">Range 24h</th>
            </tr>
            
            <!-- Filter Inputs -->
            <tr class="bg-gray-800/80 border-b border-gray-800">
              <th class="p-1 bg-gray-800 sticky left-0 z-20"><input v-model="filterCandles.symbol" class="w-20 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="Filter..." /></th>
              <th class="p-1 bg-gray-800 sticky left-[80px] z-20 shadow-[1px_0_0_#1f2937]"><input v-model="filterCandles.openTime" class="w-full bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 focus:ring-1 focus:ring-blue-500 outline-none" placeholder="Filter..." /></th>
              
              <th class="p-1"><input v-model="filterCandles.open" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder=">1000" /></th>
              <th class="p-1"><input v-model="filterCandles.high" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.low" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.close" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.volume" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.takerBuy" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              
              <th class="p-1"><input v-model="filterCandles.ema20" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.ema50" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.ema200" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.atr14" class="w-12 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.adx14" class="w-12 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.structure" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              
              <th class="p-1"><input v-model="filterCandles.oi" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder=">0" /></th>
              <th class="p-1"><input v-model="filterCandles.funding" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="<0" /></th>
              <th class="p-1"><input v-model="filterCandles.cvd" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.liq" class="w-12 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="yes" /></th>
              
              <th class="p-1"><input v-model="filterCandles.bEma50" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder=">50" /></th>
              <th class="p-1"><input v-model="filterCandles.bEma200" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder="Filter" /></th>
              <th class="p-1"><input v-model="filterCandles.range" class="w-16 bg-gray-900/50 border border-gray-700/50 text-gray-300 text-[10px] rounded px-1.5 py-1 outline-none" placeholder=">2" /></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in filteredDbCandles" :key="row.candle.open_time + row.candle.symbol" class="border-b border-gray-800 hover:bg-gray-800/50">
              <td class="px-3 py-2 font-bold text-white bg-[#12161a] sticky left-0 z-10">{{ row.candle.symbol }}</td>
              <td class="px-3 py-2 font-mono bg-[#12161a] sticky left-[80px] z-10 shadow-[1px_0_0_#1f2937]">{{ formatDate(row.candle.open_time) }}</td>
              
              <!-- Price Action -->
              <td class="px-3 py-2 border-l border-gray-800 text-gray-400">{{ formatNumber(row.candle.open) }}</td>
              <td class="px-3 py-2 text-green-400">{{ formatNumber(row.candle.high) }}</td>
              <td class="px-3 py-2 text-red-400">{{ formatNumber(row.candle.low) }}</td>
              <td class="px-3 py-2 font-bold text-white">{{ formatNumber(row.candle.close) }}</td>
              <td class="px-3 py-2">{{ formatNumber(row.candle.volume) }}</td>
              <td class="px-3 py-2 text-blue-400">{{ formatNumber(row.candle.taker_buy_volume) }}</td>
              
              <!-- Indicators -->
              <td class="px-3 py-2 border-l border-gray-800 text-yellow-600">{{ formatNumber(row.indicators?.ema20) }}</td>
              <td class="px-3 py-2 text-yellow-500">{{ formatNumber(row.indicators?.ema50) }}</td>
              <td class="px-3 py-2 text-yellow-300">{{ formatNumber(row.indicators?.ema200) }}</td>
              <td class="px-3 py-2 text-purple-400">{{ formatNumber(row.indicators?.atr14) }}</td>
              <td class="px-3 py-2 text-indigo-400">{{ formatNumber(row.indicators?.adx14) }}</td>
              <td class="px-3 py-2 text-gray-400">{{ row.indicators?.structure || 'None' }}</td>
              
              <!-- Microstructure -->
              <td :class="['px-3 py-2 border-l border-gray-800 font-medium', (row.microstructure?.oi_change_4h_pct || 0) > 0 ? 'text-green-500' : 'text-red-500']">{{ formatPct(row.microstructure?.oi_change_4h_pct) }}</td>
              <td class="px-3 py-2 text-pink-400">{{ formatNumber(row.microstructure?.funding_rate_avg) }}</td>
              <td class="px-3 py-2 text-blue-300">{{ formatNumber(row.microstructure?.cvd_4h) }}</td>
              <td class="px-3 py-2">
                <span v-if="row.microstructure?.liquidation_surge_detected" class="px-1.5 py-0.5 bg-red-500/20 text-red-400 rounded text-[10px]">Yes</span>
                <span v-else class="text-gray-600">-</span>
              </td>
              
              <!-- Market/Ranges -->
              <td class="px-3 py-2 border-l border-gray-800 text-teal-400">{{ formatPct(row.market_indices?.market_breadth_pct_above_ema50) }}</td>
              <td class="px-3 py-2 text-teal-500">{{ formatPct(row.market_indices?.market_breadth_pct_above_ema200) }}</td>
              <td class="px-3 py-2 text-orange-400">{{ formatPct(row.range_24h_pct * 100) }}</td>
            </tr>
            <tr v-if="filteredDbCandles.length === 0 && !isLoadingCandles">
              <td colspan="21" class="px-6 py-8 text-center text-gray-500">No candles match the current filters.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
