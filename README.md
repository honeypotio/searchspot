Honeysearch
===========
This service will be the endpoint responsible for Honeypot's ElasticSearch data.

Currently a really WIP. Just [biliv](https://just-believe.in).

Setup
-----
Install the latest stable release of Rust using the [official installer](https://www.rust-lang.org/downloads.html) or your package manager.

Then clone locally this repository and run the executable with

```sh
$ cargo run [examples/config.toml]
````

You can produce an optimized executable just appending `--release`, but the compile time will be longer.

Run `cargo test` to run the tests and `cargo doc` to produce the documentation.
