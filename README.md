# A CHIP-8 Interpreter

A CHIP-8 interpreter written in Rust
with customisable CPU and timer frequencies, optional quirks and adjustable sound pitch.

This interpreter implements the Cosmac VIP variant of CHIP-8,
and does _not_ support the SUPER-CHIP and XO-CHIP variants.

## Building

First, clone the repository:

```sh
git clone https://github.com/dixslyf/chip8.git
cd chip8
```

To build the interpreter, run:

```sh
cargo build --release
```

The compiled binary will be available at `target/release/chip8` (assuming Cargo's default target directory).

## Usage

For a list of available options, run:

```sh
chip8 --help
```

### Quirks

Different CHIP-8 interpreters implement a few instructions differently.
These small variations are known as _quirks_, and certain ROMs rely on them to behave correctly.

This interpreter currently supports toggling the following quirks:

- `--quirk-draw-wrap[=BOOL]` (default: `true`): When enabled, the `DXYN` (draw sprite) instruction wraps sprites that overflow the screen edges around to the opposite side.
  When disabled, sprites are clipped at the display boundary.

- `--quirk-vf-reset[=BOOL]` (default: `false`): When enabled, the 8XY1, 8XY2 and 8XY3 instructions (OR, AND and XOR) will reset the flag register (VF) to 0.

## Testing

Tests can be run with:

```sh
cargo test
```

The interpreter has also been tested against [Timendus's CHIP-8 test suite](https://github.com/Timendus/chip8-test-suite).
When running those test ROMs,
make sure to enable the VF reset quirk through the `--quirk-vf-reset` flag.
