# desksessions — design decisions

Desk display of active Claude Code and Codex sessions on a **Heltec Vision Master T190**
(Moon Phase Clock kit; ESP32-S3R8, SX1262 470–510 MHz unused, 1.9" ST7789 TFT 170×320).

North star: **friendly, responsive, playful UI — animations and transitions for everything.**

Status: fully agreed in grilling session 2026-10-07.

---

## 1. System

```
 ~/.claude/sessions/*.json ──FSEvents──┐
                                       ├─► Mac menu-bar app (Swift) ──BLE (bonded)──► T190 clock
 Codex hooks ─► desksessions-hook ─UDS─┘        ▲   │                                  │
                                                │   └─ cmux focus / activate ChatGPT ◄─┘ button events
                                       global hotkey panel
```

- One Mac, clock on the desk within BLE range. Always USB-powered.
- Transport: **BLE**. Clock = GATT peripheral; Mac = central.
- Mac writes a full **snapshot** frame (compact binary, < 512 B for ~12 rows) on every change.
  Clock sends button events (dismiss, jump) via a notify characteristic.
- Fast connection interval (~30 ms); full frame-rate animations.

## 2. Security

- LE Secure Connections, **passkey bonding**: clock shows 6-digit code, confirm on Mac.
- Clock accepts only the bonded Mac; resolvable private address.
- Re-pair: hold BOOT + GPIO21 for 5 s → wipes bond.

## 3. Data sources

- **Claude Code**: watch `~/.claude/sessions/*.json` (FSEvents). Fields: `pid`, `sessionId`, `cwd`,
  `name`, `status` (`busy` | `waiting` | `idle` | `shell`), `kind`, `entrypoint`, `updatedAt`.
  Drop stale PIDs (`kill -0`). cmux surface via process env (`CMUX_SURFACE_ID`, `CMUX_WORKSPACE_ID`,
  read with `sysctl KERN_PROCARGS2`).
- **Codex**: our own entries in `~/.codex/hooks.json` alongside Xirp/Honcho (never touch theirs;
  timestamped backup before first edit). Hook = compiled Swift binary `desksessions-hook` → one line
  to the app's Unix socket, exits silently if the app isn't running.
  - `SessionStart` → register · `UserPromptSubmit` / `PostToolUse` → WORKING ·
    `PermissionRequest` → NEEDS YOU · `Stop` → DONE
  - Hook forwards cmux env vars for jump-to.
  - **Verify during build**: whether Codex Desktop (ChatGPT.app) fires these hooks. Fallback:
    `~/.codex/state_5.sqlite` `threads.updated_at` → WORKING/DONE only.
- Not used: Xirp's private daemon (`localhost:57032`), rollout JSONL tailing.

### Filter

- Claude: `kind == interactive` (any entrypoint). Exclude headless/SDK `-p` runs, cwd under
  `/var/folders` or `/tmp`. Subagents/teammates fold into parent as `×N`.
- Codex: CLI (`codex-tui`) + Desktop threads. Subagent threads fold into parent. Exclude `codex exec`.
- Settings: cwd allow/deny patterns.

## 4. State model

| State | Meaning | Claude | Codex |
|---|---|---|---|
| 🔴 NEEDS YOU | blocked on permission / question | `waiting` | `PermissionRequest` |
| 🟡 DONE | finished turn, you haven't come back | `busy`→`idle` | `Stop` |
| 🟢 WORKING | running | `busy` | `UserPromptSubmit`, `PostToolUse` |

- Seen idle sessions are hidden.
- Claude's `shell` status means a background command is running while Claude itself is idle
  (observed lasting days), so it counts as idle, not WORKING.
- DONE **never expires by time**. Clears on: dismiss, new prompt, session exit (PID gone).
  Daily sweep auto-dismisses DONE rows older than 24 h.
- Sort: NEEDS YOU → DONE → WORKING; within group, longest-waiting first.

## 5. Clock UI (landscape 320×170)

```
┌──────────────────────────────────────┐
│ ● 2 NEED  ● 1 DONE  ● 3 WORK   14:07 │  header: counts + time
├──────────────────────────────────────┤
│▌✳ servarium-05        perm   0:42    │  red: NEEDS YOU
│▌⬡ Introduce gated SFTP  ?    3:10    │
│▌✓ documents-b9        done   12m     │  yellow: DONE
│▌✳ desksessions-d9    Bash    1:05    │  green: WORKING
│▌⬡ Check prod thumbs   ×3     8:20    │  ×3 = subagents
└──────────────────────────────────────┘
```

- 5 rows × ~30 px, 20 px header. Row: state bar · expressive agent glyph · **session name**
  (Claude `name` / Codex thread title) · detail (current tool, `perm`, `?`, `×N`) · time in state.
- Selected row marquees long names; others ellipsize. Slim scroll indicator on overflow.
- Sessions-only firmware (stock Moon Phase firmware is overwritten, no backup).
  Empty → "All quiet" character screen, dimmed backlight.
- **Expressive glyphs** (~14 px): Claude sparkle / Codex hex. WORKING spins, NEEDS YOU hops,
  DONE check pops. Empty/offline character blinks, yawns, zzz.

### Buttons

| Input | Action |
|---|---|
| BOOT short | cursor up (wraps) |
| GPIO21 short | cursor down (wraps) |
| GPIO21 long ≥ 600 ms | dismiss selected row (DONE hides until state changes) → sent to Mac |
| BOOT long ≥ 600 ms | jump to: Mac focuses the cmux surface (Claude/Codex CLI) or activates ChatGPT.app (Codex Desktop) |
| BOOT + GPIO21 5 s | wipe bond / re-pair |
| 30 s idle | cursor returns to top |

### Alerts & night mode

- New NEEDS YOU: 3 backlight pulses, then red bar breathes until resolved.
- New DONE: 1 pulse. WORKING changes: no alert.
- Bursts within 5 s merge into one alert.
- Night mode 23:00–08:00: backlight ~10 %, no pulses. Time synced from Mac over BLE on connect.

### Offline (Mac asleep / out of range / app quit)

- 0–10 s: silent reconnect, nothing visible.
- \> 10 s: keep last list, rows fade to 50 %, timers freeze, header shows "Mac asleep" + bobbing zzz.
- \> 5 min: staggered collapse to sleepy face, backlight ~15 %.
- Reconnect: wake bounce, fresh snapshot animates in via row diffing.
- Mac reconnects automatically after wake; retries with backoff.

## 6. Motion spec (approved)

Render at 60 fps; Pebble 33 ms frame tables are time-resampled. Pebble for snappy/directional
motion, Apple springs for organic motion.

| Moment | Motion | Source |
|---|---|---|
| Button press | list nudges 5 px toward the pressed button, eases back over 144 ms | Pebble action bar |
| Cursor move | moook curve: in-frames `{0,1,20}` (anticipation, jump), out-frames `{4,2,1,0}` px overshoot settle; ~231 ms | Pebble menu / moook |
| Long-press progress | fill sweeps the row over 600 ms; release pops scale 1.0→1.04→1.0, `.bouncy` | Apple spring |
| List scroll following cursor | 250 ms ease-in-out; ease-out if interrupted | Pebble scroll_layer |
| Wrap-around at list end | 6 px rubber-band stretch (c = 0.55), then moook jump to other end | Apple rubber band + Pebble |
| Row appears | slides in from right, `.snappy`; rows below reflow with `.smooth` | SwiftUI presets |
| Row dismissed / session ends | 1-frame squash, slide out left with moook; gap closes `.smooth` | Pebble + Apple |
| Row changes state / reorders | lifts (+1 brightness step), moves with `.snappy`; colour bar crossfades 250 ms on (0.25,0.1,0.25,1) | Apple |
| NEEDS YOU bar | breathing: 1.8 s period, positively biased sine, 0.4 s rest | Apple sleep-LED patent |
| NEEDS YOU glyph | every 3 s: 1-frame squash, hop, bounce on `{8,4,2,1,0}` px | Pebble peek |
| DONE check | 0 → 1.2 → 1.0 with `.bouncy`, once | Apple |
| WORKING glyph | continuous spin, 1 rev / 1.2 s, linear | — |
| Selected-row marquee | 600 ms pause → 20 ms/px → 750 ms pause → rewind 2 ms/px | Pebble status bar |
| Header counts | odometer digit roll, `.snappy` | — |
| Alert backlight pulses | NEEDS YOU ×3, DONE ×1, breathing waveform | Apple patent |
| Offline | rows fade to 50 % over 400 ms; zzz bob 1.8 s; after 5 min staggered collapse, 33 ms/row, moook | Pebble + Apple |
| Reconnect wake | moook_soft(3) ≈ 330 ms, then rows animate in | Pebble timeline |
| Screen changes | list → all-quiet: port hole 198 ms; pairing: moook slide 231 ms | Pebble compositor |
| Mac app | same `.snappy` / `.smooth` springs natively in SwiftUI | — |

### Interruptible and reversible (rule for every motion)

Nothing snaps and nothing queues. Every animated property is a retargetable channel that continues
from its current presented value and velocity:

- **Springs** retarget in place, keeping velocity (Apple).
- **Tweens** interrupted mid-flight continue from the current value with ease-out (Pebble
  scroll_layer rule).
- **Moook** interrupted mid-flight plays only its settle half from the current value (Pebble
  menu_layer rule).
- Long-press fill and row lift follow the finger and drain back with a spring on early release.
- Row enter/exit and screen transitions reverse from wherever they are if the state flips back.

### Reference values

**Pebble** (`coredevices/PebbleOS` @ `054d9d7`)
- Default duration 250 ms; design frame 33 ms (30 Hz) — `fw/applib/ui/animation.h:65-88`
- Easing tables: quadratic, 33 entries — `fw/applib/ui/animation_timing.c:12-28`
- moook: in `{0,1,20}`, out `{4,2,1,0}`, bounce-back 4, 7 frames ≈ 231 ms;
  `moook_soft(n)` adds n linear mid frames (timeline n=3 ≈ 330 ms, notifications n=6 ≈ 429 ms) —
  `fw/applib/ui/animation_interpolate.c:66-67`
- Launcher→app moook: in `{0,1,2,4,12,24,48}`, out `{12,6,3,2,1,0}`, ≈ 429 ms —
  `fw/services/compositor/default/compositor_launcher_app_transitions.c:25-26`
- Peek bounce out `{8,4,2,1,0}` — `fw/popups/timeline/peek.c:308-345`
- Menu highlight: 7 px in 100 ms ease-out, then 10 px overshoot trimmed over 250 ms ease-out;
  button repeat 100 ms; scrollbar hides after 1000 ms — `fw/applib/ui/menu_layer.c`
- Scroll layer: 32 px step, 250 ms ease-in-out, ease-out when interrupting, paging uses moook —
  `fw/applib/ui/scroll_layer.c`
- Window push/pop: moook slide 231 ms; port hole / round flip 6 frames ≈ 198 ms
- Action bar press: 5 px offset, ease back over 144 ms — `fw/applib/ui/action_bar_layer.c:22-25`
- Marquee: 20 ms/px, 600 ms start pause, 750 ms end pause, rewind 2 ms/px — `status_bar_layer.c:340-344`

**Apple**
- `CATransaction` default 0.25 s. CAMediaTimingFunction: default (0.25,0.1),(0.25,1.0);
  easeIn (0.42,0),(1,1); easeOut (0,0),(0.58,1); easeInEaseOut (0.42,0),(0.58,1)
- SwiftUI springs (duration 0.5 s): `.smooth` bounce 0; `.snappy` bounce 0.15; `.bouncy` bounce 0.30.
  `spring(response:dampingFraction:)` default 0.5 / 0.825. `Animation.default` = response 0.55, damping 1.0
- Spring mapping (WWDC23 "Animate with springs"), mass 1:
  stiffness = (2π / duration)², damping = 4π(1 − bounce) / duration (bounce ≥ 0)
- UIScrollView deceleration: normal 0.998, fast 0.99 (per-ms velocity multiplier)
- Rubber band (reverse-engineered): f(x) = (1 − 1 / (x·c/d + 1)) · d, c = 0.55
- Breathing (patent US6658577B2): ~1.8 s period, biased sine, ~0.4 s quiet period
- Not verified: UITableView row durations, UIScrollView bounce-back, notification banner timing

## 7. Mac app

- SwiftUI menu-bar app, macOS 27, `LSUIElement` (no Dock icon), ad-hoc signed, built from source.
- Launch at login via `SMAppService`. Bluetooth TCC via `NSBluetoothAlwaysUsageDescription`.
- **Raycast / Arc feel**: custom floating `NSPanel`, Liquid Glass + vibrancy, 16 px radius, spring
  scale+fade drop from the menu-bar icon.
- **Global hotkey** (default ⌥⌘S, rebindable) summons the panel centred, Raycast-style; Esc closes.
- Keyboard-first: ↑/↓ select, ↵ jump, ⌘⌫ dismiss, **type to filter**; footer action hints.
- Rows mirror the clock (state colours, glyphs, live timers, same springs).
- Menu-bar icon: connection dot + NEEDS YOU badge.
- Settings window: pairing, Codex hooks install/uninstall, hotkey, night-mode hours, launch at
  login, cwd allow/deny.

## 8. Firmware & tooling

- **Rust** (since 2026-11; ported from C++/PlatformIO/LovyanGFX, see `docs/RUST_PORT.md`).
  `esp-hal` + Embassy on the device, `trouble` as the BLE host.
- Portable crates with no BLE or display dependencies, `no_std` and allocation-free:
  `core` (domain, app, animation, wire format, scenes) and `ui` (canvas, VLW fonts, renderer). The
  canvas reproduces LovyanGFX's rasterisers (anti-aliased wedge lines and rounded rects, arcs, VLW
  text with fractional scale), so the pictures are the ones the C++ firmware drew.
- **Simulator** on the Mac (`minifb` window, ×3 scale, ↑/↓ = BOOT/GPIO21 with hold = long-press,
  or `--headless`): scripted timelines, slow motion, stills and GIF/MP4 recording via ffmpeg.

## 9. Firmware architecture (approved)

**Hexagonal (Ports & Adapters) + one active object + DDD-lite.**
Types where it pays, plain data where it's hot.

```
 BLE task (trouble host)       buttons (polled per frame)    [sim: keyboard / script]
        \______________________ | ______________________/
             Embassy channels (BLE → UI chunks + link events; UI → BLE commands)
 ┌──────────── UI loop: sole owner of all state ────────────┐
 │ Connection phases: Unpaired → Pairing → Online ⇄ Offline │
 │  domain/ SessionBoard aggregate, Session value types     │
 │  scene/  BoardScene, StageScene: retained motion state   │
 │  anim/   Channel: springs/curves, fixed 120 Hz step      │
 │  ui/     Renderer::draw(&DeskApp, Canvas) → frame pump   │
 └──── Ports: MacLink trait · backlight level from tick ────┘
```

- **DDD-lite**: ubiquitous language (Session, Board, Snapshot, Dismiss, Jump); one aggregate
  `SessionBoard` owns invariants (snapshot apply, cursor stability across reorders, optimistic
  dismiss). No repositories / domain services.
- **Ports**: `MacLink` is a trait the BLE link, the demo scenario and test fakes implement;
  `DeskApp::tick` returns the backlight level instead of calling out. Composition roots:
  `device/src/main.rs` and the simulator's `Simulation`.
- **Data-oriented where hot**: animation state lives in `Channel`s stepped once per frame; the
  renderer is a function of the app state; snapshots, commands and link events are enums.
- **Toolchain**: stable Rust for the host crates; the `esp` toolchain (Xtensa) for `device/`,
  which is its own workspace. `unsafe` forbidden in the portable crates, denied in the device crate.
- **Memory**: no heap in `core`/`ui`; `heapless` containers sized by the protocol limits (16
  sessions, 48-byte names). On the device the 108,800 B frame lives in octal PSRAM (internal RAM
  is needed by Bluetooth) and streams to the panel through two internal DMA bounce buffers, five
  64-row bands.
- **Concurrency**: the BLE task only forwards chunks and link events; it never touches domain or
  UI state. Settings and the bond are stored in a `desk` flash partition (`sequential-storage`).
- **Time**: `Instant`/`Millis` passed in from outside → deterministic tests and sim. Fixed-timestep
  accumulator for springs; bézier / Pebble tables are pure functions of t.
- **Tests**: unit tests per module (ported from the C++ GoogleTest suite), plus golden checks
  against the C++ firmware: wire vectors byte for byte, curves within 1e-4, per-frame app state
  for twelve scenarios, and stills (`firmware/golden/`). `cargo xtask check` runs every gate.

## 10. Repo layout

```
desksessions/
├─ docs/DESIGN.md
├─ protocol/                 BLE frame spec + golden frame
├─ mac/                      SwiftUI menu-bar app + desksessions-hook binary
└─ firmware/                 Cargo workspace
   ├─ crates/core/           no_std: domain, app, anim, protocol, scene
   ├─ crates/ui/             no_std: canvas (LovyanGFX-compatible), VLW fonts, glyphs, renderer
   ├─ crates/demo/           the scripted pretend Mac
   ├─ sim/                   simulator binary + golden tests
   ├─ golden/                scenarios, frozen C++ fixtures, golden stills
   ├─ xtask/                 `cargo xtask check | bless`
   ├─ device/                ESP32-S3 firmware (own workspace, `esp` toolchain)
   ├─ fonts/  tools/         Golos Text and the VLW generator; check.sh
   └─ rust-toolchain.toml  rustfmt.toml
```

## 11. Open

- Verify Codex Desktop hook behaviour during build.
