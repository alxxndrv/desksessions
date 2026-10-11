# desksessions

A small desk display that shows your running Claude Code and Codex sessions: which ones are
working, which are done, and which are waiting for you.

It has two halves:

- **The clock**: a Heltec Vision Master T190 (ESP32-S3, 1.9" 170×320 TFT, two buttons) running
  the firmware in `firmware/`.
- **The Mac app**: a menu-bar app in `mac/` that watches your sessions and sends them to the
  clock over Bluetooth LE. It also has its own panel (⌥⌘S) with the same list.

The buttons on the clock move the cursor, dismiss a finished session, or jump to the selected
session on the Mac.

## Layout

```
docs/DESIGN.md   design decisions: states, UI, motion, architecture
protocol/        the BLE frame format, with a golden frame both sides test against
firmware/        C++20 / PlatformIO: device build, SDL2 simulator, unit tests
mac/             Swift package: menu-bar app, Codex hook binary, tests
```

## Firmware

Needs [PlatformIO](https://platformio.org/). The simulator also needs SDL2 from Homebrew
(`brew install sdl2`) and `HOMEBREW_PREFIX` set in the environment.

```
cd firmware
pio test -e native_test          # unit tests
pio run -e sim                   # build the simulator
.pio/build/sim/program           # run it; ↑/↓ are the two buttons, hold for a long press
pio run -e t190 -t upload        # flash the clock
tools/check.sh                   # tests + simulator build
```

`t190_demo` is the device build with a scripted pretend Mac instead of Bluetooth, and
`t190_stats` prints frame timing over USB serial.

Flashing replaces the stock firmware on the board.

Simulator options: `--no-demo`, `--no-backlight`, `--slowmo <factor>`, `--time HH:MM`,
`--seconds <n>`, `--gif <file>`, `--shots <dir> --at <ms,ms,...>`, and
`--script <ms:cue,...>` with cues `up`, `down`, `longup`, `longdown`, `add`, `remove`, `needs`,
`pause`, `link`, `sleep`, `pair`, `empty`.

## Mac app

Needs macOS 26 and Xcode with Swift 6.2 or newer.

```
cd mac
swift test
scripts/build-app.sh             # builds build/DeskSessions.app, ad-hoc signed
scripts/build-app.sh --install   # also copies it to ~/Applications and launches it
```

Pair the clock from the app's settings: the clock shows a six-digit code, type it on the Mac.
To pair again, hold both buttons on the clock for 5 seconds.

Claude Code sessions are picked up from `~/.claude/sessions`. Codex needs hooks, which the
settings window installs into `~/.codex/hooks.json` (and removes again) without touching other
entries.

## License

MIT, see `LICENSE`. The UI font is [Golos Text](https://fonts.google.com/specimen/Golos+Text) under the SIL
Open Font License; see `firmware/fonts/OFL.txt`.
