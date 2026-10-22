# OpenToken delivery decisions

Project workspace: `OpenToken`. The original QuotaTray installation remains unchanged.

Accepted scope: new Rust/TypeScript implementation, Tauri 2 + React, single Scheduled Task with failure restarts, Claude write-back with explicit locking/conflict limitations, API default 6737, light theme plus optional dark/system, tray-adjacent placement, local reset countdowns. No .NET code is reused or invoked.

Delivery stages:

- [x] Toolchain: user-local Rust/MSVC + existing Node/WebView2; no startup registration during development.
- [x] Typed domain, provider JSON mappers, settings validation/backup/migration.
- [x] Isolated Codex stdio transport, bounded messages, environment scrubbing, cancellation, consistent-identity HTTP fallback.
- [x] Discovery/deduplication/store modes, event-triggered reconciliation, deadline/backoff scheduler, change-only cache.
- [x] Claude usage/renewal, advisory locking and concurrent-file conflict checks, multi-home support.
- [x] Native percentage icons with render caching, single-instance IPC, hide/reopen/exit lifecycle, COM scheduled startup, uninstall cleanup.
- [x] React UI: curated provider panels, accounts, one-action Codex/Claude/Cursor sign-in, automatic isolated CLI account storage, monitoring-only disconnect, aliases, settings, themes, reset countdowns, explicit sample-only preview. Legacy folder-entry/management UI removed; existing configured homes retained.
- [x] Loopback read-only API, origin/fetch/Host rejection, ETag, live port rebind/error reporting.
- [x] Rust/TS/browser tests, lint, Windows CI, release scripts, installer and portable executable generation.
- [x] Final native smoke and artifact checks: see VERIFICATION.md for actual recorded results and explicit untested OS/live-provider scenarios.

Not claimed: protocol stability, proof of live provider availability, universal cross-app token locking, hang detection, code signing, automatic updates or a completed multi-day soak. API schemas intentionally use typed OpenToken state rather than promising byte-compatible QuotaTray output. Manual-to-scheduled recovery handoff occurs at next sign-in; QuotaTray cutover is user-controlled.
