# GEMINI.md - BinanceTraderTool Project Context

This directory contains the conceptual framework, architectural specifications, and implementation plans for the **BinanceTraderTool V2**, an automated trading system designed for Binance Futures.

## 1. Project Overview
The project follows an 8-Phase modular architecture aimed at building a robust, automated trading system. The system emphasizes safety ("Risk-First" approach), relative strength analysis, and disciplined execution.

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
- `/system/`: Contains the detailed specifications for each phase (e.g., `phase0_data_pipeline_spec.md`, `phase1_market_regime_spec.md`).
- `/system/system_overview.md`: High-level system architecture and philosophy.
- `/system/0.plan/`: Implementation planning and prompt engineering files for specific phases.
- `/doc/`: Supporting conceptual documentation.

## 3. Usage & Conventions
This repository acts as the **source of truth** for the system's logic and design.
- **Consult Specs First:** When implementing features or fixing bugs, always cross-reference the corresponding `system/phaseX_..._spec.md` file to ensure the solution aligns with the established architecture.
- **Data Flow:** The system relies on a strictly defined data pipeline where outputs from earlier phases act as inputs for subsequent phases (e.g., Phase 0 JSON payload -> Phase 1 -> Phase 2).
- **Communication:** When discussing or proposing changes, refer to these specifications as the design foundation.
