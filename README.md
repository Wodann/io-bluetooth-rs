# IO Bluetooth

**IO Bluetooth is a Rust library that provides cross-platform support for Bluetooth communication.**

[![Build Status](https://github.com/wodann/io-bluetooth-rs/workflows/CI/badge.svg?branch=master)](https://github.com/wodann/io-bluetooth-rs/actions)
[![Documentation][docs-badge]][docs-url]
[![MIT OR Apache license][license-badge]][license-url]
![Lines of Code][loc-url]

[docs-badge]: https://img.shields.io/badge/docs-website-blue.svg
[docs-url]: https://docs.rs/io_bluetooth
[license-badge]: https://img.shields.io/crates/l/io_bluetooth
[license-url]: README.md
[loc-url]: https://tokei.rs/b1/github/wodann/io-bluetooth-rs?category=code

## Usage

Add the following to your `cargo.toml`:

```toml
[dependencies]
io_bluetooth = "0.2"
```

Examples of how to use the IO Bluetooth API are provided [here](examples/).

## No-std support

This crate currently requires the Rust standard library.

## Platform support

IO Bluetooth is guaranteed to build for the following platforms:

 * x86_64-pc-windows-msvc
 * x86_64-unknown-linux-gnu

## License

IO Bluetooth is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
 
 at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in IO Bluetooth by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

To contribute to IO Bluetooth, please see [CONTRIBUTING](CONTRIBUTING.md).
