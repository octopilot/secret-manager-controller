# Azure SDK Fork: rustls Support

This document describes the fork of `azure-sdk-for-rust` to add rustls support for musl compatibility.

## Branch

- **Branch**: `fix/rustls-support`
- **Repository**: `https://github.com/microscaler/azure-sdk-for-rust` (to be created)
- **Purpose**: Enable musl cross-compilation without OpenSSL dependencies

## Changes

### Workspace Dependencies

Modified `Cargo.toml` workspace dependencies:

```toml
reqwest = { version = "0.12.23", features = [
  "stream",
  "rustls-tls",
  "rustls-tls-webpki-roots",
], default-features = false }
```

### typespec_client_core

Added `reqwest_rustls` feature to `sdk/core/typespec_client_core/Cargo.toml`:

```toml
reqwest_rustls = ["reqwest", "reqwest/rustls-tls", "reqwest/rustls-tls-webpki-roots"]
```

### azure_core

Added `reqwest_rustls` feature to `sdk/core/azure_core/Cargo.toml`:

```toml
reqwest_rustls = ["reqwest", "typespec_client_core/reqwest_rustls"]
```

## Usage

Once the fork repository is created, use it in your `Cargo.toml`:

```toml
[dependencies]
azure_core = { git = "https://github.com/microscaler/azure-sdk-for-rust", branch = "fix/rustls-support", default-features = false, features = ["reqwest", "reqwest_deflate", "reqwest_gzip", "reqwest_rustls"] }
azure_identity = { git = "https://github.com/microscaler/azure-sdk-for-rust", branch = "fix/rustls-support", package = "azure_identity" }
azure_security_keyvault_secrets = { git = "https://github.com/microscaler/azure-sdk-for-rust", branch = "fix/rustls-support", package = "azure_security_keyvault_secrets" }
```

## Benefits

1. **No OpenSSL Required**: Pure Rust TLS implementation via rustls
2. **Musl Compatible**: Works seamlessly with `x86_64-unknown-linux-musl` targets
3. **Smaller Binaries**: No C dependencies means smaller, statically linked binaries
4. **Cross-Compilation**: No need for OpenSSL cross-compilation toolchains

## Current Workaround

Until the fork is available, the controller uses a direct `reqwest` dependency with rustls features to work around Azure SDK's default native-tls usage. This works due to Cargo's feature unification, but the fork provides a cleaner solution.

## Future

This fork may become unnecessary if:
1. The upstream `azure-sdk-for-rust` adds `reqwest_rustls` feature
2. All dependencies migrate to rustls by default

Until then, this fork provides a clean solution for musl-based deployments.

