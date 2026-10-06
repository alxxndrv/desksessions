# desksessions BLE protocol — version 1

The clock is a GATT peripheral; the Mac is the central. All characteristics require an
encrypted, bonded link (LE Secure Connections, passkey). Integers are little-endian.
Strings are UTF-8, length-prefixed, never NUL-terminated.

## GATT

| Item | UUID | Properties |
|---|---|---|
| Service `desksessions` | `b4d4f873-8133-48f0-9479-88481c6d52cf` | primary, advertised |
| `snapshot` (Mac → clock) | `8c9ccb7b-95a9-411e-a3db-00b974bbb1ea` | write (encrypted) |
| `events` (clock → Mac) | `6fbc6994-1373-4ace-9710-e9bf939d9251` | notify (encrypted) |

Device name: `desk-XXXX`, the last two bytes of the BLE address in hex.

## Snapshot (Mac → clock)

The Mac sends the whole state every time anything changes, and once on every connect. A frame
can exceed one ATT write, so it is split into chunks.

### Chunk

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | `frame` — sequence number, wraps at 255 |
| 1 | 1 | `index` — 0-based chunk index |
| 2 | 1 | `count` — total chunks in this frame (1–8) |
| 3 | n | payload slice |

Chunks are written in order with write-with-response. The clock reassembles by `frame`; a chunk
for a new `frame` discards any partial one. Each chunk is at most `min(MTU − 3, 512)` bytes.

### Frame

| Size | Field |
|---|---|
| 1 | `version` = 1 |
| 1 | `flags` — bit 0: flip the screen 180°, bit 1: swap the buttons; others 0 |
| 4 | `secondsOfDay` — Mac local time, seconds past midnight |
| 1 | `nightStart` — hour 0–23 |
| 1 | `nightEnd` — hour 0–23 |
| 1 | `dayBrightness` — percent, 0–100 |
| 1 | `nightBrightness` — percent, 0–100, used during quiet hours |
| 1 | `count` — sessions that follow, 0–16 |
| … | `count` × session |

### Session

| Size | Field |
|---|---|
| 4 | `id` — stable for the session's lifetime |
| 1 | `agent` — 0 Claude Code, 1 Codex |
| 1 | `state` — 0 NEEDS YOU, 1 DONE, 2 WORKING |
| 1 | `subagents` — folded subagent count, 0 if none |
| 4 | `inState` — seconds in the current state |
| 1 + n | `name` — n ≤ 48 bytes |
| 1 + n | `detail` — n ≤ 12 bytes (tool name, `perm`, `?`) |

The Mac truncates strings on a UTF-8 boundary. The clock rejects a frame whose version it doesn't
know or whose lengths overrun.

## Events (clock → Mac)

| Size | Field |
|---|---|
| 1 | `kind` — 0 dismiss, 1 jump |
| 4 | `id` — the session |

The Mac answers a dismiss by leaving that session out of the next snapshot.
