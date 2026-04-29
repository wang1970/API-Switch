# API Switch v0.3.0 → v0.4.1 Development Summary

> Development period: 2026-04-26 ~ 2026-04-28

---

## v0.4.1 (2026-04-28) — Bug Fixes

| # | Change | Description |
|---|--------|-------------|
| 1 | **Channel save button unlocked** | New channels no longer require model fetch before saving; only name/URL/API Key needed |
| 2 | **gzip decompression support** | Enabled reqwest `gzip` feature, fixing `error decoding response body` for gzip-compressed upstream responses |
| 3 | **Add model dialog cleanup** | Removed model metadata info block from AddApiDialog |

---

## v0.4.0 (2026-04-28) — Latency Testing / Bilingual Guide / Smart Model Selection

| # | Change | Description |
|---|--------|-------------|
| 1 | **API pool one-click latency test** | New test button to measure latency for all models; green on success, red ✗ on failure |
| 2 | **API pool response time field** | Added `response_ms` column to `api_entries` table with auto-migration |
| 3 | **Channel latency test improvement** | Tests all channels (not just enabled); uses local state to avoid list jumping |
| 4 | **Bilingual usage guide** | Sidebar auto-switches GUIDE_CN.md / GUIDE.md based on `i18n.language` |
| 5 | **Auto-open add dialog** | Auto-opens channel add dialog when entering channel page with no channels |
| 6 | **Smart model pre-selection** | Auto-selects models released within 6 months + existing channel models after fetch |
| 7 | **New models enabled by default** | `sync_entries_for_channel` now sets `enabled = 1` for new entries |
| 8 | **Selection sync fix** | Channel save always calls `selectModels`, even with empty selection |
| 9 | **API pool cache refresh** | Channel save invalidates both `entries` and `channels` queries |
| 10 | **Channel list nowrap** | Status, response time, model count columns prevent line wrapping |

### Additional Changes (accumulated during v0.3.x)

| # | Change | Description |
|---|--------|-------------|
| 1 | **Settings page slider** | Recovery timeout changed from number input to slider (300-1800s, default 600s) |
| 2 | **Channel batch latency test** | Refresh button in "Response" column header to test all channels |
| 3 | **Channel response time** | Edit-save auto-probes URL latency and stores in `response_ms` |
| 4 | **Tray menu restore** | Restored "Open Main Window" tray menu item |

---

## v0.3.0 (2026-04-27) — Smart Model Fetch / Model Catalog / Auto-Disable Fix

| # | Change | Description |
|---|--------|-------------|
| 1 | **Single-button smart model fetch** | Removed two-step detection UI; backend validates API type + base URL then fallback-fetches models |
| 2 | **URL / type auto-correction** | Falls back from wrong endpoint to correct base site; auto-fills `api_type` and `base_url` |
| 3 | **Avoid Gemini misclassification** | Only classifies as Gemini when hitting `/v1beta/openai/*` path |
| 4 | **Filter `auto` model** | Upstream `auto` entries excluded from channel model list |
| 5 | **Local model catalog** | Added `models.json` + `modelsCatalog.ts`; cards and dialogs show release date, capabilities, context/output limits |
| 6 | **Fuzzy model matching** | Supports `provider/model`, `-free`, `-preview` suffix cleaning and similarity matching |
| 7 | **Auto-disable default codes** | Status codes changed to `401,403,410`; settings page exposes input for customization |
| 8 | **Proxy auto-disable** | Matching status codes set `enabled=false` directly |
| 9 | **Log path fix** | `attempt_path` correctly formatted from object arrays |
| 10 | **Test dialog close fix** | Request sequence isolation + forced cleanup on close |
| 11 | **API pool text compression** | Title changed to `Channel / Model`; cooldown shown inline |

---

## Known Issues (v0.4.0 Deprecation Reasons)

| Issue | Severity | Description |
|-------|----------|-------------|
| **Channel fetch auto-disables entries** | High | When network is unreachable, Step 1 validation failure auto-disables channel and all its entries with no recovery mechanism |
| **Endpoint detection request explosion** | Medium | 3 base_urls × 6 types × 6 URLs = up to 108 serial requests; worst case ~6 min wait |
| **gzip response decoding failure** | High (fixed) | reqwest lacked `gzip` feature; some upstreams return compressed responses causing parse failure |

---

## Technical Changes Summary

### Backend (Rust)
- Enabled reqwest `gzip` feature
- `fetch_models` split into Step 1 endpoint detection + Step 2 model fetch
- `detect_endpoint_guess` refactored to phased detection (Phase 1 user type → Phase 2 cross-type correction)
- `canonical_base_url_for_success` case-insensitive matching
- `join_url` case-insensitive `/v1`, `/v1beta` suffix stripping

### Frontend (React/TypeScript)
- `fetchModels` return type changed from `ModelInfo[]` to `FetchModelsResult`
- Channel editor dialog: removed API type lock, supports re-validation on edit
- `AddApiDialog` removed model metadata info block
- Local model catalog `modelsCatalog.ts` integration

### Database
- `api_entries` added `response_ms TEXT DEFAULT ''`
- `circuit_recovery_secs` default changed from 300 to 600

---

*Generated: 2026-04-28*
