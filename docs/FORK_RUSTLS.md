# Fork: rustls Support for musl Compatibility

This fork of `google-cloud-rust` modifies the workspace `reqwest` dependency to use `rustls` instead of `native-tls` (OpenSSL).

## Branch

- **Branch**: `fix/openssl-tls-issue`
- **Purpose**: Enable musl cross-compilation without OpenSSL dependencies

## Changes

### Workspace Dependencies

Modified `Cargo.toml` workspace dependencies:

```toml
reqwest = { default-features = false, version = "0.12.24", features = ["json", "rustls-tls", "rustls-tls-webpki-roots"] }
```

This replaces the default `native-tls` backend with `rustls-tls`, which:
- Eliminates OpenSSL dependency and cross-compilation complexity
- Enables musl target builds without vendored OpenSSL or `cargo-zigbuild`
- Reduces binary size (rustls is pure Rust, no C dependencies)
- Improves musl compatibility and static linking support

## Usage

Use this fork in your `Cargo.toml`:

```toml
[dependencies]
google-cloud-gax-internal = { git = "https://github.com/microscaler/google-cloud-rust", branch = "fix/openssl-tls-issue", package = "google-cloud-gax-internal" }
google-cloud-secretmanager-v1 = { git = "https://github.com/microscaler/google-cloud-rust", branch = "fix/openssl-tls-issue", package = "google-cloud-secretmanager-v1" }
google-cloud-auth = { git = "https://github.com/microscaler/google-cloud-rust", branch = "fix/openssl-tls-issue", package = "google-cloud-auth" }
```

## Benefits

1. **No OpenSSL Required**: Pure Rust TLS implementation
2. **Musl Compatible**: Works seamlessly with `x86_64-unknown-linux-musl` targets
3. **Smaller Binaries**: No C dependencies means smaller, statically linked binaries
4. **Cross-Compilation**: No need for OpenSSL cross-compilation toolchains

## API Compatibility

The change maintains API compatibility as `reqwest`'s rustls backend provides equivalent functionality to `native-tls` for HTTP client operations.

## Upstream

This fork is based on the official `google-cloud-rust` repository. The change is minimal and focused solely on the TLS backend selection.

## Future

This fork may become unnecessary if:
1. The upstream `google-cloud-rust` adds rustls support as an option
2. All dependencies migrate to rustls by default

Until then, this fork provides a clean solution for musl-based deployments.

