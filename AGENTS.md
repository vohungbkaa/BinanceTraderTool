# Repository Guidelines

## Project Structure & Module Organization
This repository contains a Tauri desktop trading tool. The runnable workspace is `app/`.

- `app/src/`: Vue 3 + TypeScript frontend. Components are grouped under `components/dashboard/`, `components/common/`, and `components/layout/`. Pinia stores live in `stores/`, formatters in `composables/`, and shared TS types in `types/`.
- `app/src-tauri/src/`: Rust backend. Core trading, data, and integration logic is under `core/`; scanner/regime simulation code is under `engine/`.
- `app/src-tauri/data.db`: local SQLite cache used by the app.
- `system/`: phase-by-phase architecture specs. Check the relevant `phaseX_...md` file before changing trading logic.
- `doc/`: supporting notes.

## Build, Test, and Development Commands
Run commands from `app/` unless noted.

- `npm install`: install frontend and Tauri dependencies.
- `npm run dev`: start the Vite frontend only.
- `npm run tauri dev`: run the full desktop app with Rust backend and frontend.
- `npm run build`: type-check TypeScript and build the frontend.
- `npm run tauri build`: create a production desktop build.
- `cd app/src-tauri && cargo test`: run Rust unit and integration tests.
- `cd app/src-tauri && cargo fmt`: format Rust code before committing.

## Coding Style & Naming Conventions
Use Vue single-file components with `<script setup lang="ts">`. Name components in PascalCase, for example `AltcoinScanner.vue`, and keep shared domain types in `app/src/types/`. Prefer small composables for reused formatting or state helpers.

Rust uses the 2021 edition and `rustfmt`. Use snake_case for modules, functions, and tests; PascalCase for structs/enums. Keep backend modules aligned with `core/` and `engine/`.

## Testing Guidelines
Rust tests use Cargo with `#[tokio::test]` for async code. Existing tests are colocated in `core/smoke_test.rs`, `core/integration_test.rs`, and `core/pipeline/pipeline_test.rs`. Name tests descriptively with `test_...`.

Some tests call Binance APIs or touch persistence, so document network assumptions in PRs and prefer in-memory SQLite (`sqlite::memory:`) for new database tests.

## Commit & Pull Request Guidelines
Recent history uses short subjects like `get top alt coin`; future commits should be clearer and imperative, for example `Add scanner risk filters` or `Fix metadata cache update`.

Pull requests should include a summary, tests run, linked issue or spec phase when applicable, and screenshots for UI changes. Call out changes to `data.db`, API behavior, rate limits, or trading-risk logic.

## Security & Configuration Tips
Do not commit API keys, secrets, or account-specific trading configuration. Treat `app/src-tauri/config.json` and `data.db` as local runtime state unless a change is intentional. Deleting `data.db` resets the local candle cache.
