# ![TockLoader](http://www.tockos.org/assets/img/tockloader.svg#a "Tockloader Logo")

This is a final port to Rust for Tock Loader.

Please use the original Python version of [TockLoader](https://www.github.com/tock/tockloader).

## Adding support for a new board

If you want to add support for a new board, you have 3 options:

1. Implement support for the bootloader for your board. For this, please see the
[tock-bootloader](https://github.com/tock/tock-bootloader/tree/master) repo for more details.
   > Note: this approach will limit you to use the bootloader for all operations (using the
   > `--serial` flag).
2. Add support for your board in
[probe-rs](https://github.com/probe-rs/probe-rs?tab=readme-ov-file#adding-targets). This should be a
straight-forward process if a CMSIS packs is available for your board.
3. Implement a custom debug probe for your board. This is the most complex option, but it will give
you the most flexibility:
    - First, add your debug probe to the `Connect` enum in `tockloader-lib/src/connection.rs`.
    - Then, implement each command individually. There is no predefined interface for this, as debug probes
      can be very different from each other. You can take a look at the existing implementations for inspiration, and feel free to contact us if you need help.

## Install Dev Prerequisites

### All distrubitions

For all distributions, you must install [tauri
prerequisites](https://tauri.app/start/prerequisites/) if you want to work on
`tock-ui`, or to have rust-analyzer work out of the box. You can go without
installing these if you configure rust-analyzer to ignore the `tock-ui` project.

### Linux

```bash
sudo apt update
sudo apt install libudev-dev
```

### WSL

```bash
sudo apt update
sudo apt install libudev-dev pkg-config
```

License
-------

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
