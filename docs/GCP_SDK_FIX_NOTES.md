# GCP SDK Integration - Fix Notes

## Current Status

The GCP Secret Manager SDK integration is **partially complete**:
- ✅ Client structure is correct (`SecretManagerService`)
- ✅ API methods are implemented (`get_secret_value`, `delete_secret`)
- ✅ Request/response handling is correct
- ❌ **Client initialization is blocked** - need to verify correct API

## Issue

The `SecretManagerService` client initialization method needs to be verified. The SDK API documentation needs to be checked to determine the correct way to create the client.

## What's Working

The following methods already use the client correctly:
- `get_secret_value()` - Uses `self.client.access_secret_version()`
- `delete_secret()` - Uses `self.client.delete_secret()`

This confirms that:
1. The client type (`SecretManagerService`) is correct
2. The API methods are correct
3. Only initialization needs to be fixed

## What Needs Fixing

The `new()` method in `src/provider/gcp/mod.rs` needs to correctly initialize the `SecretManagerService` client.

## Possible Solutions

Based on common Google Cloud Rust SDK patterns, try one of these:

### Option 1: Direct initialization
```rust
let client = SecretManagerService::new().await?;
```

### Option 2: With credentials
```rust
use google_cloud_auth::credentials::Credentials;
let credentials = Credentials::find_default().await?;
let client = SecretManagerService::with_credentials(credentials).await?;
```

### Option 3: Builder pattern
```rust
let client = SecretManagerService::builder()
    .build()
    .await?;
```

### Option 4: Default implementation
```rust
let client = SecretManagerService::default().await?;
```

## Next Steps

1. **Check SDK Documentation**: Review `google-cloud-secretmanager-v1` documentation
   - URL: https://docs.rs/google-cloud-secretmanager-v1
   - Or check local docs: `cargo doc --package google-cloud-secretmanager-v1 --open`

2. **Check SDK Source**: Review the SDK source code to find the correct initialization
   - GitHub: https://github.com/googleapis/google-cloud-rust
   - Or check local git checkout

3. **Test**: Once initialized correctly, test with actual GCP credentials

## Testing

After fixing initialization:

1. **Unit Test**: Verify client creation doesn't error
2. **Integration Test**: Test with real GCP credentials (Workload Identity or JSON)
3. **End-to-End Test**: Test full secret creation/update workflow

## References

- SDK Documentation: https://docs.rs/google-cloud-secretmanager-v1
- GitHub Repository: https://github.com/googleapis/google-cloud-rust
- Google Cloud Auth: https://docs.rs/google-cloud-auth

