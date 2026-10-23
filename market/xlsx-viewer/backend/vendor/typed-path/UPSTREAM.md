Source: https://github.com/chipsenkbeil/typed-path/tree/v0.12.3
Version: 0.12.3 (MIT OR Apache-2.0; license files retained).
Local changes: the `std::os::wasi` OsStr conversions are limited to wasip1 and the `feature(wasip2)` attribute is removed, because `std::os::wasi` is unstable on wasm32-wasip2 and the crate otherwise needs a nightly compiler there. zip pulls this crate in through calamine; it is patched for the whole workspace in the root Cargo.toml.
