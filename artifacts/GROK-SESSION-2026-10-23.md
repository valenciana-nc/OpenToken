# Grok session lifetime and sign-in handoff — 2026-10-23

## What was actually wrong

The prior resilience change did not renew rejected Grok sessions. The selected saved profile had a six-hour access-token lifetime: file update `2026-10-22T15:39:53Z`, expiry hint `21:39:52Z`, followed by real xAI identity rejection logged at `21:44:58Z`. Expiry alone is not authentication evidence; the corresponding HTTP rejection is the operative failure. No Grok sign-in process was still running when the later failed handoff was inspected. A browser response cannot complete a CLI attempt whose process has already ended.

The installed official CLI contains its own authentication documentation. It documents owner-side token refresh, `grok login --device-auth`, and ordinary browser OAuth. Its OAuth fallback prompt is **Paste the URL here if it doesn't connect**: it accepts a complete return URL, not an arbitrary code-only field in OpenToken. The UI had not explained this step. Existing terminal-handle tests proved a terminal existed; they did not prove that a person could finish the browser/terminal handoff.

Read-only inspection of the installed executable (SHA-256 `9891863f48c0cd973930d1e54fb28b02adb10521b2704d0ba7a3c625c26bb94d`) and a separately isolated, update-disabled `--version` call identified **grok 1.0.67 (2fe064390d07)**. Embedded documentation was read as protocol evidence, not copied into or bundled with OpenToken. No provider runtime is shipped. Selected auth metadata was inspected in memory; no access/refresh token, client registration, callback URL or browser code was exported or committed.

## Sign-in changes

- **Sign in with Grok** requests the installed CLI's `login --device-auth` flow. Enter the terminal's device code on the official page it opens. Keep the terminal open until the CLI confirms completion. This avoids the loopback/browser-return-code handoff.
- **Browser return-code help → Use browser sign-in instead** explicitly retains `login --oauth`, with full-return-URL instructions and a recognizable **OpenToken - Grok sign-in** terminal title. Alt+Tab returns to it. Expired/closed attempts require a new attempt; codes are not entered in OpenToken or chat.
- Explicit sign-in launches the requested official command instead of reporting success merely because an old saved session is currently usable. Recovery remains a separate one-shot `models` action; there is no automatic extra login command.
- A successful process exit followed by a missing/rejected/changed session stops with an actionable error, rather than waiting on an unchanged token marker until timeout. Provider identity and quota must pass before a connection is published.
- Cancellation and the ten-minute sign-in deadline still stop only the owned process. No password, terminal output or browser response is collected by the sign-in UI.

## Optional owner-maintained monitoring

`grok_cli` remains read-only. **Enable automatic Grok renewal…** requires a separate owned default-No native Windows review. Only a successfully verified upgrade of an already saved Grok account becomes `grok_cli_renewal`. Merely opening the review, reading a source, selecting a provider, navigating, signing in or accepting a diagnostic is not this grant.

For the approved mode:

1. Read identity/quota normally. Only an actual `NeedsRelogin` failure can invoke the owner operation; cached expiry is not sufficient.
2. Re-read the selected profile, require the same saved subject and the standard `https://auth.x.ai` issuer. Never follow a different CLI user implicitly.
3. Acquire the same owner-operation lease used by interactive Grok sign-in. No overlap from OpenToken is permitted. Another independently started Grok process remains the official CLI's concurrency responsibility.
4. Allow at most one owner attempt per 15 minutes per profile/process lifetime. Existing provider gates and Retry-After remain in force; no extra quota cycle is purchased.
5. Validate bounded, unlinked paths and the supported standard configuration. Custom authentication/endpoints/helpers/hooks/managed policy are refused, not modified. UI/default-model preferences remain intact.
6. Execute only `--version` (5 seconds) and `models` (25 seconds); the reviewed version must match **1.0.67 (2fe064390d07)**. An unknown/future build stops before `models`. The executable metadata is checked around calls. This is not a cryptographic attestation against a hostile same-user replacement.
7. Clear ambient Grok/xAI/auth/config/debug overrides, disable self-update/telemetry for the child, use noninteractive stdin and bounded zeroizing stdout/stderr (64 KiB each). No output, credential or model list reaches WebView/logs. No model prompt or automatic extra `grok login` command is sent.
8. Re-read the local session and verify the original subject, authenticated remote identity and actual quota again. A changed identity, unreadable result, denial or failed persistence cannot publish success. Owner-maintained reports do not fall back to old quota after failure. Total observation deadline is 60 seconds.

The official CLI may renew/replace its own credentials and may request authentication through its own browser. This is explicitly disclosed as **not read-only**. OpenToken never implements an OAuth refresh grant, borrows a client registration, copies credentials to its vault or invents entitlement. Provider-side effects already completed cannot be undone by cancellation or rollback of OpenToken settings.

The grant survives restart only in primary settings. Automatic recovery backups omit it. Disconnect removes monitoring without revoking/deleting provider credentials; successful read-only reuse or a new sign-in replaces the grant with ordinary `grok_cli` mode. Unsupported versions, revoked refresh grants or provider failures still require explicit recovery; this is not a promise that authentication can never expire or be revoked.

## Acceptance and limits

**239 Rust / 59 frontend / 97 browser** tests and optimized/native acceptance passed. HTTP fixtures, a synthetic owner, native synthetic sign-in children and the actual default-No dialog exercise the boundaries described in [VERIFICATION](VERIFICATION.md). All unrelated working scopes/preferences and the one approved Scheduled Task remain deployment invariants.

Tests are not real-account authentication evidence. Production renewal, real device-code completion and a later six-hour expiry require separate live acceptance. The publication contains no real credentials or private account identities. Logs/evidence stay outside Git under `%LOCALAPPDATA%/OpenToken-build/grok-session-20261023/`.
