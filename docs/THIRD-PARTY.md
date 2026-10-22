# Third-party notices

OpenToken is independent software, not affiliated with or endorsed by OpenAI, Anthropic, Cognition or other provider owners. Provider names and marks remain their respective owners' trademarks and identify the supported/planned providers in the navigation.

## Provider marks

### Reference UI grid (CodexBar, MIT)

The twelve vector marks in `src/assets/providers/` are Codex, Claude, Antigravity, Grok, Muse Code, Cursor, Copilot, Devin, Z.ai, DeepSeek, Kimi and OpenCode. They are bundled from [CodexBar](https://github.com/steipete/CodexBar), MIT, copyright (c) 2026 Peter Steinberger, pinned revision `74f64b62b5d38b9cea59107d15799d69563eb38a`, under `Sources/CodexBar/Resources/ProviderIcon-*.svg`.

Amp, ElevenLabs, Zed, Perplexity, Pi and TypeSafe remain removed from the UI and bundled asset set.

`scripts/fetch-provider-marks.mjs` is a maintainer-only, revision-pinned importer. There are no runtime image downloads, remote fonts or network dependencies for the UI. Full copyright/permission/disclaimer are in [license notices](THIRD-PARTY-LICENSES.txt). These navigation marks do not imply uniform live connector support. Codex/Claude quotas, opt-in personal Cursor desktop-session dashboard quotas, Cursor Enterprise member usage and personal Copilot billing have separate supported scopes; Z.ai/Kimi Code/OpenCode Go now have key-scoped quota adapters, DeepSeek an API-balance adapter, and Copilot a separate official-CLI-owned login for unofficial quota. Grok also has official-CLI-owned sign-in and an original private-credit adapter. Muse has an original local-history reader independent of authentication, an official-protocol last-observation reader, and a separate explicitly approved credential-issuing quota mode. Antigravity and Devin now have original owned-local-RPC/official-CLI source readers, not bundled external trackers. See [connection capabilities](CONNECTIONS.md).

### Native tray marks (Simple Icons, CC0; CodexBar, MIT)

The bundled `src-tauri/icons/openai.svg` and `claude.svg` are from **Simple Icons 13.21.0**, distributed under [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/).

- Project: https://github.com/simple-icons/simple-icons
- Pinned OpenAI source: https://cdn.jsdelivr.net/npm/simple-icons@13.21.0/icons/openai.svg
- Pinned Claude source: https://cdn.jsdelivr.net/npm/simple-icons@13.21.0/icons/claude.svg
- Upstream license: https://github.com/simple-icons/simple-icons/blob/13.21.0/LICENSE.md

The native `cursor-tray.png`, `copilot-tray.png`, `zai-tray.png`, `deepseek-tray.png`, `kimi-tray.png`, `opencode-tray.png`, `grok-tray.png`, `muse-tray.png`, `antigravity-tray.png` and `devin-tray.png` are rasterized from the CodexBar MIT vectors listed above, not Simple Icons. Their attribution and license remain CodexBar's.

The SVGs are bundled, not fetched by the app. `scripts/generate-provider-icons.mjs` rasterizes those same local marks to the native tray PNGs; it requires installed Edge and Playwright only at generation time. UI masks use the reference vectors above; native tray icon colors follow the taskbar theme.

Personal Cursor's native adapter is an original implementation; public protocol/source research is recorded in [CURSOR-RESEARCH.md](CURSOR-RESEARCH.md). No external tracker code or cookie-extraction library is bundled for this feature. Expanded protocol research, original native adapters and rejected borrowed OAuth client IDs are documented in [PROVIDER-RESEARCH.md](PROVIDER-RESEARCH.md); downloaded source and plugins were not executed or bundled.

## Antigravity, Devin and Muse protocol research

The original Rust `local_providers` readers, Windows ownership checks and Muse transports are described in [REMAINING-PROVIDERS.md](REMAINING-PROVIDERS.md), with immutable revisions and individual provenance/hash manifests outside the repo. Selected public OpenUsage/Windows-port/Pane/usage/Muse SDK/plugin sources were read for protocol/schema/Windows path and behavior research. Their tracker/plugin/SDK runtime code was not executed, copied or bundled. In particular, no AGPL tracker code, borrowed OAuth registration/client secret, browser extraction library or external process/netstat script is shipped. Existing MIT artwork attribution is separate from this protocol research. New reader code is original OpenToken Rust/TypeScript; native Windows APIs come from the existing windows-rs dependency.

## Usage tracking and ordering

The original source registry, existing-session broker and Grok/Kimi/Muse native record parsers are described in [SOURCE-ARCHITECTURE.md](SOURCE-ARCHITECTURE.md). Pinned public ccusage/CodexBar source was inspected for protocol/schema research, not executed or bundled. Supporting a local client does not imply subscription quota or account identity.

- The bundled reference-price snapshot is adapted from LiteLLM's non-enterprise `model_prices_and_context_window.json`, MIT, copyright (c) 2023 Berri AI. Snapshot provenance and revision are in `crates/opentoken-core/src/spending/prices.json`; `scripts/update-prices.mjs` refreshes it only as a maintainer action. Prices are estimates, not provider invoices. No runtime download occurs. See [license notices](THIRD-PARTY-LICENSES.txt).
- Drag/keyboard sorting uses dnd-kit (MIT, copyright (c) 2021 Claudéric Demers). SQLite indexing uses rusqlite (MIT) and bundled SQLite (public domain); upstream dependency licenses remain applicable. See [license notices](THIRD-PARTY-LICENSES.txt).

QuotaTray/OpenUsage attribution is retained separately in the root LICENSE and README. Rust/JavaScript dependencies retain their upstream licenses.
