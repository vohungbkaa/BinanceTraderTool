# GEMINI.md - BinanceTraderTool Project Context

This directory contains the conceptual framework, architectural specifications, implementation plans, and the application source code for the **BinanceTraderTool V2**, an automated trading system designed for Binance Futures USDT-M.

## 1. Project Overview
The project is built as a Desktop Application using **Tauri + Vue 3 + Rust**. It follows an 8-Phase modular architecture aimed at building a robust, automated trading system. The system emphasizes safety ("Risk-First" approach), relative strength analysis, and disciplined execution.

### Architectural Phases:
- **Phase 0:** Data Pipeline & Preprocessing
- **Phase 1:** Market Regime & Context Engine
- **Phase 2:** Relative Strength Scanner
- **Phase 3:** Entry Setup Validation
- **Phase 4:** Risk Management & Scoring
- **Phase 5:** Signal Engine (Telegram/Logging)
- **Phase 6:** Execution & Monitoring
- **Phase 7:** Backtesting & Validation

## 2. Directory Structure
- `/app/`: The main application directory containing the full codebase.
  - `/app/src-tauri/`: Backend core logic (Rust) handling WebSockets (`websocket.rs`), technical indicators (`indicators.rs`), SQLite storage (`db.rs`), and event pipelines (`pipeline.rs`).
  - `/app/src/`: Frontend UI (Vue 3, TypeScript, TailwindCSS, Pinia) for the main dashboard.
- `/system/`: Contains the detailed specifications for each phase (e.g., `phase0_data_pipeline_spec.md`, `phase1_market_regime_spec.md`).
- `/system/system_overview.md`: High-level system architecture and philosophy.
- `/system/0.plan/`: Implementation planning and prompt engineering files for specific phases.
- `/doc/`: Supporting conceptual documentation.

## 3. Tech Stack
- **Frontend**: Vue 3, Vite, TypeScript, TailwindCSS, Pinia, Lucide-Vue.
- **Backend (Tauri Core)**: Rust, Tokio (async runtime), Tungstenite (WebSockets), SQLx (SQLite interactions), Tracing, Reqwest.
- **Database**: SQLite3 (`app/src-tauri/data.db`). Used to store closed candles, technical indicators, and system context risks.

## 4. Building and Running
The primary scripts for development and building are defined in `app/package.json`. Make sure you have Node.js, Rust/Cargo, and SQLite3 installed.

- **Development Mode**: Starts the frontend and Tauri backend with hot-reloading and detailed terminal logs.
  ```bash
  cd app
  npm run tauri dev
  ```
- **Production Build**: Creates the final executable (`.dmg` for macOS, `.exe` for Windows).
  ```bash
  cd app
  npm run tauri build
  ```
- **Database Reset**: If you need to clear the cache and restart data collection from scratch:
  ```bash
  cd app/src-tauri
  rm data.db
  ```

## 5. Usage & Conventions
This repository acts as the **source of truth** for the system's logic and design.
- **Consult Specs First:** When implementing features or fixing bugs, always cross-reference the corresponding `system/phaseX_..._spec.md` file to ensure the solution aligns with the established architecture.
- **Data Flow:** The system relies on a strictly defined data pipeline where outputs from earlier phases act as inputs for subsequent phases (e.g., Phase 0 JSON payload -> Phase 1 -> Phase 2).
- **Database Caching:** The system automatically caches old candles into SQLite to avoid Binance IP bans on restarts. Use standard SQLite tools to inspect `app/src-tauri/data.db` (e.g., querying `closed_candles` or `system_events`).
- **Rate Limiting:** Always ensure there are sleep periods between API requests.
- **Communication:** When discussing or proposing changes, refer to these specifications as the design foundation.
