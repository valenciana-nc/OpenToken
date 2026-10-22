# Personal Cursor connections: implementation research

Reviewed 2026-10-16. This record documents the research preceding implementation. The native desktop-session option is now implemented as described in [CONNECTIONS.md](CONNECTIONS.md); the other alternatives remain research, not a claim of CLI/browser integration or live-provider acceptance.

## Conclusion

Personal Cursor usage monitoring is feasible enough to implement and test: several inspected projects reuse the signed-in Cursor application session to query Cursor's personal dashboard. An Enterprise admin key is not the only technical route. The earlier Enterprise-only recommendation was too restrictive.

However, distinguish **official authentication** from **documented third-party usage access**. Cursor documents browser login through its own CLI. The personal dashboard usage endpoints found in these implementations are undocumented, can change, and are not evidence of a supported third-party OAuth integration or provider approval. A repo's fixtures do not prove the endpoint works with this user's current account/version.

## Inspected GitHub implementations

### 1. CodeZeno/Claude-Code-Usage-Monitor — Windows/Rust reference

- Revision: `d0ab8b74640493cac74786518c21d23d228f553a`.
- [Actual Cursor poller](https://github.com/CodeZeno/Claude-Code-Usage-Monitor/blob/d0ab8b74640493cac74786518c21d23d228f553a/src/poller/cursor.rs).
- Reads `%APPDATA%/Cursor/User/globalStorage/state.vscdb`, `ItemTable` key `cursorAuth/accessToken`.
- Derives a `WorkosCursorSessionToken` cookie from the access token's subject and calls `GET https://cursor.com/api/usage-summary`.
- Parses provider-reported Auto/API/total usage percentages and billing-cycle end. It also accepts an explicit environment session-token override.
- Particularly relevant because the platform and language match OpenToken.
- Do not copy its temporary full-database fallback: Cursor's database can contain unrelated private data, a main-file copy can omit active WAL changes, and failure cleanup is not secure erasure. Also strengthen identity/expiry verification and use honest monthly/bucket labels rather than treating Auto/API as session/weekly windows.
- License reviewed: MIT, Copyright (C) 2026 Code Zeno Pty Ltd. Attribution would be required for copied/adapted code.

### 2. steipete/CodexBar — app-session and browser-login reference

- Revision: `0ff1cf55c0a27b63cbb4a7359e0ef2d098114d97`.
- [Cursor source documentation](https://github.com/steipete/CodexBar/blob/0ff1cf55c0a27b63cbb4a7359e0ef2d098114d97/docs/cursor.md).
- [Read-only app/CLI session access](https://github.com/steipete/CodexBar/blob/0ff1cf55c0a27b63cbb4a7359e0ef2d098114d97/Sources/CodexBarCore/Providers/Cursor/CursorAppAuth.swift).
- [Personal dashboard transport](https://github.com/steipete/CodexBar/blob/0ff1cf55c0a27b63cbb4a7359e0ef2d098114d97/Sources/CodexBarCore/Providers/Cursor/CursorStatusProbe.swift).
- [Interactive browser login](https://github.com/steipete/CodexBar/blob/0ff1cf55c0a27b63cbb4a7359e0ef2d098114d97/Sources/CodexBar/CursorLoginRunner.swift).
- Supports read-only desktop session lookup, JWT expiry checks, SQLite text/BLOB decoding and careful WAL handling. It does not refresh Cursor-owned desktop/CLI tokens. Its Linux reader can also use the session saved by `cursor-agent login`.
- Dashboard sources include `/api/usage-summary` and `/api/auth/me`, plus optional legacy usage, team-member budgets and usage-event sources. Useful identity and missing-field lessons, but OpenToken should still require its own before/after account consistency gates and avoid raw response/debug-body persistence.
- Its macOS Add/Switch flow opens `https://authenticator.cursor.sh/` in a concrete external browser, then imports the selected browser's session cookies, confirms account selection and commits the chosen session. That is **not** a documented third-party OAuth callback and not a drop-in Windows implementation; the inspected browser integration is macOS-specific.
- License reviewed: MIT, Copyright (c) 2026 Peter Steinberger. Research uses a newer revision than OpenToken's separately pinned artwork; no artwork pin was changed.

### 3. getagentseal/codeburn — quota and history separation

- Revision: `9a40cc603fcd194caca9daa8c5134a61ea7e0e2b`.
- [Cursor quota implementation](https://github.com/getagentseal/codeburn/blob/9a40cc603fcd194caca9daa8c5134a61ea7e0e2b/src/quota/cursor.ts).
- [Cursor history/import/sync documentation](https://github.com/getagentseal/codeburn/blob/9a40cc603fcd194caca9daa8c5134a61ea7e0e2b/docs/providers/cursor.md).
- Reads the existing app token, checks expiry with a 60-second margin, derives the dashboard session cookie and fetches `/api/usage-summary`. Handles missing/expired credentials, auth rejection, unavailable metrics and rate limiting.
- Separately supports imported/synced dashboard usage CSVs, including source/account/coverage deduplication. This is useful for later history work, not proof that local chat records reveal subscription quota.
- Important caveat from its docs: some local Cursor token figures are estimated from text/context, whereas dashboard exports carry actual reported token counters. OpenToken must never present character-based estimates as recorded tokens or use them to invent percent-left. Provider-metered usage amounts, API-equivalent estimates and final invoices also remain different concepts.
- License reviewed: MIT, Copyright (c) 2026 AgentSeal.

### 4. wakamex/cursor-cli-usage — alternative bearer transport

- Revision: `ecf7f00a390ebc2aae8ba4648c0199e5904b5dd8`.
- [Implementation](https://github.com/wakamex/cursor-cli-usage/blob/ecf7f00a390ebc2aae8ba4648c0199e5904b5dd8/src/cursor_cli_usage/__init__.py).
- [Fixture tests](https://github.com/wakamex/cursor-cli-usage/blob/ecf7f00a390ebc2aae8ba4648c0199e5904b5dd8/tests/test_cursor_cli_usage.py).
- Attempts `agent status --format json`, extracting `auth.accessToken`, then falls back to the read-only IDE database.
- Sends a Bearer-authenticated POST to `https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage`, with Connect protocol headers. Parses `planUsage`, `spendLimitUsage` and billing dates.
- This is a concrete second transport to investigate, not merely a README claim. However, the shown CLI status token shape is fixture-tested in that repo, not established by the official authentication documentation or live-tested here. It needs version-specific validation before becoming an OpenToken dependency.
- No repository LICENSE file or declared license was found in the inspected tree/project metadata. Do not copy this implementation; treat it as a protocol lead only.

### Excluded lead: coding_agent_usage_tracker

The README/repository description mentions Cursor, but its inspected `src/providers/cursor/mod.rs` explicitly returns an empty fetch plan and says Cursor is not yet ported. It is not evidence of a functioning connector. Its license also contains a nonstandard additional rider. No code from it was adopted.

## Official Cursor authentication documentation

- [CLI authentication](https://cursor.com/docs/cli/reference/authentication): documents `agent login` opening the default browser and `agent status` reporting authentication/account information. OpenToken can delegate user authentication to Cursor rather than collecting a password or inventing its own OAuth client.
- [CLI installation](https://cursor.com/docs/cli/installation): currently documents native Windows as well as WSL. This research did not install or execute either CLI/remote installer.
- These documents **do not establish** that `agent status --format json` exports an access token, nor do they promise a third-party personal quota API. Confirm those separately against supported versions.

## Options for OpenToken

| Option | Ordinary personal account? | Trade-off | Recommendation |
|---|---|---|---|
| Explicitly connect the installed Cursor app's existing sign-in | Implemented by multiple inspected projects | Read-only access to sensitive local auth; personal dashboard API is undocumented | Best first Windows option, opt-in |
| Launch Cursor's official CLI browser login and reuse its resulting local sign-in | Official login exists; exact Windows token access still needs validation | CLI dependency/version-specific credential format; usage API still undocumented | Good login-assisted alternative after validating the installed CLI |
| External browser login plus selected-session import | CodexBar implements this on macOS | Windows cookie access/protection, profile ambiguity and broad credential access; not ordinary public OAuth | Not the first implementation; do not inspect unrelated browser profiles or bypass browser protection |
| Enterprise admin API | Already implemented | Enterprise entitlement and privileged admin key, not personal allowance | Keep as a clearly separate advanced option |
| Dashboard CSV import/sync | Useful history source | Reported usage/cost history is not a live remaining-quota API | Later, separately scoped history feature |

## Recommended implementation scope

1. Cursor's connection screen offers **Personal account — use Cursor sign-in** as the ordinary-user path, with the existing **Enterprise admin API** under a separate advanced choice.
2. User explicitly consents to a bounded, read-only lookup of the selected installed Cursor profile. No automatic credential harvesting, browser-database scanning, copying the full state database or exporting keys to the WebView.
3. If signed out, guide the user to sign in through Cursor itself, or launch an already-installed, verified Cursor CLI's official browser-login command. Never install/run remote scripts automatically or claim an unverified third-party OAuth callback.
4. Verify the current authenticated provider identity, fetch bounded usage through the selected fixed-host adapter, then check the local session and provider identity again. A changed account invalidates old usage; transient errors may retain only same-identity stale data. JWT decoding alone is not signature verification or proof of the account.
5. Cursor remains owner of its own refresh tokens. Do not modify its database, refresh its tokens, save its token in OpenToken preferences/logs, or delete its login when disconnecting monitoring.
6. Publish only finite, actually reported plan/bucket values and reset dates. Keep missing/unlimited/team/individual meanings distinct. Do not average Auto/API percentages, guess monthly limits, map monthly data to a weekly quota, or present local text estimates as recorded tokens.
7. Clearly label personal usage as an **unofficial dashboard connector**. Reuse existing backoff/read budgets and identity tests; add synthetic SQLite/WAL/encoding/expiry, account-switch, cancellation, malformed response and restart cases before opt-in live acceptance.

## Research boundary

Public source and documentation were inspected, not executed. No real Cursor/CLI/browser credentials were read, no new provider login was performed, and no live quota endpoint was tested. The running OpenToken executable, credentials, settings and startup entries were not changed. At the research stage this report was the only project addition. The later native implementation is documented separately in CONNECTIONS.md and VERIFICATION.md.

Downloaded public snapshots and SHA-256 manifests are under `%LOCALAPPDATA%/OpenToken-build/cursor-research/`; they are research material, not bundled dependencies or production code. Upstream licensing and provider terms must still be respected if implementation code is adapted.
