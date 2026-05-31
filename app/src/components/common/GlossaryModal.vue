<script setup lang="ts">
import { X, BookOpen, Info, Target, AlertTriangle } from '@lucide/vue';

defineProps<{
  show: boolean;
}>();

defineEmits(['close']);
</script>

<template>
  <Transition name="fade">
    <div v-if="show" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
      <div class="bg-[#1e2329] w-full max-w-4xl max-h-[80vh] rounded-2xl border border-gray-800 shadow-2xl flex flex-col overflow-hidden">
        <!-- Header -->
        <div class="p-6 border-b border-gray-800 flex justify-between items-center bg-black/20">
          <div class="flex items-center gap-3">
            <div class="p-2 bg-yellow-500/10 rounded-lg">
              <BookOpen class="w-6 h-6 text-yellow-500" />
            </div>
            <div>
              <h2 class="text-xl font-bold">Logic Engine Glossary</h2>
              <p class="text-xs text-gray-500 uppercase tracking-widest">Giải thích chi tiết các chỉ số bối cảnh thị trường</p>
            </div>
          </div>
          <button @click="$emit('close')" class="p-2 hover:bg-white/10 rounded-full transition-colors">
            <X class="w-6 h-6 text-gray-400" />
          </button>
        </div>

        <!-- Content -->
        <div class="p-6 overflow-y-auto space-y-10 custom-scrollbar">
          
          <!-- PHASE 0: DATA PIPELINE -->
          <section>
            <div class="flex items-center gap-2 mb-6 text-yellow-500 border-b border-yellow-500/20 pb-2">
              <Activity class="w-5 h-5" />
              <h3 class="font-black uppercase tracking-widest text-lg">PHASE 0: DATA PIPELINE & BENCHMARKS</h3>
            </div>
            
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
              <!-- Market Breadth -->
              <div class="space-y-3">
                <p class="text-blue-400 font-bold flex items-center gap-2 text-xs uppercase">
                  <BarChart3 class="w-4 h-4" /> Market Breadth (Độ rộng thị trường)
                </p>
                <p class="text-[11px] text-gray-300 leading-relaxed bg-blue-500/5 p-3 rounded-lg border border-blue-500/10">
                  Đo lường tỷ lệ % trong <strong>Top 100 Altcoin</strong> có giá nằm trên EMA50/EMA200. <br/>
                  • <strong>> 50%:</strong> Thị trường khỏe, dòng tiền lan tỏa. <br/>
                  • <strong>< 30%:</strong> Thị trường cực yếu, đa số coin đang bị bán tháo.
                </p>
              </div>

              <!-- Capital Flow -->
              <div class="space-y-3">
                <p class="text-orange-400 font-bold flex items-center gap-2 text-xs uppercase">
                  <TrendingUp class="w-4 h-4" /> Capital Flow (Dòng tiền)
                </p>
                <div class="grid grid-cols-1 gap-2 text-[11px]">
                  <div class="p-2 bg-black/20 rounded border border-gray-800">
                    <span class="text-orange-500 font-bold">BTC Dominance:</span> Nếu BTC.D giảm trong khi giá BTC tăng/ngang -> Tiền đang chảy mạnh vào Altcoin (Altcoin Season).
                  </div>
                  <div class="p-2 bg-black/20 rounded border border-gray-800">
                    <span class="text-purple-500 font-bold">TOTAL3 Trend:</span> Đại diện cho vốn hóa Altcoin (trừ BTC/ETH). Trend UP xác nhận sự tăng trưởng bền vững của nhóm Alt.
                  </div>
                </div>
              </div>

              <!-- Indicators -->
              <div class="md:col-span-2 p-4 bg-yellow-500/5 rounded-xl border border-yellow-500/10">
                <p class="text-yellow-500 font-bold text-xs uppercase mb-3">Chỉ báo kỹ thuật (BTC Benchmark)</p>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-[10px]">
                  <div>
                    <span class="text-gray-200 font-bold block mb-1">EMA 50/200:</span>
                    Hỗ trợ/kháng cự động. Giá trên EMA là xu hướng tăng, dưới EMA là xu hướng giảm.
                  </div>
                  <div>
                    <span class="text-gray-200 font-bold block mb-1">ADX (Trend Strength):</span>
                    • <strong>> 25:</strong> Xu hướng mạnh. <br/>
                    • <strong>< 20:</strong> Thị trường không xu hướng (Sideway).
                  </div>
                  <div>
                    <span class="text-gray-200 font-bold block mb-1">Pivot Structure:</span>
                    • <strong>HH/HL:</strong> Đỉnh/Đáy cao dần (Tăng). <br/>
                    • <strong>LL/LH:</strong> Đáy/Đỉnh thấp dần (Giảm).
                  </div>
                </div>
              </div>
            </div>
          </section>

          <!-- Section 1: Structural Trend -->
          <section>
            <div class="flex items-center gap-2 mb-6 text-blue-400 border-b border-blue-400/20 pb-2">
              <Target class="w-5 h-5" />
              <h3 class="font-black uppercase tracking-widest text-lg">PHASE 1: STRUCTURAL TREND (1D)</h3>
            </div>
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800">
                <p class="text-green-500 font-bold mb-2">MacroBullish</p>
                <p class="text-xs text-gray-400 leading-relaxed italic">"Bò kiểm soát dài hạn"</p>
                <ul class="text-[11px] mt-2 space-y-1 list-disc list-inside text-gray-300">
                  <li>Giá đóng cửa > EMA200 (1D) ít nhất 3 phiên.</li>
                  <li>Cấu trúc Đỉnh/Đáy sau cao hơn trước (HH/HL).</li>
                  <li><strong>Hành động:</strong> Ưu tiên canh LONG.</li>
                </ul>
              </div>
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800">
                <p class="text-red-500 font-bold mb-2">MacroBearish</p>
                <p class="text-xs text-gray-400 leading-relaxed italic">"Gấu kiểm soát dài hạn"</p>
                <ul class="text-[11px] mt-2 space-y-1 list-disc list-inside text-gray-300">
                  <li>Giá đóng cửa < EMA200 (1D) ít nhất 3 phiên.</li>
                  <li>Cấu trúc Đỉnh/Đáy sau thấp hơn trước (LL/LH).</li>
                  <li><strong>Hành động:</strong> Ưu tiên canh SHORT.</li>
                </ul>
              </div>
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800">
                <p class="text-gray-400 font-bold mb-2">MacroNeutral</p>
                <p class="text-xs text-gray-400 leading-relaxed italic">"Trạng thái lưỡng lự"</p>
                <ul class="text-[11px] mt-2 space-y-1 list-disc list-inside text-gray-300">
                  <li>Giá đi ngang quanh EMA200 hoặc cấu trúc bị gãy.</li>
                  <li>Chưa xác định được bên thắng cuộc.</li>
                  <li><strong>Hành động:</strong> Giảm volume, chờ xác nhận.</li>
                </ul>
              </div>
            </div>
          </section>

          <!-- Section 2: Operational State -->
          <section>
            <div class="flex items-center gap-2 mb-4 text-purple-400">
              <Zap class="w-5 h-5" />
              <h3 class="font-bold uppercase tracking-tight">OPERATIONAL STATE (Khung 4H)</h3>
            </div>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800">
                <p class="text-yellow-500 font-bold mb-1">ActiveBullish / Bearish</p>
                <p class="text-xs text-gray-400 mb-2 font-medium">Xu hướng 4H đồng thuận mạnh</p>
                <p class="text-[11px] text-gray-300 italic leading-relaxed border-l-2 border-yellow-500/50 pl-3">
                  Xảy ra khi: Giá nằm trên/dưới EMA50 & EMA200 + ADX(14) > 25 + Cấu trúc đồng hướng. Đây là trạng thái "Đèn xanh" để đánh theo Trend.
                </p>
              </div>
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800">
                <p class="text-blue-400 font-bold mb-1">Pullback</p>
                <p class="text-xs text-gray-400 mb-2 font-medium">Nhịp điều chỉnh kỹ thuật</p>
                <p class="text-[11px] text-gray-300 italic leading-relaxed border-l-2 border-blue-500/50 pl-3">
                  Xu hướng ngắn hạn đi ngược xu hướng dài hạn nhưng cấu trúc chính chưa hỏng. Cơ hội tìm điểm vào lệnh giá tốt (Buy the dip / Sell the rally).
                </p>
              </div>
              <div class="p-4 bg-black/20 rounded-xl border border-gray-800 col-span-1 md:col-span-2 border-dashed border-red-500/30">
                <p class="text-orange-400 font-bold mb-1">DynamicSideway (Percentile 40)</p>
                <p class="text-xs text-gray-400 mb-2 font-medium">Thị trường nén chặt - "Kẻ giết bot"</p>
                <p class="text-[11px] text-gray-300 italic leading-relaxed">
                  Biên độ 24h nhỏ hơn Percentile 40 của 90 ngày gần nhất + ADX < 20 + EMA50 đi ngang. Hệ thống sẽ **KHÓA QUÉT** để tránh các tín hiệu Break-out giả (Fakeout).
                </p>
              </div>
            </div>
          </section>

          <!-- Section 3: Risk Status -->
          <section>
            <div class="flex items-center gap-2 mb-4 text-green-400">
              <AlertTriangle class="w-5 h-5" />
              <h3 class="font-bold uppercase tracking-tight">RISK STATUS (Chốt chặn bảo vệ)</h3>
            </div>
            <div class="space-y-3">
              <div class="flex gap-4 items-start p-3 bg-red-500/5 rounded-lg border border-red-500/10">
                <span class="text-red-500 font-black text-[10px] bg-red-500/10 px-2 py-1 rounded min-w-[120px] text-center">EventBlock</span>
                <p class="text-[11px] text-gray-400">Sắp có tin vĩ mô cực mạnh (FOMC, CPI). Hệ thống dừng mở lệnh mới (T-60p đến T+30p) để tránh quét râu nến.</p>
              </div>
              <div class="flex gap-4 items-start p-3 bg-orange-500/5 rounded-lg border border-orange-500/10">
                <span class="text-orange-500 font-black text-[10px] bg-orange-500/10 px-2 py-1 rounded min-w-[120px] text-center">VolatilityAlert</span>
                <p class="text-[11px] text-gray-400">ATR tăng đột biến (>3x trung bình). Nguy cơ trượt giá cao. Chuyển sang chiến thuật Scalp hoặc đứng ngoài.</p>
              </div>
              <div class="flex gap-4 items-start p-3 bg-purple-500/5 rounded-lg border border-purple-500/10">
                <span class="text-purple-500 font-black text-[10px] bg-purple-500/10 px-2 py-1 rounded min-w-[120px] text-center">Micro_Reset</span>
                <p class="text-[11px] text-gray-400">Xảy ra hiện tượng thanh lý dây chuyền (Liquidation surge). Chờ thị trường hấp thụ hết lực bán tháo trước khi quét tiếp.</p>
              </div>
            </div>
          </section>
        </div>

        <!-- Footer -->
        <div class="p-6 border-t border-gray-800 bg-black/20 flex justify-center">
          <p class="text-[10px] text-gray-600 uppercase font-black tracking-widest">BinanceTraderTool V2 - The Brain Component Specification</p>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.3s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #333;
  border-radius: 10px;
}
</style>
