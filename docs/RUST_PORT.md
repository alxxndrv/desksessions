# Rust port — decisions

Agreed 2026-11-06. The firmware moves from C++ (PlatformIO, Arduino, LovyanGFX, NimBLE) to Rust.

## Scope

- Firmware only. The Swift Mac app is untouched and is the protocol reference.

## What "byte for byte" means

| Layer | Check |
|---|---|
| BLE wire format | exact, against test vectors captured from the C++ encoder/decoder |
| Animation curves | sampled every millisecond, within `1e-4` of the C++ values |
| Pixels | per frame, < 0.2 % of pixels differ by more than 2 levels per channel from the C++ sim |

- Idiomatic Rust: enums with data, traits for ports, `heapless`, Embassy tasks. Same algorithms,
  constants, curves and the motion spec in `DESIGN.md`.
- The C++ reference build is not modified. No `mul_add`; `sin`/`cos` come from `std` on the host
  and `libm` on the device.
- Settings keep their meaning; one re-pair is expected after the switch.

## Stack

- `core`, `ui`, `demo`: `no_std`, no `alloc`, `heapless` capacities from the protocol constants.
- Device: `esp-hal` + Embassy + `trouble`. Fallback if passkey bonding with macOS fails:
  `esp-idf-svc` + `esp32-nimble` (the portable crates are unaffected either way).
- Simulator: `minifb`, `clap` (same flags as the C++ sim), `png`, `ffmpeg` for GIF/MP4, headless mode.
- Fonts: `tools/make_vlw.py` writes `.vlw` files, embedded with `include_bytes!`.

## Layout

```
firmware-rs/
  crates/core   crates/ui   crates/demo
  sim/          golden/     xtask/
  device/       (own workspace, `esp` toolchain, xtensa-esp32s3-none-elf)
```

Lives next to `firmware/` until parity, then replaces it.

## Milestones

1. Workspace + reference harness (wire vectors, curve dumps, C++ sim goldens)
2. `core` with all unit tests ported
3. `ui`
4. `sim` + `demo`, pixel goldens pass
5. Device crate; BLE spike on hardware
6. Switch-over: delete C++, rename `firmware-rs/` → `firmware/`, update docs

After the switch, wire vectors and curve dumps stay as frozen fixtures; pixel goldens become Rust
snapshots, re-blessed with `cargo xtask bless`.

## Gates

`cargo xtask check`: fmt, clippy `-D warnings`, tests, device build when the `esp` toolchain is
installed. `tools/check.sh` wraps it.
