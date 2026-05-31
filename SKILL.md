# REQUIRED SKILLS FOR BUILDING TAURI + VUE + RUST TRADING SYSTEM

Tài liệu này tổng hợp toàn bộ các kỹ năng cần thiết để xây dựng và vận hành hệ thống Desktop Trading/Scanner hiệu năng cao, bảo mật và ổn định.

---

## 1. FRONTEND SKILLS (VUE 3 + TYPESCRIPT)
Chịu trách nhiệm hiển thị Dashboard, Watchlist, Tín hiệu, Biểu đồ và cấu hình hệ thống.

- **Framework:** Vue 3 (Composition API), TypeScript.
- **State Management:** Pinia (quản lý trạng thái realtime toàn ứng dụng).
- **Routing:** Vue Router.
- **UI/UX:** Responsive Desktop Layout, UI Component Design (Tailwind CSS hoặc UI Library).
- **Realtime UI:** Tối ưu hóa rendering khi dữ liệu nến và ticker cập nhật liên tục (High-frequency updates).
- **Chart Integration:** Tích hợp Lightweight Charts (TradingView) hoặc các thư viện biểu đồ tài chính.
- **Tauri Integration:** Kết nối với Rust Backend thông qua Tauri Command / Event system.

---

## 2. RUST BACKEND SKILLS
Lớp xử lý Core Engine, chịu trách nhiệm cho các module tính toán hiệu năng cao và ổn định.

- **Rust Fundamentals:** Ownership, Borrowing, Structs, Enums, Traits.
- **Async Programming:** Sử dụng **Tokio** cho concurrency và task scheduling.
- **Market Data Handling:** WebSocket client, xử lý luồng dữ liệu realtime từ sàn giao dịch.
- **Performance:** Serialization/Deserialization cực nhanh với **Serde**.
- **System:** File system, local persistence, logging (Tracing), retry, timeout, reconnect logic.
- **State Management:** Quản lý Thread-safe state (Mutex, Arc, Channels) giữa các luồng xử lý.

---

## 3. TAURI-SPECIFIC SKILLS
Application shell đóng gói hệ thống thành ứng dụng Native Desktop (Windows, macOS).

- **Core Tauri:** Project structure, Tauri commands (Invoke), Event emit/listen.
- **OS Integration:** Window management, System tray, Native notifications.
- **Security:** Cấu hình Permission (Allowlist), Secure IPC giữa Frontend và Backend.
- **Release:** App packaging, Code signing, Auto-update strategy.
- **Secret Management:** Quản lý API Key và thông tin nhạy cảm một cách an toàn ở cấp độ Native.

---

## 4. TRADING SYSTEM ENGINEERING SKILLS
Nhóm kỹ năng cốt lõi để hiện thực hóa các chiến thuật giao dịch.

- **Data Processing:** OHLCV aggregation, dữ liệu nến đa khung thời gian.
- **Technical Logic:** Tính toán Relative Strength (RS), Trend/Volatility detection.
- **Strategy Engine:** Setup validation logic, Liquidity sweep detection, Signal scoring.
- **Risk Engine:** Position sizing, Stop-loss/Take-profit logic, Portfolio risk management.
- **Safety:** Thiết kế Kill-switch, Fail-safe handling khi gặp biến động cực lớn hoặc lỗi sàn.

---

## 5. DATABASE & PERSISTENCE SKILLS
Lưu trữ lịch sử tín hiệu, cấu hình và log hệ thống.

- **Local Database:** **SQLite** (ưu tiên cho Desktop local-first).
- **Database Design:** Schema design, Query optimization cho dữ liệu lịch sử lớn.
- **Logic:** Transaction handling, Migration strategy, Local caching.
- **Advanced:** Nếu cần đồng bộ Cloud, cần thêm kiến thức về PostgreSQL / API integration.

---

## 6. DEVOPS & SECURITY SKILLS
Đảm bảo hệ thống được vận hành ổn định và bảo vệ tài sản của người dùng.

### DevOps:
- **Build Pipeline:** CI/CD cho Desktop release (GitHub Actions).
- **Release:** Versioning, Crash logging, Error monitoring (Sentry).
- **Deployment:** Code signing cho Windows và macOS.

### Security (Bắt buộc):
- **API Key Safety:** Tuyệt đối không để lộ Secret ở Frontend hoặc ghi vào log.
- **Sanitization:** Input validation và Log sanitization.
- **IPC Control:** Kiểm soát quyền hạn giữa UI và Native layer.

---

## 7. TEAM SKILL MATRIX (MA TRẬN VAI TRÒ)

| Vai trò | Kỹ năng trọng tâm |
| :--- | :--- |
| **Product / Trading Analyst** | Trading logic, Market regime, Setup rules, Risk rules. |
| **Frontend Developer** | Vue 3, TypeScript, Pinia, Chart UI, Realtime Dashboard. |
| **Rust Developer** | Async Rust, WebSocket, Core Engine, Tauri commands. |
| **Desktop App Engineer** | Tauri packaging, OS integration, Auto-updater. |
| **QA / System Tester** | Replay test, Signal validation, Failure scenario test. |

---

## 8. LỘ TRÌNH PHÁT TRIỂN KHUYẾN NGHỊ (DEVELOPMENT APPROACH)

1. **Giai đoạn 1 (UI Shell):** Xây dựng giao diện với Vue 3 và tạo shell Desktop bằng Tauri.
2. **Giai đoạn 2 (Data Layer):** Viết Module kết nối WebSocket Binance bằng Rust để lấy dữ liệu realtime.
3. **Giai đoạn 3 (Logic Engine):** Hiện thực hóa Scanner / Setup / Risk engine trong Rust.
4. **Giai đoạn 4 (Persistence):** Lưu trữ dữ liệu và cấu hình bằng SQLite.
5. **Giai đoạn 5 (Safety & Alert):** Thêm Logging, Native Notification và cơ chế Fail-safe.
6. **Giai đoạn 6 (Validation):** Chạy Paper mode, Replay test và kiểm tra các kịch bản lỗi.
7. **Giai đoạn 7 (Release):** Đóng gói, ký ứng dụng và phát hành phiên bản MVP.
