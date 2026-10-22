# Antigravity: second-pass protocol investigation — 2026-10-19

This supersedes the IDE recovery recommendation in [the previous session investigation](SESSION-RECOVERY-2026-10-19.md). An expired cached token is not a complete implementation diagnosis. This investigation found concrete OpenToken bugs and better official interfaces. The diagnostic alone did **not** establish live Antigravity monitoring. The subsequent implementation below closes the missing connection/display path.

**Live integration outcome (2026-10-20):** the new mode was deployed, the native approval completed without automation choosing Yes, and the actual production window now visibly shows the CLI-profile label plus Gemini/third-party weekly meters (both reporting 100% left). This was checked with read-only Windows UI Automation, separately from synthetic screenshots and API diagnosis. Only connection metadata changed; the other 22 saved properties stayed identical. Grok's preexisting relogin fault remains unrelated. See VERIFICATION.md for precise acceptance and limits.

## Other-repository comparison and the missing visible monitor

| Repository / pinned revision | Actual handling | OpenToken decision |
|---|---|---|
| CodexBar `8fa003e092fe5e713fda8f7ace5c6f0b73305e64` | `AntigravityProviderDescriptor.swift` detects the CLI localhost CSRF gate from agy 1.2.2, skips an unusable warm-server spawn, then uses version-gated `-p /usage --output-format json`. `fetchWithReportFallback` allows identity-free results only under CLI source authority, not as a substitute for a selected OAuth account. The result is actually returned to its provider/UI. | Adopt the **separate CLI-profile authority**, official zero-inference quota command and real persistence/rendering. Do not require an email just to display an honestly scoped CLI observation. Do not adopt its credential-staging/writeback path for named accounts. |
| Pane `5d2f43aa453d850a4e32e9a213601e60158f14c8` | Tries language-server RPC, then Windows vault/Cloud Code with a CLI-shaped User-Agent, borrowed Google refresh registration and daily/production gap filling. | Keep original native owned-RPC verification, but reject borrowed refresh registrations, fake versions/caller identity and cross-pool gap filling. |
| OpenUsage `26132ff48dd78eb2c4a9498e40424e6365f99fec` | Tries app/CLI local services, then cached/vault bearer and Google renewal; renders provider-level quota independently of a verified human email. | Use explicit source-scoped observations, not a fabricated human identity. Do not import its token cache/renewal or host merging. |
| OpenUsage Windows `971f4ba68870218a302191d2760abb552c274709` | Reads exact stored USS OAuth state and the shared agy entry; its fork deliberately leaves borrowed Google client credentials empty, so renewal is not a universal working fallback. | Source-format evidence only. An expired snapshot plus an incomplete refresh recipe is not a reason to block the separately working official CLI report. |

The key OpenToken bug was not merely failing to obtain quota: **the successful CLI diagnostic was never connected to settings, the engine, cards or tray.** Blocking every unnamed CLI report pending human-email verification went beyond what a source-scoped monitor requires.

### Implemented CLI-profile connection

Antigravity now offers **Monitor with agy CLI** directly in its connection panel. A separate owned native **default-No** confirmation explains repeated background calls, possible official-client login renewal, missing human identity and current-profile account switching. Existing read-only reuse is unchanged and does not silently fall through to this mode.

Approval creates `antigravity_cli_usage` connection/report provenance, verifies the installed executable/version and exact zero-model-work report, saves the connection transactionally, publishes real engine quota, and renders cards/grid/accounts/tray. Native scheduling, retry budgets/backoff, cancellation, duplicate rejection and monitoring-only disconnect apply. The CLI owns credentials; no OAuth registration, copied credential, prompt or key issuance is used. Short existing verification spacing is awaited rather than bypassed; longer provider pauses remain errors/backoff.

The source is explicitly labelled **Antigravity CLI · current profile**, with no email or invented human account. Its context is the current-user home, verified installed CLI path, controlled environment, fixed `/usage` operation and supported report protocol—not the selected OpenToken person's identity. User approval follows that current CLI profile, including account switches; no human/local-history attribution is made. `verifiedAt` is absent because no human identity was verified. Data is labelled provider-reported CLI quota, not independent proof of non-placeholder entitlement.

Old CLI-profile quota is discarded on restart and failed reads, expires at a reported reset or after one hour, and never rolls forward to full. Automatic settings backups omit this owner-operation grant, preventing corruption recovery from re-enabling it. Separate named app/IDE/OAuth modes are not silently replaced or merged.

Validation/deployment status is recorded in [VERIFICATION.md](VERIFICATION.md). Synthetic UI approval is never used to approve a production native dialog.

## Separately approved native CLI probe — actual result

After the user explicitly approved one official quota probe, the native debug executable ran the installed **agy 1.3.5** with `--print /usage --output-format json`. It first rechecked the version, disabled self-update, used noninteractive bounded pipes and required both existing-source and owner-recovery consent flags. No token, OAuth registration, temporary credential copy, browser import, model prompt or connection transaction was supplied by OpenToken. The official CLI was permitted to renew its own existing login.

The CLI exited successfully and returned a structured `command.name=usage` envelope with **zero turns and zero model-token counters**. The sanitized diagnostic reports:

- `gemini-weekly`: **100% reported remaining**;
- `3p-weekly`: **100% reported remaining**;
- both reported resets at `2026-10-26T18:57:47Z`;
- no five-hour bucket was reported, so none was invented;
- `identityVerified:false`, `persisted:false`.

This establishes that the official CLI quota interface executes on this installation and returns those observations. It is **not** independent proof of the intended human account, full/live subscription entitlement, or that an all-full report is not a provider default/placeholder. No Antigravity connection was added and no reading was assigned to the selected account. At that checkpoint, the result was diagnostic only. The new explicit CLI-profile connection above supplies source-context authority and a separate repeated-operation approval; it does not relabel this result as verified human identity.

Raw CLI stdout/stderr were captured only in bounded zeroizing native buffers and were not saved/exported. The research folder contains only the sanitized result (`usage-probe-live.json`). A companion check without recovery consent rejected before launching the CLI. The OpenToken settings file hash remained identical; cached API health remained **six scopes/all ready**. No production executable was replaced or restarted.

## Findings that change the implementation

### 1. IDE recovery selected the wrong transport

OpenToken opened the IDE but then forced verification through its persisted OAuth topic and a direct Cloud Code request. That bypassed the running official client's authenticated quota service. An expired/stale disk observation, or a direct-API client-context limitation, could therefore defeat recovery even when the official IDE could read its own quota.

**Correction staged:** selected IDE and standalone GUI recovery now verify their corresponding owned local service and persist `antigravity_app` / `antigravity_local`, not force the stored-token lane. Exact executable, current-user SID/session, process lifetime, profile, CSRF, PID-owned listener and before/after authenticated account checks remain required. A damaged saved IDE OAuth database must not block an otherwise valid running-client recovery.

### 2. The quota RPC has its own request schema

The installed standalone 2.23.1 binary's public protobuf descriptor defines `RetrieveUserQuotaSummaryRequest` with `request` and `force_refresh` fields. It does not define the GetUserStatus metadata envelope as that request's schema. OpenToken had reused that metadata envelope for both methods and never requested a fresh quota observation.

**Correction staged:**

```text
POST /exa.language_server_pb.LanguageServerService/GetUserStatus
{"metadata":{"ideName":"antigravity","extensionName":"opentoken","ideVersion":"unknown","locale":"en"}}

POST /exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary
{"forceRefresh":true}
```

The optional quota request override is omitted so the official service retains its own selected project/context. Quota reading creates no inference prompt. Existing engine spacing/backoff still bounds calls; `forceRefresh` is not permission to bypass those limits. Older services may use the existing conservative per-model GetUserStatus fallback only when the newer RPC is unsupported.

The synthetic server now rejects the old body rather than answering any arbitrary request with fixture quota. Windows integration additionally verifies explicitly selected IDE recovery with an intentionally unusable saved OAuth database. Earlier loose fixtures did not prove this compatibility.

### 3. Official CLI `/usage` is not model inference

The official [model quotas documentation](https://antigravity.google/docs/cli/commands/usage/) says `/usage` (alias `/quota`) refreshes backend quota and opens the quota panel. The [headless documentation](https://antigravity.google/docs/cli/headless/) explicitly distinguishes CLI-handled `/usage` from agent prompts, and says to run such commands as separate print invocations, not streaming user messages.

The candidate native command is:

```text
agy --print /usage --output-format json
```

This is materially different from a dummy `agy --print "hi"` inference. It is also different from `agy models`, which only lists model slugs. The installed native executable reports **1.3.5** through `--version`; automatic updating was disabled for that metadata check. Its static public binary contains usage-report symbols. No real `/usage`, recovery, OAuth or model command was executed during the initial research. The separately approved native `/usage` probe and its exact limits are recorded above.

Pinned independent protocol tests corroborate a structured envelope with:

- `status: "SUCCESS"`;
- `command.name: "usage"`;
- `command.data.groups[].buckets[]` containing `id`, `remaining_fraction`, `reset_time`, `disabled` and window information;
- zero `num_turns` and zero model-token usage for a CLI-handled report.

A production bridge must version-gate **before** launching the command, refuse unsupported versions, require the exact command envelope and zero agent work, bound output/deadlines, sanitize errors, and never treat a successful model-list exit as quota. The official CLI owns its credentials; it can renew its own saved session while reading quota. Therefore this cannot silently replace an existing read-only source permission: owner-side recovery/background effects need explicit approval.

**Identity caveat:** a quota report can omit authenticated human identity. Such a report must not be assigned to whichever account is currently selected in OpenToken, nor silently inherit an old account's cache. Independent source/identity binding is still required before persistent account monitoring. Upstream temporary credential staging/copying is not adopted as a solution.

### 4. There is an official quota observation payload

The official [status-line documentation](https://antigravity.google/docs/cli/statusline/) explicitly provides:

```text
quota[model_or_bucket_id].remaining_fraction
quota[model_or_bucket_id].reset_time
quota[model_or_bucket_id].reset_in_seconds
email
plan_tier
version
product
```

This is a promising native observation bridge: the real CLI supplies quota and authenticated context while the user uses it; OpenToken need not borrow OAuth or generate model work. It requires a separately approved native helper/configuration integration, preserving any existing custom status-line command and the built-in display. No hook or CLI setting was changed here. Missing/cold observations remain unavailable; old windows must expire rather than roll forward to full. The documentation does not promise a continuously fresh quota observation while every official client is closed.

## Why raw Google-bearer fallback is insufficient

Direct Cloud Code is an undocumented provider interface with client-family/tier/project context, not a public "any Google login = Antigravity quota" API. The existing stored-reader context request sends `{}`; first-party clients supply product metadata. Public trackers also contain compatibility User-Agent impersonation, client-registration reuse, token refresh/writeback, onboarding/project creation and production/daily quota merging. Those are not interchangeable read-only fixes and are **not** adopted.

The daily and production/GCP hosts are different pools. Matching identity is not authority to combine them. Use the official client's own selected context, or an independently justified fixed context; do not obtain a prettier meter by changing hosts or inventing missing buckets.

The official [SDK](https://antigravity.google/docs/sdk/overview/) is an agent runtime using Gemini API keys or GCP/Vertex, not documented proof of consumer Antigravity subscription quota. It is not a reason to install a Python runtime, create an agent or send inference for monitoring.

## Actual-machine evidence and limits

- Installed standalone **2.23.1**, IDE **2.0.4**, CLI **1.3.5**.
- Known `.gemini/antigravity`, `antigravity-ide` and `antigravity-cli` directories exist. Configuration directory names alone do not prove the credential namespace or signed-in identity.
- A consent-first native exact check found **no** `gemini:antigravity-cli` Windows entry. This used no vault enumeration or HTTP and exported no blob/token. The no-consent check refused before credential access. The shared entry was not silently replaced by a guessed alternative.
- Earlier native userinfo rejection proves those particular saved observations were rejected, not that every installed official client lacks working authentication.
- No matching running Antigravity/IDE/agy language-server process was present during the checks so far. A real local-RPC quota read remains unverified while those official app/IDE services are closed. The separately approved one-shot CLI result above is evidence for that different interface, not proof of the local-RPC path.
- Working Codex/Copilot/Cursor/Devin scopes are left untouched. The final cached API check reports **six ready scopes**, including Grok; no monitoring activation was performed by this investigation. Actual connection activation and optimized/native release acceptance must be recorded separately from this diagnostic evidence.
- **Initial staged correction:** 219 Rust tests, formatting and strict Clippy passed; native Windows synthetic-process tests covered the exact fresh-quota body and selected IDE recovery despite a broken saved OAuth database. At that checkpoint the frontend was unchanged. The later CLI-profile connection implementation changes frontend/persistence as described above; its separate release/deployment evidence is in VERIFICATION.md.

## Evidence provenance

Research artifacts are under `%LOCALAPPDATA%/OpenToken-build/antigravity-deep-20261019/`: official HTML/text and hashes; installed public-code/binary hashes and narrow descriptor snippets; pinned public-source snapshots/tests; sanitized native namespace results; build/test logs. No provider credential/database was copied into that research folder. No downloaded tracker, SDK, installer or public source snapshot was executed or bundled.

Public comparison pin: CodexBar `8fa003e092fe5e713fda8f7ace5c6f0b73305e64`, especially [CLI quota-report fixtures](https://github.com/steipete/CodexBar/blob/8fa003e092fe5e713fda8f7ace5c6f0b73305e64/Tests/CodexBarTests/AntigravityCLIUsageReportTests.swift) and [local quota request](https://github.com/steipete/CodexBar/blob/8fa003e092fe5e713fda8f7ace5c6f0b73305e64/Sources/CodexBarCore/Providers/Antigravity/AntigravityStatusProbe.swift). Fixtures are protocol evidence, not this user's live acceptance. Implementation remains original Rust; vendor/tracker runtime and borrowed registrations are not copied.
