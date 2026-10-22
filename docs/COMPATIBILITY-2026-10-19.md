# Installed-client compatibility investigation — 2026-10-19

**Latest continuation:** [session-source and official-owner recovery correction](SESSION-RECOVERY-2026-10-19.md) supersedes the initial local-expiry conclusions below. Native requests now confirm provider rejection for the IDE's separate persisted Antigravity session and Grok; Claude's selected record lacks a usable access token. Recovery is explicit, never an existing-read fallback.

## Why fixtures were not enough

The reported Antigravity “No usable existing source” failure was genuine. The earlier optimized/native/HTTP fixtures demonstrated implemented transports, not compatibility with this user's installed profiles and credential lifetimes. The blanket end-to-end wording was too broad. This investigation keeps synthetic regression acceptance, real existing-session diagnosis, monitoring activation and new authorization separate.

No password, token, auth database copy, browser session or returned inference key was collected. Real source reads occurred only inside the native OpenToken executable after deliberately enabling existing-source diagnosis or the real app's **Use existing** action. No login, refresh grant, Meta key endpoint, model prompt, installer, session takeover or new OAuth authorization was started.

## Concrete findings and fixes — initial compatibility release

| Source on this machine | Finding | Change / boundary |
|---|---|---|
| Cursor 3.24.18 | The real Use existing action failed with generic invalid-data text despite a saved desktop session. The identity decoder was stricter than the canonical OIDC subject contract. | Numeric auxiliary `id` fields no longer invalidate a valid authenticated `sub`. Missing/mismatched subjects, invalid email, changed local token or changed remote identity still fail. Real native read now returns three quota meters. Stage-specific errors distinguish account identity, usage JSON and billing-period parsing; 403 no longer automatically means “sign in again.” |
| Devin desktop 3.10.67 | Installed/signed-in desktop state exists; CLI `credentials.toml` does not. The old reader only supported the latter. | Native exact-key, bounded read-only SQLite/WAL access to the actual Devin profile's `windsurfAuthStatus`, followed by the fixed private status RPC and post-read source/context verification. Real native read returns two percentage meters with `devin_app` provenance. No Windsurf/browser credentials are silently substituted. |
| Antigravity standalone 2.23.1 + IDE 2.0.4 + agy CLI | Both old/new Roaming folders coexist. Selecting the first existing folder can hide the running IDE. At diagnosis neither app/server was running. An existing CLI credential was found, but its access token was expired. | Verify both native desktop profiles, reject ambiguous active instances, and bind the actual verified profile. Tests cover both real executable names, including standalone `language_server.exe`, with both old/new folders present. Closed-app access can explicitly reuse a valid agy token through exactly `gemini:antigravity`, never vault enumeration/copy/renewal. Expiry requires official owner recovery; it is not “nothing installed.” |
| Grok CLI | Real Use existing reported an expired cached access token. | Explain that the saved sign-in was found and the official CLI owns renewal. Never silently renew it or relaunch authentication. An invalid relative `GROK_HOME` no longer falls back to a different default account. |
| Claude Code | Native read-only diagnosis requires session recovery; no refresh was sent. Claude monitoring was disabled in saved preferences and is not automatically re-enabled. | Existing-session checks do not insist on the proactive five-minute renewal window used by separately approved monitoring. A still-valid token with safe request headroom can be read without renewal. Expired/near-expiry sessions need official CLI recovery, not a new OpenToken-managed profile by default. |
| Muse | No supported Windows CLI executable or inline subscription profile was found at the documented known locations. | Distinguish missing installation from missing profile and from a cold MSP host with no observation. A web login is not a CLI quota grant. No dummy prompt, automatic install or credential-issuing fallback is used. |

API-key providers retain their actual scopes: Z.ai Coding Plan, Kimi Code, OpenCode Go and DeepSeek API balance are not browser/consumer-chat quotas. An installed client or website sign-in cannot supply a missing API entitlement.

An additional installed **Grok Bot 0.71.2** is not silently treated as the xAI CLI's auth profile. Its public package reports SpaceXAI and Cursor service integration. This is evidence of a different client/backend context, not permission to borrow its account, assume identical subscription meters or import its browser storage. Grok's current connector remains explicitly the official CLI credit/budget scope.

## First-party installed-code corroboration

Only public application resources were inspected, not private client transcripts or credential contents in scripts:

- Devin's installed workbench/extension code stores JSON at `windsurfAuthStatus`, contains the private status protocol and default `https://server.codeium.com`, and retains login-selected deployment context in `codeium.windsurf` global state (`apiServerUrl`). Native reads refuse unsupported desktop deployment overrides before sending a credential. User settings JSONC is bounded/parsed without regex URL extraction; provider/context changes are checked again after the request. Unknown/custom tenants are not forwarded to public cloud.
- Antigravity standalone's installed `dist/languageServer.js` uses `resources/bin/language_server.exe`, explicit process/CSRF/data-dir flags and **`https://daily-cloudcode-pa.googleapis.com`**. `dist/paths.js` distinguishes `.gemini/antigravity` from `.gemini/antigravity-ide`. These facts explain the executable/profile split.
- Current pinned Pane research identifies production Gemini as a different quota pool from daily. The native CLI reader therefore uses the evidenced **daily host only**. It never merges production Gemini merely because the Google bearer has the same user. Missing third-party buckets stay missing.
- The CLI credential parser accepts bounded UTF-8, UTF-16LE and `go-keyring-base64:` JSON envelopes, requires a still-valid access token, ignores renewal fields, verifies Google userinfo before/after quota and rereads the exact entry afterward. Google client IDs/secrets and refresh logic found upstream were not adopted.

Public installed-code hashes:

| Resource | SHA-256 |
|---|---|
| Devin workbench | `302af387606888341f563bbbac96db129c821602a9fa2bd7f37313899450a6e1` |
| Devin extension | `78df25665e0c8a5528a3116b5f88206d755d3974db4a2364fdd0b56c9fae1d06` |
| Antigravity languageServer.js | `5c781d05050506d7643a88b1eeadc9bb4f320d7133f84b075e6338a6ce158d26` |
| Antigravity paths.js | `43eeac76c59932cbcb398321a2802d9bb2d779155c035ef4895791e54bee769f` |

Protocol facts were implemented in original Rust/TypeScript, not copied vendor/tracker runtimes. No downloaded tracker/plugin/package was executed or embedded.

## Pinned public research

Research root: `%LOCALAPPDATA%/OpenToken-build/compatibility-20261019/`. `manifest.json` has **168 hash-verified entries / 163 unique files**, not a claim that every saved file was reviewed. Key revisions:

- CodexBar `8fa003e092fe5e713fda8f7ace5c6f0b73305e64`: Cursor canonical identity/usage summary, Grok CLI auth/credit transport, Muse and Antigravity source boundaries.
- Pane `5d2f43aa453d850a4e32e9a213601e60158f14c8`: updated Antigravity daily/production pool distinction, native Windows credential encoding and provider scope limits. Its renewal/write-back paths were deliberately rejected.
- OpenUsage Windows `971f4ba68870218a302191d2760abb552c274709`: Devin desktop exact authentication key and documented Connect service, independently corroborated against installed first-party code. Its disputed hidden-daily percentage interpretation was not adopted.
- OpenUsage `26132ff48dd78eb2c4a9498e40424e6365f99fec`: prior quota/window research.
- Official Muse SDK `d2d89a7dc4e7f00723334992f73970ff2cd16364`: typed observation protocol, not proof of a missing user's CLI/profile.

The old research evidence remains in [REMAINING-PROVIDERS.md](REMAINING-PROVIDERS.md); current source behavior is in [CONNECTIONS.md](CONNECTIONS.md).

## Safe real-source diagnosis

The native executable supports an explicit, read-only maintenance path:

```text
OpenToken.exe --diagnose-existing cursor --allow-existing-source-read
```

Supported providers: `cursor`, `devin`, `antigravity`, `grok`, `claude`, `muse`. Without explicit consent, or for an unsupported provider, no credential access starts. This path runs before logging/settings/startup/WebView/single-instance initialization. It does not save a connection, modify preferences, touch startup, expose a token getter or mint a key. Output contains only provider/source/counts or sanitized fault classification; no email, subject, credential, raw HTTP reply or CLI output. Native unit tests assert no persistence/vault use and consent-first rejection. It is deliberate real-source diagnosis, **not** part of normal synthetic regression tests.

All regression fixtures still isolate source homes, APPDATA and client roots. Isolated source homes cannot read the real user's vendor vault. The source watcher registers before verification, includes Devin desktop/context changes, throttles event-driven checks and respects existing network budgets. Disconnect does not delete/modify provider credentials or stop the provider. Automatic recovery now also omits removed connection grants, not just the always-excluded Muse credential-issuing approval.

## Acceptance is intentionally scoped

Cursor and Devin have real read-only existing-session acceptance on the installed versions above. After deployment, both were deliberately activated through the actual native Use existing buttons; the live app now reports five ready scopes (two Codex, Copilot, Cursor, Devin). An initial Cursor provider pause was honored before a later normal action succeeded. Deployment changed no settings; only connection metadata changed during those requested monitoring activations. Antigravity's exact credential source/encoding/expiry detection was exercised natively, but a fresh Google quota entitlement still needs an owner-renewed session. Grok and Claude need owner session recovery. Muse needs an actual supported installation/profile, and a cold host still may have no observation. No new sign-in, provider credential mutation or Meta key issuance was used to turn an error green.

Final synthetic suite/build/native results and live deployment evidence are recorded in [VERIFICATION.md](VERIFICATION.md). No statement here promises all twelve live subscriptions, perpetual closed-app access to expiring credentials, every client/profile, harmless Meta key replacement or invoice reconciliation.
