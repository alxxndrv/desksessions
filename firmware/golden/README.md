# Golden references

The firmware was ported from C++ (PlatformIO, LovyanGFX, NimBLE) to Rust. These files pin the
port to what the C++ firmware did; commit `ee9f304` is the last one with the C++ code beside the
Rust, if they ever need regenerating from it.

| File | From | Checked by |
|---|---|---|
| `fixtures/wire.txt` | the C++ encoder, decoder and reassembler | `sim/tests/wire.rs`: byte for byte |
| `fixtures/curves.txt` | the C++ easing, moook, spring, breathing, marquee and channel code, sampled every millisecond | `sim/tests/curves.rs`: within 1e-4 |
| `fixtures/trace-*.txt` | the C++ app run through each scenario, its animated state every frame | `sim/tests/golden.rs`: within 1e-3 |
| `frames/*/shot-*.png` | the Rust simulator, `cargo xtask bless` | `sim/tests/golden.rs`: under 0.2 % of pixels off by more than 2 levels |

`scenarios.txt` lists the scenarios: simulator arguments, the same flags the C++ simulator took.

When the C++ stills were the reference, 182 of the 183 stills matched the Rust renderer bit for
bit and the last differed in one pixel. Wire bytes matched exactly; curves to within 3e-5.

The wire vectors, curves and traces are frozen: they describe the protocol and the motion as
designed. Change them only on purpose, by hand. The stills follow the renderer: after a deliberate
visual change, run `cargo xtask bless` and look at the diff before committing it.
