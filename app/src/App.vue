<template>
  <div class="min-h-screen bg-gray-900 text-white flex">
    <!-- Sidebar -->
    <aside class="w-64 bg-gray-800 p-4">
      <h1 class="text-xl font-bold mb-8 text-blue-400">Binance Bot V2</h1>
      <nav class="space-y-2">
        <a href="#" class="block p-2 rounded hover:bg-gray-700">Dashboard</a>
        <a href="#" class="block p-2 rounded hover:bg-gray-700">Watchlist</a>
        <a href="#" class="block p-2 rounded hover:bg-gray-700">Settings</a>
      </nav>
    </aside>

    <!-- Main Content -->
    <main class="flex-1 p-6">
      <header class="flex justify-between items-center mb-6">
        <h2 class="text-2xl font-semibold">Trading Dashboard</h2>
        <div class="bg-gray-800 px-4 py-2 rounded">Status: <span class="text-green-500">Connected</span></div>
      </header>
      
      <!-- Chart Area Placeholder -->
      <div class="bg-gray-800 h-96 rounded flex items-center justify-center border border-gray-700">
        <p class="text-gray-400">Trading Chart Area</p>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
// Logic sẽ được thêm ở đây sau
</script>
