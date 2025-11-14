# Fork Plan: Fix OpenSSL/Rustls Issue in Google Cloud SDK

## Problem Statement

The `google-cloud-secretmanager-v1` crate depends on `google-cloud-gax-internal`, which uses `reqwest` with default features (OpenSSL). This causes cross-compilation issues for musl targets because OpenSSL requires complex cross-compilation setup.

**Solution**: Fork the Google Cloud Rust SDK and modify `google-cloud-gax-internal` to use `rustls-tls` instead of `native-tls` (OpenSSL).

## Repository to Fork

**Repository**: https://github.com/googleapis/google-cloud-rust

**Crate to Modify**: `google-cloud-gax-internal`

**Why this crate?**
- It's the lowest-level crate that depends on `reqwest`
- All other Google Cloud SDK crates depend on it
- Changing it here will propagate to all dependent crates

## Step-by-Step Plan

### 1. Fork the Repository

1. Go to https://github.com/googleapis/google-cloud-rust
2. Click "Fork" to create your fork
3. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/google-cloud-rust.git
   cd google-cloud-rust
   ```

### 2. Create a Feature Branch

```bash
git checkout -b feature/rustls-support
```

### 3. Locate and Modify `google-cloud-gax-internal/Cargo.toml`

**File to modify**: `google-cloud-gax-internal/Cargo.toml`

**Current dependency** (likely):
```toml
reqwest = "0.12"
# or
reqwest = { version = "0.12", features = [...] }
```

**Change to**:
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "rustls-tls-webpki-roots"] }
```

**Why these features?**
- `default-features = false` - Disables OpenSSL/native-tls
- `json` - Required for JSON API communication
- `rustls-tls` - Enables rustls TLS backend
- `rustls-tls-webpki-roots` - Uses webpki for certificate validation (no system certs needed)

### 4. Test the Changes

```bash
cd google-cloud-gax-internal
cargo check
cargo test
```

### 5. Update All Dependent Crates

Since `google-cloud-gax-internal` is a workspace member, you may need to:

1. Check if other crates in the workspace also depend on `reqwest` directly
2. Update them similarly if needed
3. Run workspace tests:
   ```bash
   cargo test --workspace
   ```

### 6. Use the Forked Version in Our Project

**Update `hack/controllers/secret-manager-controller/Cargo.toml`**:

```toml
[dependencies]
# Use forked google-cloud-rust with rustls support
google-cloud-gax-internal = { git = "https://github.com/YOUR_USERNAME/google-cloud-rust", branch = "feature/rustls-support", package = "google-cloud-gax-internal" }
google-cloud-secretmanager-v1 = "1.1.1"
google-cloud-auth = "1.1.1"

# Remove vendored OpenSSL - no longer needed!
# openssl = { version = "0.10", features = ["vendored"] }  # REMOVE THIS
```

**Note**: Cargo will use the forked `gax-internal` for all Google Cloud SDK crates automatically due to dependency resolution.

### 7. Test Our Controller

```bash
cd hack/controllers/secret-manager-controller
cargo clean
./scripts/host-aware-build.sh --bins
cargo test
```

### 8. Create Pull Request

1. Commit your changes:
   ```bash
   git add google-cloud-gax-internal/Cargo.toml
   git commit -m "feat: Add rustls support to gax-internal for musl compatibility

   - Replace native-tls (OpenSSL) with rustls-tls
   - Enables cross-compilation to musl targets
   - Resolves cross-compilation issues for musl builds"
   ```

2. Push to your fork:
   ```bash
   git push origin feature/rustls-support
   ```

3. Create PR on upstream repository:
   - Go to https://github.com/googleapis/google-cloud-rust
   - Click "New Pull Request"
   - Select your fork and branch
   - Title: "Add rustls support to gax-internal for musl compatibility"
   - Description: Explain the cross-compilation issue and how rustls solves it

### 9. Alternative: Use Git Dependency with Specific Commit

If you want to pin to a specific commit while waiting for PR:

```toml
google-cloud-gax-internal = { 
    git = "https://github.com/YOUR_USERNAME/google-cloud-rust", 
    rev = "COMMIT_HASH",
    package = "google-cloud-gax-internal" 
}
```

## Files That Will Need Updates

### In Forked Repository:
- `google-cloud-gax-internal/Cargo.toml` - Main change
- Possibly other workspace crates if they also depend on reqwest

### In Our Project:
- `hack/controllers/secret-manager-controller/Cargo.toml` - Add git dependency, remove vendored OpenSSL
- `hack/controllers/secret-manager-controller/README.md` - Update documentation about rustls support
- `hack/controllers/secret-manager-controller/docs/FORK_PLAN.md` - This file

## Benefits of This Approach

1. ✅ **Eliminates OpenSSL dependency** - No more cross-compilation issues
2. ✅ **Faster builds** - rustls is pure Rust, no C dependencies
3. ✅ **Smaller binaries** - rustls is more lightweight
4. ✅ **Better musl compatibility** - rustls works perfectly with musl
5. ✅ **Contributes back** - PR helps the community

## Potential Issues

1. **Certificate validation**: rustls uses webpki instead of system certs
   - Solution: `rustls-tls-webpki-roots` includes Mozilla's CA bundle
   
2. **Feature conflicts**: If other crates explicitly enable native-tls
   - Solution: Cargo's feature unification should handle this

3. **Upstream changes**: If upstream changes reqwest version
   - Solution: Keep fork updated, or use git dependency with rev pinning

## Testing Checklist

- [ ] Fork repository created
- [ ] Feature branch created
- [ ] `gax-internal/Cargo.toml` modified
- [ ] Workspace tests pass
- [ ] Our controller builds with forked version
- [ ] Our controller tests pass
- [ ] Cross-compilation to musl works
- [ ] PR created and submitted

## Next Steps After Fork is Ready

1. Update `Cargo.toml` to use git dependency
2. Remove `openssl = { version = "0.10", features = ["vendored"] }`
3. Test build and functionality
4. Update documentation
5. Monitor PR status

