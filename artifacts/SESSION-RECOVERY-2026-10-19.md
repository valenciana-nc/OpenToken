# Session-source and owner-recovery correction — 2026-10-19

**Second-pass correction:** [deeper Antigravity protocol research and staged fixes](ANTIGRAVITY-RESEARCH-2026-10-19.md) found that GUI IDE recovery must use the running client's local RPC, not force this document's stored-token lane. It also identified the exact fresh-quota request and the official non-inference CLI `/usage` interface. These corrections and the explicit CLI-profile connection are now deployed. The production window's two weekly meters were verified visibly after native approval; no verified-human identity is implied.

This continues [the installed-client investigation](COMPATIBILITY-2026-10-19.md). It does not claim that all twelve live entitlements work.

## Actual findings

The screenshots stopped at local expiry checks. Those timestamps were not sufficient evidence that the provider would reject a structurally valid bearer. Explicit native read-only diagnosis now reaches the fixed providers:

- **Antigravity IDE:** its exact persisted OAuth topic was successfully decoded; Google's userinfo endpoint rejected that saved session. This is a different source from the agy Windows credential entry.
- **Grok:** xAI userinfo rejected the saved OAuth session after local-expiry rejection was removed.
- **Claude:** the selected saved record supplies no usable access token. This is a structural source failure, not proof that a cached date expired or that the server rejected a request.
- No new connection, provider credential write/renewal, real recovery command, new OAuth flow, model prompt or Meta key issuance was performed by those diagnoses.

Native diagnosis outputs contain only provider, sanitized fault or quota counts, and `persisted:false`. Credentials remain inside native OpenToken. Debug and optimized native diagnosis independently agree on these outcomes; neither is a successful fresh entitlement. See [VERIFICATION.md](VERIFICATION.md) for the separate evidence levels.

## Existing-source reads remain read-only

Antigravity/Grok/Claude cache expiry is a hint, not an authoritative authentication result. A structurally valid bearer can reach a bounded, fixed-provider request even when its separate cached expiry is stale or missing. Grok recognizes RFC3339, numeric/string seconds and milliseconds without treating an unknown date as rejection. Existing-source Claude never invokes legacy renewal, including after an actual 401. Its usage request identifies OpenToken rather than impersonating a hardcoded Claude Code version. The separately approved legacy renewal path remains opt-in, locked and conflict-checked.

Discovery remains metadata-only. Neither discovery nor **Use existing** launches any client, renews a grant, issues a key, changes an account or sends inference.

## Exact Antigravity IDE source and quota context

The installed first-party IDE persists `antigravityUnifiedStateSync.oauthToken` in its standard `User/globalStorage/state.vscdb`. An original bounded native protobuf reader selects only USS `oauthTokenInfoSentinelKey` and its OAuthTokenInfo bearer. It does not use the refresh field. Exact-key read-only SQLite/WAL access creates no database copy/checkpoint/new sidecars. Base64/protobuf input, fields, lengths, token shape and duplicate selected fields are bounded/validated.

`antigravity_stored` is a distinct connection/report source. If neither owned desktop service is running, existing-source selection considers this IDE record before the separate agy credential entry. A malformed/rejected selected IDE source does not silently switch to a different client/account. Identity is verified before/after quota, with a final exact record/configuration reread. Token/context changes invalidate prior readings without buying another network read.

Daily Cloud Code and production Gemini/GCP are different pools. The installed IDE's public `cloudCode.js` selector chooses production while its tier is unknown or GCP, and daily for its established personal tier. Consequently the stored reader:

1. Rejects GCP token flags and non-daily/custom `jetski.cloudCodeUrl` configuration before forwarding a bearer.
2. Authenticates through fixed Google userinfo.
3. Performs fixed `cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` **tier-context lookup only**; requires an established tier and rejects production/GCP context. It never calls onboarding or production quota.
4. Reads only `daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary`, then verifies identity and source again.

It does not merge or substitute production quota because a bearer belongs to the same person. Unknown/onboarding-only context is unavailable, not a guessed personal allowance. A verified transition between the same IDE's running/stored source replaces that profile's old monitoring binding, without inheriting its old report or duplicating a badge. Separate CLI/app profiles remain distinct.

## Explicit official-owner recovery

These actions are separate from **Use existing** and require an explicit native command consent flag:

- **Antigravity → Session recovery:** choose the installed IDE, standalone app or agy CLI. GUI choices open only the selected known installed application. CLI choice executes only `agy models`. No installer, downloaded script, prompt or OpenToken OAuth registration is used. IDE recovery verifies/persists the stored source so an open IDE process is not subsequently required for each fresh-token read. Standalone recovery retains native executable/SID/session/lifetime/CSRF/listener safeguards.
- **Grok → Account setup → Recover Grok session:** executes official `grok models`. The CLI may renew its own session or ask the user to authenticate. Successful model-list process exit is **not** proof of subscription access: fixed-provider identity/quota verification must still pass.
- **Grok → Sign in with Grok:** separately runs `grok login --oauth`, matching the reader's auth.x.ai OAuth mode. A failed model check does not silently invoke this additional login command.

Client-owned renewal/authentication can mutate provider credentials; that is why recovery is a separate action, not a read-only fallback. Model commands send no inference prompt. Their real owner-side renewal behavior still requires live acceptance; synthetic success is not proof that a vendor will restore this user's grant.

Flows have ten-minute recovery deadlines, cancellation, duplicate-run/epoch protection and cached status without credential I/O. Provider pauses and verification budgets are honored before credential probing or launch. Successful owned CLI exit is required before quota verification; no green card is inferred from a process launch/exit. Cancellation terminates only an owned CLI child; GUI clients deliberately remain open, and completed provider-side effects cannot be undone. Disconnect removes monitoring, not vendor sign-in.

## Acceptance and remaining limits

Regression results at this stage: **218 Rust / 46 frontend / 81 browser** passed, with strict formatting/Clippy/TypeScript. Coverage includes stale/missing expiry with provider acceptance/rejection, exact stored topic/identity/source changes, database immutability, quota-context/configuration rejection, same-profile source replacement, pause-before-credentials, recovery cancellation/navigation/provenance and no implicit OAuth fallback.

Optimized packaging and enhanced synthetic native recovery/three-terminal-handle/console/tray tests passed. Production deployment and final live task/API preservation are recorded separately in [VERIFICATION.md](VERIFICATION.md). Five previously confirmed scopes (Codex twice, Copilot, Cursor, Devin) must remain unchanged. No fresh Antigravity/Grok/Claude entitlement is claimed until the owner actually restores the session and native verification succeeds. Muse installation/profile discovery and the API-key providers' actual coding/API entitlements remain unresolved live-source requirements, not inferred consumer-chat subscriptions.
