# Local usage and estimated value

The spending ring is backed by local usage records, **not just quota endpoints**. A subscription can generate token usage without generating a per-token cash charge. OpenToken therefore keeps these concepts separate:

- **Recorded tokens:** actual counters in Codex/Claude/Grok/Kimi/Muse CLI records, including subscription sessions and API-backed CLI sessions.
- **API-equivalent value:** an estimate using reference USD-per-token prices. This is not your subscription invoice, credit purchase total, tax, refund or actual provider charge.
- **Observed credit changes:** differences between ready, identity-keyed balance observations. Decreases and increases are separate. Expiry and top-ups can affect balances; changes between observations can cancel out. This is not an authoritative consumption ledger. Tracking begins with the first observation, with no invented backfill.

## Where to find it

Open the header's **Usage** pie-chart button for token history and spending. Choose **All providers** or a specific provider independently from the main quota grid. Today/Last 30 days counters, period selection, donut, legend, report copying and observed-credit disclosure remain available. Missing history is not zero spending. Model-rate links open **Providers & settings → App settings → Local usage & pricing** without saving unrelated edits.

Provider quota cards remain on the main page; their selected tab does not assign device history to an account. The Settings gear now contains both **Providers** and **App settings** sections. Add/provider-title shortcuts select Providers directly. Switching pages does not stop tracking.

## Account setup

**Providers & settings → Providers** defaults to **Use existing** access, not another login profile. **Track local usage** enables supported device/client records independently from quota authentication; it can be stopped under detected source capabilities without erasing imported totals. Optional official sign-in remains under explicit Account setup. Existing configured/discovered sign-ins keep working; there is no manual folder form or required migration. Disconnect stops monitoring without deleting provider credentials. See [connection setup](CONNECTIONS.md). New empty CLI homes do not invent token history or move old history to the newest identity; previously indexed counters retain their provider-level attribution.

## Sources and coverage

Enabled Codex homes contribute `sessions/**/*.jsonl` and `archived_sessions/**/*.jsonl`. Enabled Claude homes contribute `projects/**/*.jsonl`, including subagents. Configured homes, existing discovered account sources, environment overrides and ignored homes are respected. Explicit isolated homes with discovery off do not silently add the real default home (including during native smoke tests).

Disabling/removing a source stops new imports; already indexed historical counters remain until retention or an explicit index reset.

Grok, Kimi and Muse native readers are described in [SOURCE-ARCHITECTURE.md](SOURCE-ARCHITECTURE.md): completed turns, versioned runtime events, recorded timestamps, stable IDs and cache/reasoning subset safety; cumulative rollups are not additional calls. They need no credential or model request.

History is **this-device client-level**, not authenticated-account or billing-provider evidence. Logs cannot reliably prove which subscription identity was active historically, so OpenToken does not assign old costs to whichever account is currently signed in. Activity outside these logs—browser chats, unrelated API clients, other providers and deleted/disabled logging—is not recovered. Actual invoiced API costs require a billing-authorized data source; subscription quota access does not grant that access.

Today and Yesterday use local calendar boundaries; 30 Days includes today and the preceding 29 dates. Source event timestamps select periods. Credit changes use the time OpenToken observed the balance, not an asserted transaction time.

## Live tracking

**Settings → App settings → Live tracking** defaults on, including for existing settings files. The Rust background owner watches only enabled/configured history roots, even with the flyout hidden. After the CLI writes a complete usage record, changed-file indexing pushes token/value updates to the UI—typically within 1–2 seconds, with a little longer possible during backlogs, large imports or slow storage. This is live recorded usage, not a stream of every generated token; CLI buffering/turn completion controls when records exist.

Notifications are coalesced: 250 ms while visible, 800 ms while hidden, with maximum burst waits of 1/2 seconds to avoid starvation. Only one bounded indexing pass runs at once. Watcher failure/overflow is recovered with reconciliation; failed watching falls back to a 15-second full check with a visible periodic-tracking label. A normal full safety scan runs every minute. Turning live tracking off unregisters the watchers and retains periodic local indexing plus the selected idle quota interval. The frontend subscribes before reading its snapshot, rejects older revisions, and only checks the cache every two minutes as a safety fallback. Hidden windows stop their display-clock timers.

**Tray badges and mini provider stripes** update on engine snapshots, without a refresh click. Fresh log activity advances a verified account read: normally about two seconds after a turn if the last read was at least a minute ago, otherwise at the next one-minute budget boundary. While active, checks continue once a minute for up to ten minutes after the last activity, then relax to the selected **Idle quota refresh** (default five minutes). Read cycles share a rolling 60/hour per-identity budget within the running app, including manual requests. Server backoff still wins. CLI failover/internal provider retries can involve multiple transport requests within one read cycle. Browser chats/unlogged API clients have no local activity signal, so only the idle checks can observe them.

Codex `event_msg/token_count` records often include rate-limit snapshots, but **normally no account identity**. A folder, timestamp or latest sign-in is not identity proof: these snapshots trigger a provider read, not a guessed percentage on the current account. A provisional direct update is accepted only if an optional explicit `payload.account` identity matches the already verified identity, current file-auth checks pass, the record is fresh, status is ready, the reset window is unchanged, and used quota has not decreased. Current anonymous Codex logs normally use the provider-confirmed route. Keyring/ambiguous records and Claude limits always use authenticated reads. The UI, tray tooltip and API expose `cli_log` versus `provider` provenance; credits are observed only from ready provider reports. No reset or quota change is inferred from elapsed time or token counts.

Provider APIs may lag, throttle or be offline, so sub-second quota availability cannot be guaranteed. Last provider verification and provisional/cached states remain distinct. Local recording and API-equivalent pricing continue independently; estimates are still not subscription charges.

## Parsing and deduplication

New Codex `token_usage_record` entries use stable response IDs. Legacy `event_msg/token_count` entries fall back to changing cumulative counters and `last_token_usage`; repeated quota echoes do not create extra calls. Matching new/legacy records are not both charged. Copied response records deduplicate across homes; turn/timestamp/counter keys cover legacy copies. Codex reasoning output is already part of output tokens and is not added twice. Cached input is subtracted from the total input before applying uncached pricing.

Claude assistant records use stable message IDs. Streaming revisions and repeated tool/content blocks merge maximum counters rather than sum the same call again. Regular input, cache reads, five-minute/hour cache writes and output are separate. Synthetic zero-token errors do not create spend.

Only metadata/counter fields are decoded. Prompt, assistant-content, tool-output and instruction fields are ignored and never stored or shared. The index stores sanitized counters and cursor/parser state, never credentials or transcript text.

## Pricing

`spending/prices.json` is a pinned, bundled LiteLLM reference snapshot with source revision/date. No prices are fetched at runtime. Known cached-input, cache-write, long-context and recorded service-tier rates are applied. Tools, taxes and unrecorded/special billing arrangements are not included. A secondary reference catalog is not a provider invoice or a guarantee of current pricing.

Settings → App settings → Local usage & pricing lists used reference rates and allows exact-model, fixed USD-per-million overrides. All five override fields require explicit values; zero explicitly means free, not unknown. Overrides replace tiered reference pricing and recalculate retained estimates, not historical invoices. Unknown models stay unpriced: a mixed-priced ring shows `≥` and the known dollar value as a lower bound, disclosing unpriced tokens. If no positive-priced portion exists, it shows tokens rather than inventing dollars. Sample-only values remain confined to the visibly labelled demo.

Maintainers can run `node scripts/update-prices.mjs` to refresh the pinned snapshot, then review changes and rebuild. See [third-party notices](THIRD-PARTY.md).

## Storage, performance and recovery

`%LOCALAPPDATA%\OpenToken\usage.sqlite3` (and SQLite WAL/SHM files) contains the index. SQL transactions commit event counters and file cursors together. Restart, append, partial last lines and replay do not double-charge records. A truncated/replaced file is reparsed using stable event IDs. Usage events have a rolling 32-day logical retention window; this is not secure erasure. Verified credit observations and file cursors are retained separately.

Indexing runs on a native blocking worker, not the UI thread. One request imports at most 64 MiB, with 8 MiB per file, a 2 MiB line-allocation ceiling, depth 8, 8,000 directories and 40,000 files per traversal. Symlink/reparse traversal is refused. Exceptionally large or inaccessible trees may therefore have incomplete coverage. Giant non-usage lines are discarded without retaining their content. Malformed/oversized usage records are counted and disclosed; incomplete final appends are retried. While a backlog exists the background owner resumes bounded passes and labels totals partial, without requiring an open UI. Changed paths avoid enumerating the whole tree on each write. Parsing/pricing makes no network requests; fresh activity may separately request a budgeted, identity-checked quota confirmation as described above.

The maintainer probe `cargo run -p opentoken-core --example usage-summary -- <isolated-index-dir> codex=<home>` prints aggregate counters/value without transcript content. Use an isolated index directory for diagnostics, never attach raw CLI logs to issues.

To reset local history, exit OpenToken before removing only `usage.sqlite3` and its WAL/SHM files. Existing logs will be reindexed; the credit-observation baseline restarts. Do not delete CLI credentials or session logs.
