# Rust port — decisions

Agreed 2026-11-06. The firmware moves from C++ (PlatformIO, Arduino, LovyanGFX, NimBLE) to Rust.

## Status

Milestones 1–4 and 6 are done; 5 is built but not yet run on hardware.

- Wire format: every vector from the C++ encoder, decoder and reassembler matches byte for byte,
  and the Mac's golden frame decodes.
- Curves: 32,103 samples agree with C++ to within 3e-5.
- Scenes: twelve scripted scenarios agree with the C++ app's animated state on every frame.
- Pixels: 182 of 183 stills from the C++ simulator matched bit for bit; the last was one pixel off.
  The Rust stills are now the baseline (`cargo xtask bless`).
- Device: builds for the ESP32-S3 (`firmware/device`, 879 KB image) with `esp-hal` 1.2,
  `esp-radio` 1.0.0-beta.1, `esp-rtos` 0.4 and `trouble-host` 0.7 (0.8 needs a newer `bt-hci` than
  `esp-radio` ships). Not yet flashed: pairing with the Mac, re-encryption after a reboot, the
  scanline read and the frame rate still need checking on the clock.

Deviations from the C++ device build, all forced by the platform:
- The frame buffer is in PSRAM and goes out in five 64-row bands through internal DMA buffers
  (one SPI DMA transfer is at most 32 KB, and two 108 KB frames don't fit next to Bluetooth).
- Settings and the bond live in a `desk` flash partition (`device/partitions.csv`), not NimBLE's
  and Arduino's NVS keys, so the clock needs pairing once after the switch.
- The Bluetooth address is the public one, as NimBLE used; the name is still `desk-XXXX`.

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
firmware/
  crates/core   crates/ui   crates/demo
  sim/          golden/     xtask/
  device/       (own workspace, `esp` toolchain, xtensa-esp32s3-none-elf)
```

Lived in `firmware-rs/` next to the C++ `firmware/` until parity, then replaced it.

## Milestones

1. Workspace + reference harness (wire vectors, curve dumps, C++ sim goldens)
2. `core` with all unit tests ported
3. `ui`
4. `sim` + `demo`, pixel goldens pass
5. Device crate; BLE spike on hardware
6. Switch-over: delete C++, rename `firmware-rs/` → `firmware/`, update docs

After the switch, wire vectors, curve dumps and per-frame traces stay as frozen fixtures; pixel
goldens become Rust snapshots, re-blessed with `cargo xtask bless`. See `firmware/golden/README.md`.

## Gates

`cargo xtask check`: fmt, clippy `-D warnings`, tests, device build when the `esp` toolchain is
installed. `firmware/tools/check.sh` wraps it.
