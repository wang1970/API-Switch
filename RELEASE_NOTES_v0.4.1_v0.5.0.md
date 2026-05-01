# API Switch v0.4.1 → v0.5.0 Development Summary

> Development period: 2026-04-28 ~ 2026-05-01  
> Release target: v0.5.0

---

## Highlights

### 1. Proxy routing hardening

- Reworked explicit-model and `auto` routing rules so disabled entries remain visible and callable by explicit model name, while only enabled entries participate in `auto` routing.
- Added failover behavior for explicit model requests: if matching entries are cooled down or fail, routing can fall back to the `auto` pool.
- Centralized route sorting so `/v1/models`, proxy routing, tray menu, and UI sorting use the same custom/latest/fastest model priority model.

### 2. API pool sorting and metadata

- Added the API pool three-way sorting bar: **Specified Models / Latest Models / Fastest Models**.
- Persisted the selected default sort mode through settings and backend storage.
- Added catalog-backed model metadata normalization and backfill for provider logo, release date, and localized model summaries.
- Improved model matching quality by preferring authoritative provider catalog metadata over third-party aggregator metadata.

### 3. Latency testing and failure handling

- Improved one-click latency testing by running channels in parallel while keeping entries within the same channel controlled.
- Added progress display during full-pool latency tests.
- Failed latency tests now directly disable unavailable entries where appropriate.
- Stored response times as millisecond values and reused them for fastest-model sorting.

### 4. Circuit breaker and timeout controls

- Added configurable upstream proxy connect timeout in Settings.
- Stored the new `proxy_connect_timeout_secs` config key with database defaults and full frontend/backend type synchronization.
- Adjusted circuit breaker defaults and labels to better match the continuous-failure disable behavior.

### 5. Dashboard and usage statistics

- Fixed “today” statistics to use local-day boundaries instead of UTC-style rolling 24-hour windows.
- Improved dashboard token charts with compact axis labels and full formatted tooltip values.

### 6. Logs and debugging

- Enhanced upstream error logging so raw upstream error bodies, including detailed 429 responses, are preserved.
- Changed unauthenticated/internal token display in logs from `auto` to `NONE` for clearer diagnostics.
- Avoided treating client-side stream disconnects as upstream failures.

### 7. Channel and model management

- Allowed channels to be saved without requiring a successful model fetch first.
- When manually adding a model to the API pool, the channel model list is updated if that model was missing.
- Improved channel table layout, model count display, and release-date normalization.

### 8. UI and product cleanup

- Refined the API pool header, sticky search behavior, and three-button sorting layout alignment.
- Removed the experimental CLI page from the main sidebar.
- Updated bilingual labels for sort modes, circuit settings, and related UI text.
- Added personal planning-note ignore rules and project agent instructions.

### 9. Version and planning updates

- Bumped application version from `0.4.1` to `0.5.0` across npm, Tauri, Cargo, and lock metadata.
- Updated `PLAN.md` to reflect version `0.5.0`.

---

## Small polish

Minor layout alignment, wording, metadata fallback, comments, and formatting fixes were applied across the UI and backend.

---

## Technical notes

### Backend

- Added settings storage and runtime use for `proxy_connect_timeout_secs`.
- Added model metadata persistence/backfill support for API pool entries.
- Added helper logic to keep manually created API pool entries synchronized with channel model lists.
- Added lazy tray refresh infrastructure while keeping eager refresh as the current default.

### Frontend

- Synced API pool sort mode with backend settings using full `AppSettings` updates.
- Matched frontend latest/fastest sorting with backend route sorting semantics.
- Added catalog metadata precomputation for display and normalization.
- Improved dashboard number formatting and local-day filtering.

### Database

- Added default config key: `proxy_connect_timeout_secs = 30`.
- Continued compatibility-first behavior through default config insertion and frontend/backend type updates.
