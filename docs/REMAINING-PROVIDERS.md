# Native Antigravity, Devin and Muse monitoring

## Outcome and evidence boundary

**Latest source/recovery correction:** [provider-authoritative bearer checks, exact IDE stored source and explicit official-owner recovery](SESSION-RECOVERY-2026-10-19.md). This supersedes the local-expiry conclusions in the earlier findings below; confirmed working connections remain unchanged.

**2026-10-19 installed-client correction:** [the deep compatibility investigation](COMPATIBILITY-2026-10-19.md) supersedes the initial path assumptions below. Cursor's identity decoder and Devin desktop source were fixed and passed real native read-only usage checks. Both Antigravity desktop profiles are now considered, with a separate valid-token-only agy credential reader for closed apps. Expired Antigravity/Grok/Claude sessions and absent Muse installation/profile remain explicit owner-recovery/setup requirements, not fake success. Earlier synthetic results below remain historical.

Follow-up public-source research found a substantially better **Devin CLI credential** path, a newer authoritative **Antigravity quota-summary RPC**, and **Muse's official subscription-observation protocol**. The old conclusion that Devin necessarily required browser-session extraction was too narrow. Muse is not restricted to an inference-key mint response either, although its read-only alternative has an important cold-start limitation.

**Implementation continuation:** all three now have original native paths in `crates/opentoken-core/src/local_providers{.rs,/}` integrated through engine/IPC/cache/refresh/persistence/API/cards/tray/disconnect. Antigravity's owned Windows RPC and Muse's stdio observation path passed real-release synthetic-process/restart acceptance. Devin's private status and Muse's separate key-endpoint mode passed actual loopback HTTP lifecycle tests; 200 Rust / 46 UI / 75 browser tests and native checks passed. This does **not** establish real private-API entitlement or universal profile compatibility. No real vendor credential, downloaded tracker/plugin/installer, browser storage, OAuth, model prompt or Meta key mint was used by tests.

**Muse now has two deliberately different modes.** Default **Use existing** invokes only the official last-observation protocol; a new host can have none. Advanced subscription refresh is an optional credential-issuing request, **not read-only**: a separate default-No native confirmation warns about undocumented replacement effects/running-session risk and repeated background calls until disconnected. It requires actual authenticated identity/quota, discards returned inference keys, never uses/stores them or writes Muse's file, and cannot undo a server request already sent. It is not an automatic fallback. Automatic settings backups omit this approval. Read [CONNECTIONS.md](CONNECTIONS.md) and [SECURITY.md](SECURITY.md) before enabling it. Default observation/local-history support remains usable without accepting this operation.

Source snapshots, immutable revisions and SHA-256 manifests: `%LOCALAPPDATA%/OpenToken-build/provider-research/remaining/{manifest.json,official/manifest.json}`. Downloads alone are not proof every file was reviewed. The specific files discussed below were inspected. Implement in original Rust; do not bundle these trackers/plugins or run them as helpers.

| Provider | Best native next route | What is still not established |
|---|---|---|
| **Devin** | CLI TOML or bounded read-only Devin desktop SQLite authentication record, deployment/context binding and honest status RPC metadata | Desktop 3.10.67 real read accepted; other versions/tenants remain scoped. Hidden-daily ambiguity stays raw/unmapped |
| **Antigravity** | Owned app/IDE RPC with both-profile selection, or explicit read-only valid agy credential / daily Cloud Code source | Native credential decoding/expiry detection accepted; fresh-token quota entitlement not yet live-accepted. No OAuth renewal or history importer |
| **Muse** | Implemented official MSP observations plus a separately approved quota-bearing key endpoint | Cold host may have no observation; key endpoint is credential-issuing with unknown replacement effects, not harmless/fresh-quota guaranteed; keyring-only profiles not imported |

## 1. Devin: official CLI credentials, not browser scraping

### New evidence

[Official Devin authentication documentation](https://docs.devin.ai/cli/enterprise/devin-auth) explicitly documents the native Windows path:

```text
%APPDATA%\devin\credentials.toml
```

The official CLI owns `devin auth login` and `devin auth logout`. Its credentials are sensitive and persistent; OpenToken should never copy, rewrite, remove or rotate them. The official [command reference](https://docs.devin.ai/cli/reference/commands) lists `devin auth status`; that command establishes authentication status, not subscription quota. Its interactive `/usage` is session consumption, not remaining account allowance.

Independently inspected `mesomya/openusage-windows`, `ItsJazii/pane`, and `robinebers/openusage` implement a different reader from CodexBar's browser organization-quota path:

```text
credential field: windsurf_api_key
reported CLI server setting: api_server_url
default fixed destination:
  POST https://server.codeium.com/exa.seat_management_pb.SeatManagementService/GetUserStatus
headers:
  Content-Type: application/json
  Connect-Protocol-Version: 1
body credential location:
  metadata.apiKey
```

The `windsurf_` field name is historical; sources show Devin CLI credentials such as a `devin-session-token$...` session. It is not proof that arbitrary Windsurf credentials, a generic `cog_` developer key or every organization can access personal Devin quotas. Do not query a Windsurf credential database to pretend it is a Devin login.

Useful response fields in the inspected sources:

- `userStatus.planStatus.planInfo.planName`, `isDevin`, `hideDailyQuota`, `billingStrategy`.
- `dailyQuotaRemainingPercent`, `weeklyQuotaRemainingPercent`.
- `dailyQuotaResetAtUnix`, `weeklyQuotaResetAtUnix`.
- `overageBalanceMicros` — a separate reported balance, not a percentage or invented allowance.

### Native implementation

1. Metadata-only discovery of the known current-user CLI file. Do not read secrets at render/discovery time, search all drives, or launch login when it is absent.
2. Explicit **Use existing Devin** authorizes a bounded, regular/unlinked-path-checked, zeroizing read. Use a real TOML decoder with top-level field checks, not a regex that accidentally reads keys under another table. Honor the actual absolute Windows APPDATA root and isolated fixture roots.
3. Fixed HTTPS host/path, redirects disabled, bounded response/deadline, sanitized errors. A configured unknown enterprise/server URL is **unsupported**, not an excuse to send the key to an arbitrary host or silently send that deployment's credential to the default cloud.
4. Upstream clients hardcode IDE names/version `1.108.2`. OpenToken does **not** adopt the fake version or a borrowed OAuth registration. Its request uses `ideName=devin` as monitored product context, `ideVersion=unknown`, and its own extension name/version and User-Agent. Live private-server compatibility still needs acceptance.
5. Require a useful quota/balance response and justified context before saving monitoring. Bind actual authenticated subject/tenant when supplied; otherwise use explicitly labelled CLI credential scope, not a fabricated human email or an org guessed from a folder. Reread the exact local credential/server context after the response; reject a switch. Never transfer an old scope's cached readings to a new credential.
6. Continue through native engine scheduling/budgets/backoff and monitoring-only disconnect. Official CLI remains the authentication/credential owner. Add app-state SQLite reuse only later, as a separately justified opt-in source; it is unnecessary for the first CLI implementation.

### Do not copy these upstream semantic shortcuts

The inspected implementations **disagree**:

- `mesomya/openusage-windows` maps `dailyQuotaRemainingPercent` directly to **used** on a weekly row when `hideDailyQuota` is true and no weekly percent exists.
- Pinned `robinebers/openusage` and `ItsJazii/pane` flip remaining to used, including their hidden-daily fallback. They also treat an omitted weekly value plus reset as an exhausted proto3 zero.

Those are conflicting implementation claims, not independent live verification. A reset timestamp alone does not prove the field is an omitted zero rather than an unavailable/unsupported meter. Preserve missing as unknown unless the actual versioned protocol proves default-value semantics. Do not relabel a daily field as weekly merely to keep a tile populated. Validate one deliberately authorized real CLI/app reading and capture **sanitized field values, displayed meaning, plan and version**, not credentials. Add fixtures for explicit zero, missing, malformed and both hide-daily modes. Prefer explicit unambiguous meter fields; leave an ambiguous meter unavailable rather than invert it speculatively.

Public [Devin v3 APIs](https://docs.devin.ai/api-reference/overview) are a **separate** potential organization/enterprise analytics connector. ACUs, service-user/PAT permissions and organization consumption are not automatically the CLI seat's daily/weekly allowance.

## 2. Antigravity: owned local RPC, authoritative bucket identities

### New evidence

Windows Rust research (`ItsJazii/pane`) finds the language server with process metadata and listening ports; the inspected code uses PowerShell/CIM plus `netstat`. That demonstrates a concrete Windows discovery approach, but OpenToken should implement the same role with native documented Windows APIs, not spawn scripts or inherit its weaker process checks.

The newer `robinebers/openusage` implementation calls:

```text
POST https://127.0.0.1:<owned-port>/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary
Content-Type: application/json
Connect-Protocol-Version: 1
x-codeium-csrf-token: <that server's current CSRF>
```

Some builds also serve HTTP loopback. The summary can be `response.groups` or bare `groups`. The inspected exact shared bucket IDs are:

| Bucket ID | Meaning in this source revision |
|---|---|
| `gemini-5h` | Gemini five-hour pool |
| `gemini-weekly` | Gemini weekly pool |
| `3p-5h` | Third-party-model five-hour pool |
| `3p-weekly` | Third-party-model weekly pool |

Each usable bucket supplies `remainingFraction` (0–1) and optionally its reset. Used percent is `100 * (1 - remainingFraction)`. Some accounts have a separate explicitly weekly Argon group; it must not be merged into shared Gemini limits by substring matching. Unknown/missing/disabled/ambiguous buckets remain unavailable.

`GetUserStatus` supplies plan/session information and older per-model quota. `GetCommandModelConfigs` is another legacy source, but it does not independently prove account identity. Neither legacy endpoint justifies manufacturing missing weekly windows. A parsed partial summary must not fall through to legacy code that interprets absent quotas as depletion.

### Native implementation

1. Explicit access to the existing running app; no automatic `agy -p /usage` on a timer. Prior installed-help research showed print mode can enter authentication and does not itself establish account identity.
2. Use narrowly filtered Windows process metadata (native WMI/COM where command-line data is required), actual executable path/known installation provenance, current-user SID/session and process creation time. A process **name or Antigravity-looking command-line string is not sufficient**: Windsurf-derived servers share names/protocols.
3. Bind TCP listener ownership with native IP Helper APIs to that exact process/lifetime. Parse flags with correct Windows quoting; keep CSRF native-only and out of logs/IPC. Bound candidates and ports; never scan arbitrary localhost services.
4. Construct a loopback-only client with **no proxy and no redirects**. Any self-signed-certificate exception belongs only to the verified loopback transport, never the general provider HTTP pool. Revalidate ownership/lifetime and source scope around the read; discard a result if the process/profile/session changed.
5. Bind reported authenticated identity when available. If the protocol cannot establish a human, model an explicit current-client/session scope and corroborating credential/profile fingerprint rather than attach the quota to whichever email is stored in that folder. Do not carry unidentified readings across sign-in changes or restarts as if they belonged to a verified account.
6. Parse explicit bucket IDs and windows, retain separate shared pools, and expose provenance. Missing percentage is neither full nor empty. Use existing refresh/backoff and monitoring-only disconnect.

### App-closed CLI route (implemented with valid-token-only scope)

Two inspected Windows implementations identify the CLI-owned Windows Credential Manager target **`gemini:antigravity`**. The payload can be JSON (sometimes UTF-16LE / `go-keyring-base64:` wrapped) containing `token.access_token` and `token.expiry`. This is a concrete optional read-only session source, **not permission to enumerate or import the user's vault**.

The explicitly authorized native adapter now reads exactly that entry, uses only a still-valid access token, corroborates real Google identity and queries the first-party daily Cloud Code quota endpoint (`/v1internal:retrieveUserQuotaSummary`). Newer evidence shows production Gemini is a different pool, so it is never merged; daily is used alone and missing buckets stay unknown. The real entry was found and decoded natively but expired. A fresh-token quota entitlement still needs owner recovery/live acceptance.

The researched implementations also extract/hardcode Google's installed-app OAuth client credentials and refresh on their own. OpenToken must **not** adopt that part. Do not copy the vendor credential into the OpenToken vault, perform refresh with borrowed client registration, rewrite the CLI store, or launch authentication implicitly. If the token expires, the official app/CLI owns recovery; the always-available closed-app refresh problem is not solved merely by finding a credential target.

## 3. Muse: official read-only observations before any minting fallback

### New first-party evidence

The [official Muse changelog](https://dev.meta.ai/docs/muse-code/changelog), release **1.4.0**, explicitly adds:

> `usage/read` returns the last-seen 5-hour and weekly windows without spending a prompt, and `usage/changed` pushes updates when they move.

The official [MSP TypeScript declarations](https://github.com/meta-models/muse-code-sdk/blob/ae469342dfe6ccd5142947af5f7dcdd5d50ccc18/schema/msp/msp.d.ts) confirm:

```text
usage/read result: { usage?: SubscriptionUsage }
usage/changed params: SubscriptionUsage
SubscriptionUsage:
  observedAtMs
  tier
  window: { usedPercent, resetsAtMs, windowDurationMins }
  weekly: { usedPercent, resetsAtMs }
```

`usage` is **omitted**, not a fake full quota, when the host has observed nothing. The protocol calls these point-in-time observations and explicitly says to render "as of", not imply live data. Percent used can exceed 100; preserve the reported fact while clamping only a remaining/progress display as needed.

[Official Muse documentation](https://dev.meta.ai/docs/muse-code) confirms native Windows support and `muse serve`/`muse schema`; [authentication](https://dev.meta.ai/docs/muse-code/auth) and [subscriptions](https://dev.meta.ai/docs/muse-code/subscriptions) distinguish the CLI subscription from extra pay-as-you-go API keys. A normal Meta inference key is not a substitute subscription quota grant.

### Native implementation and the unresolved bootstrap

Write a minimal bounded Rust JSON-RPC/MSP consumer using the installed official executable/authorized transport, existing CLI-process safety and honest client identity. There is no need to bundle Node, the TypeScript SDK or another tracker. Read `usage/read`; subscribe to `usage/changed` **where a legitimately accessible initialized host/transport exists**. Persist only typed quota observations with actual observed time and correct source scope; never serialize device/inference tokens. No `turn/start`, dummy prompt, automatic session creation/resume, sandbox changes, MCP/hook execution, browser login or vendor credential writes just to acquire a meter.

However, this is **not yet a proven drop-in observer of every ordinary Muse terminal session**. The inspected BB provider owns the `muse serve` hosts that perform the user's real work. Its comments explicitly say a fresh host with no model call can know nothing, and it caches observations from its already-running hosts. Its cache is third-party state, not an official Muse-wide quota database. Do not claim simply spawning another `muse serve` makes those existing hosts' readings available, intercept their stdio, take their session writer lease or install BB as a runtime dependency.

First prove, using a safe isolated protocol probe and then a deliberately authorized existing source, whether the installed first-party version exposes a reusable read-only transport or durable native subscription observation. If yes, connect it with source/identity-switch safeguards. If no, request/use a first-party observer/export facility or explicitly scope support to available observations. Existing Muse local-token tracking remains useful independently; it must not be relabelled remaining subscription quota.

When an observation ages past its reset, mark it stale/unknown pending a real reading. **Do not** roll reset dates forward and declare 0% used from inactivity; another client/device can have consumed the new window.

### Minting remains a separate, permissioned design decision

CodexBar and `CodingChiao/magpie-muse-code` obtain `subs_usage` through:

```text
POST https://api.meta.ai/muse-code/key
Authorization: Bearer <CLI device credential beginning dca:>
```

That response can include a model API key plus subscription/identity/payment metadata. The Magpie implementation adopts newly minted keys and can write them back to the CLI file. **Do not copy that behavior.** Neither a misleading "Use existing read-only" label nor discarding a returned key makes a credential-issuing request side-effect-free. Effects on key lifetime/replacement, payment requirements, idle-window omission and account binding must be understood before any optional explicit consent for this route. It cannot be silently substituted when MSP has no observation. The implemented advanced mode therefore requires a separate native warning, user approval and saved `muse_device` grant. It explicitly warns that replacement effects are unknown and could affect running sessions; no claim that key issuance is harmless is made. The returned key is discarded/zeroized where owned, not adopted or written back. Network/save/cancellation failures cannot undo a request already sent.

The first-party docs confirm Windows support, but neither the macOS Keychain implementation nor third-party `auth.json` path guesses establish all supported Windows credential-storage modes. Validate the installed vendor version's contract before reading credentials. No new Muse login/key mint was attempted in this research.

## Implementation order and acceptance gates (continuation executed)

1. **Devin native CLI reader first**: official Windows credential location and an implemented quota protocol are now evidenced. Gate ambiguous quota-field mapping and caller compatibility on a real, intentionally authorized comparison; useful explicit balance/clear meters can be supported without fabricating the rest.
2. **Antigravity desktop RPC second**: implement native ownership/profile/lifetime checks and summary parsing together. A localhost response alone is not acceptance. Keep optional CLI credential reuse separate and never borrow OAuth renewal registration.
3. **Muse protocol feasibility probe before claiming full quota support**: native MSP transport is implemented, but a cold host still may have no observation. This is not hidden by creating work or intercepting another terminal. A separate user-approved, credential-issuing API mode now supplies actual returned quota when available; its risks and missing/idle-data boundary are explicit. No harmless-key-replacement or universal read-only bootstrap claim is made.

For each implemented path: isolated HTTP/CLI/process fixtures, account/source-switch rejection, secret-free serialization, bounded/cancellable operations, shared read gates/backoff, targeted settings persistence/rollback/restart, explicit disconnect leaving provider access untouched, honest UI/tray provenance and independent local-history controls. Existing Codex accounts, production app/task, QuotaTray and Windows preferences must remain untouched during tests.

## Pinned sources inspected

| Repository | Revision | Relevant inspected paths / use |
|---|---|---|
| [mesomya/openusage-windows](https://github.com/mesomya/openusage-windows) | `971f4ba68870218a302191d2760abb552c274709` | `plugins/devin/plugin.js`, `docs/providers/{devin,antigravity}.md`, relevant plugin fixtures; CLI fields, Windows paths, Connect RPC, semantic conflict |
| [robinebers/openusage](https://github.com/robinebers/openusage) | `26132ff48dd78eb2c4a9498e40424e6365f99fec` | `Providers/{Devin,Antigravity}/*Usage{Client,Mapper}.swift`, Antigravity provider, provider docs, selected quota fixtures; explicit summary buckets, remaining-field interpretation, unsafe-for-us borrowed renewal registration |
| [ItsJazii/pane](https://github.com/ItsJazii/pane) | `3ec67cbfae6075fd8431cea7ca0c570dc889cf07` | `src-tauri/src/providers/{devin,antigravity}.rs`; concrete Windows/Rust discovery and CLI reuse, host protection, semantic shortcuts to avoid; no code adopted |
| [aqua5230/usage](https://github.com/aqua5230/usage) | `88c3becd13c17bbb907074daa43b574e46c0668b` | `loaders/agy_quota_probe.py`, relevant tests, README; exact Windows vendor credential target/encoding; its renewal/client registration is not adopted (AGPL source read for protocol research only) |
| [meta-models/muse-code-sdk](https://github.com/meta-models/muse-code-sdk) | `ae469342dfe6ccd5142947af5f7dcdd5d50ccc18` | `schema/msp/msp.d.ts`, SDK README/client; official optional observation schema, timestamps, truthful absence; registry metadata identifies SDK 1.4.10; package not installed/executed |
| [braedonsaunders/bb-plugin-provider-muse](https://github.com/braedonsaunders/bb-plugin-provider-muse) | `d6b808d4f32193cd229c78462410e85546c30866` | `src/subscription-usage.ts`, MSP declarations, selected usage integration and README; observation-only hosts and cold-start limits, reset extrapolation not adopted |
| [CodingChiao/magpie-muse-code](https://github.com/CodingChiao/magpie-muse-code) | `c195923dfb41b3daa9a5a78a8ebe7b6e05d2f122` | `index.mjs`, selected fixtures, README; confirms quota-through-mint and credential-write side effects; borrowed OAuth client and write-back not adopted |

Additional candidate repositories/snapshots are in the external manifest. A marketing README, green fixture, downloaded file or typed field name is not proof of a successful live provider reading. This record preserves the research rationale and current implementation boundary. Code/fixture/native acceptance does not establish live provider authorization, safe key replacement or every profile/version.
