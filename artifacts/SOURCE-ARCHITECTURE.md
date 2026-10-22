# Discovery-first usage architecture

## Goal and current boundary

Monitoring is not a login feature. The default flow is **discover known sources → explicitly use existing access → verify the appropriate scope → read/persist observations → recover authentication only when requested**. Credentials and account choices owned by provider apps/CLIs are not replaced just to enable tracking.

The initial migration covered nine quota/balance providers. The continuation adds **Antigravity owned desktop RPC, Devin official-CLI credential/status RPC and Muse subscription paths**, so all twelve catalogue types now have native reader paths. This is **not proof of twelve live entitlements or universal profiles**. Muse's default MSP path only returns real last-observed windows; a fresh host may have none. Its separate key endpoint is credential-issuing and needs explicit default-No native confirmation, never an automatic fallback. Local history remains independent for Codex/Claude/Grok/Kimi/Muse; Antigravity/Devin local history and OpenCode/ZCode SQLite importers remain future work. A catalogue entry, executable, OAuth success, fixture or key is not a real-seat quota guarantee.

## Native ownership and separate domains

- `model::Provider::ALL` and `monitoring.rs` describe all twelve offered providers with separate quota reader, local-history reader and balance-only capabilities. Unsupported data kinds stay explicit.
- Discovery reads **metadata only** at known current-user/native configuration paths. It does not parse credentials, enumerate other users, scan browsers, launch programs, install anything or make authentication requests. The engine owns revisioned cached source inventory; the WebView subscribes and reads the cached value.
- `existing.rs` is the connection broker. It has no login/browser launcher. Codex and Claude use existing safe configured/default sources; no managed profile is prepared. Cursor/GitHub/Grok/Devin/Antigravity/Muse reuse their native source readers. The separate Muse key-endpoint operation requires an additional native warning/approval and is not a read-only existing-source fallback. API keys remain scoped API credentials, not invented people. Explicit disconnect exclusions and implicit legacy defaults are preserved.
- A successful response and durable settings save precede published connected quota. Missing or provider-rejected access is an error; cached expiry alone is not authoritative and never invokes sign-in automatically. Existing-session Claude verification is read-only even if the legacy renewal preference was enabled.
- The existing official-CLI/app login coordinators remain **explicit recovery/add-account tools**, behind Account setup. Cursor Enterprise and GitHub personal billing remain separate advanced connections. Their successful login is not itself evidence of quota permission.
- `historySources` is an engine-owned per-client boolean map, independent of subscription monitoring. Absent Codex/Claude entries preserve old monitoring behavior; an explicit override can enable local-only usage with quota disabled or stop history while quota stays enabled. Targeted writes preserve unrelated preferences; stale generic preference saves cannot erase source choices. Stopping imports retains already indexed totals until normal retention, not secure deletion.
- The native background tracker owns all log indexing, watches/debounce, deduplication, pricing and pushed revisions while hidden. New client logs never publish anonymous quota or claim account identity.

### What data means

1. **Quota observation:** a provider/official-CLI reading, scoped to authenticated account, credential or explicitly selected context as the adapter can actually establish. Percent used, supplied allowance and reset timestamps are not inferred from local tokens.
2. **Local usage:** recorded counters from a particular client on this Windows user's device. A configurable client can use another vendor's model; client history is not proof of a billing provider, logged-in person, every device or all account usage.
3. **Reference value:** separately labelled API-equivalent estimate, when a known/explicit model price exists. Missing pricing stays unavailable/lower-bound. It is not subscription charges, API-balance spend or an invoice.

Claude source IDs retain compatibility, but credential fingerprints corroborate the current source scope. Cached readings require the same provider/ID/identity scope, not merely the same folder. A changed scope clears the prior report even if the read is currently gated; original server backoff/budgets remain intact. This is credential/profile evidence, **not a fabricated verified human email**. The scope attached to a successful read uses the actual token used, including a legacy explicitly enabled renewal. New profiles default renewal off; saved choices survive.

## Original local readers

All extend the bounded, typed native index; no downloaded tracker code is executed or bundled.

- **Muse:** known session tree, `session.jsonl`, versioned `runtime.session` event envelopes. Only `model_completed`/`automated_review_completed` count; microsecond recorded timestamps, stable event IDs, string/review-descriptor models. Input/output are required; cache aliases must agree; cached/reasoning subsets are not added twice. The reader never calls the credential-mint endpoint, handles device tokens or invents a subscription bar.
- **Grok CLI:** `sessions/**/updates.jsonl`, completed turns only, recorded event/session IDs and agent/root timestamps. Per-model breakdowns replace aggregate totals instead of doubling them. Input includes cache subsets; reasoning is already output. Missing identity/timestamp/counters are skipped as incomplete coverage, never placed into today by file modification time. Log model/cost data is not an account quota.
- **Kimi CLI:** selected `KIMI_DATA_DIR` roots or standard `.kimi`/`.kimi-code` session trees. Legacy status token records and new turn-scoped `usage.record` events; cumulative session rollups are excluded. Stable session/agent/record position prevents equal turns collapsing and distinct agents/sessions mixing. Missing recorded timestamps remain unavailable; no model is guessed from a later config or a date-based release assumption.
- Existing Codex/Claude readers retain their streaming/restart/partial-line/echo safety.

Bounds remain 64 MiB/pass, 8 MiB/file, 2 MiB/line, bounded directories/files/depth, no linked traversal and 32-day logical retention. Only counters/model/time/opaque IDs are persisted, not prompts/content/tool output. Partial/unknown schemas are disclosed. This is not a universal transcript/history reconstruction claim.

## Research, not assumptions

Two complementary inspected implementations:

- `steipete/CodexBar` at `6480977b47f6e4ffd5dcaae1cb0c7fdf34ee449e`: subscription adapters, existing-session ownership, Muse local-envelope schema, Antigravity RPC/CLI limits, Devin organization/session requirements. Its browser import/macOS Keychain/OAuth-client extraction paths are not silently ported to Windows.
- `ccusage/ccusage` at `6548615e30b464be8944de6cc71767336596d432`: local usage rather than quota. Public path/parser/loader snapshots for Grok, Kimi, Antigravity, OpenCode and ZCode were downloaded read-only with SHA-256 manifest at `%LOCALAPPDATA%/OpenToken-build/provider-research/discovery/`. Field declarations, selected parsers and relevant fixtures were inspected; snapshot download is not a claim every line was reviewed or live-validated. Upstream packages/installers were not run. The new original Rust readers retain stricter no-guessed-timestamp/model/quota behavior.

Research found concrete remaining paths, not blanket claims of impossibility:

- **Antigravity:** implemented original native Windows same-user SID/session/executable/lifetime/listener/CSRF checks, signed-in identity before/after, summary bucket parsing and conservative legacy fallback. No scripts/process-memory access, arbitrary local probing or borrowed OAuth renewal. Multiple candidate instances/custom installs fail honestly. Both app/IDE profiles are considered; an old folder cannot shadow the running IDE. Separate `antigravity_cli` mode verifies a structurally valid saved bearer from its exact vendor-owned Windows entry around daily-host quota. `antigravity_stored` reads only the exact IDE USS OAuth record, validates deployment/active tier, and verifies Google identity around daily-only quota with a final record/configuration check. See [owner recovery and source details](SESSION-RECOVERY-2026-10-19.md). No token renewal or production-Gemini pool substitution; SQLite history remains unimplemented.
- **Devin:** bounded read-only CLI TOML or the actual Devin desktop authentication record, fixed-host status RPC, deployment/context checks, credential binding, persistence/refresh/disconnect. `devin_app`/`devin_cli` stay distinct. Desktop SQLite/WAL is never copied or checkpointed; custom deployment overrides are not forwarded to cloud. Ambiguous fields remain raw/unknown. Real desktop 3.10.67 access and Cursor 3.24.18 usage now pass native read-only diagnosis; other versions/tenants are not assumed. See [the compatibility correction](COMPATIBILITY-2026-10-19.md).
- **Muse:** implemented bounded official MSP initialization/usage-only transport, no session/prompt/auth requests, exact profile scope, real observation time/expiry and honest empty-host failure. Separately implemented explicit `muse_device` mode for the credential-issuing Meta quota endpoint: mandatory native default-No warning, keys discarded rather than used/persisted/written back, no automatic fallback, authenticated identity and actual supplied quota required. Requests already sent cannot be undone. Saved-mode background calls repeat until disconnected; automatic settings backups omit this approval so recovery cannot resurrect issuance. Inline `dca:` profile support is scoped, not universal keyring import. Sharing arbitrary other hosts' MSP observations remains unproven. Local token tracking is independent.
- **OpenCode/ZCode/Antigravity history:** known SQLite schemas; safe WAL access, migrations/fork/retry/copy deduplication and finite reads need native implementation before claiming totals. OpenCode Go quota is already separate from all OpenCode client history.

The follow-up protocol details, immutable source revisions, semantic conflicts and concrete implementation order are recorded in [REMAINING-PROVIDERS.md](REMAINING-PROVIDERS.md). The implementation, fixture acceptance and remaining live-provider/profile limitations are separately recorded; native test success is not real subscription authorization.

## Safety and compatibility

No global pin/motion/tray/startup changes, no QuotaTray changes, no credential copies/deletions, no auto-installers, no browser protection bypass, no model requests and no borrowed OAuth registration. Existing configured/managed accounts, aliases, secure-store namespaces, API port 6737 and background behavior remain. The app still uses one approved Scheduled Task, not a supervisor pair.

Native fixtures now isolate source discovery home, APPDATA, GitHub/Grok/Kimi/Muse/XDG locations as well as app data. `OPENTOKEN_SOURCE_HOME` only applies to an explicit absolute-data-dir `--no-startup` fixture; normal launches retain the Windows user's real home. Fixtures have path-scoped single-instance identities and must not stop the running app.

See `VERIFICATION.md` for actual tests/build/deployment evidence and remaining live-provider acceptance. No real credential belongs in chat, screenshots, logs or demo mode.
