# Code Style Guidelines

Comprehensive guide to code style, formatting, and conventions for the Secret Manager Controller.

## Overview

This project follows Rust community standards with some project-specific conventions. Code style is enforced via `cargo fmt` and `cargo clippy` in the pre-commit hook and CI pipeline.

## Code Formatting

### Rustfmt

All code must be formatted with `cargo fmt`:

```bash
# Format all code
cargo fmt --all

# Check formatting (used in CI)
cargo fmt --all -- --check
```

**Rules:**
- Formatting is enforced in pre-commit hooks
- CI will fail if code is not formatted
- Use default `rustfmt` settings (no custom configuration file)

**Pre-commit Hook:**
The pre-commit hook automatically runs `cargo fmt` and will auto-format code if needed.

---

## Naming Conventions

### Modules

- **Module names**: Use `snake_case`
- **Module files**: Match module name (e.g., `mod.rs` for directory modules)

**Examples:**
```rust
// Good
mod parser;
mod reconciler;
mod provider;

// Bad
mod Parser;
mod Reconciler;
mod Provider;
```

### Types

- **Structs**: Use `PascalCase`
- **Enums**: Use `PascalCase`
- **Enum variants**: Use `PascalCase`
- **Type aliases**: Use `PascalCase`

**Examples:**
```rust
// Good
pub struct SecretManagerConfig;
pub enum ProviderConfig {
    Gcp(GcpConfig),
    Aws(AwsConfig),
    Azure(AzureConfig),
}

// Bad
pub struct secret_manager_config;
pub enum provider_config {
    gcp,
    aws,
    azure,
}
```

### Functions and Variables

- **Functions**: Use `snake_case`
- **Variables**: Use `snake_case`
- **Constants**: Use `SCREAMING_SNAKE_CASE`
- **Static variables**: Use `SCREAMING_SNAKE_CASE`

**Examples:**
```rust
// Good
fn reconcile(config: Arc<SecretManagerConfig>) -> Result<Action> {
    let secret_name = "my-secret";
    const MAX_RETRIES: u32 = 3;
}

// Bad
fn Reconcile(config: Arc<SecretManagerConfig>) -> Result<Action> {
    let SecretName = "my-secret";
    const maxRetries: u32 = 3;
}
```

### Traits

- **Trait names**: Use `PascalCase`, often ending with a noun or `Trait` suffix

**Examples:**
```rust
// Good
pub trait SecretManagerProvider {
    async fn create_secret(&self, name: &str, value: &str) -> Result<()>;
}

pub trait ConfigStoreProvider {
    async fn get_config(&self, key: &str) -> Result<Option<String>>;
}
```

### Lifetimes

- **Lifetime parameters**: Use single lowercase letters, prefer `'a`, `'b`, etc.
- **Common lifetimes**: Use descriptive names when appropriate (`'static`, `'async`)

**Examples:**
```rust
// Good
fn process<'a>(data: &'a str) -> &'a str {
    data
}

// Bad
fn process<'data>(data: &'data str) -> &'data str {
    data
}
```

---

## Module Organization

### Directory Structure

Organize modules hierarchically by functionality:

```
crates/controller/src/
├── main.rs              # Entry point
├── lib.rs               # Library root, re-exports
├── config/              # Configuration management
├── controller/          # Controller logic
│   ├── reconciler/     # Reconciliation logic
│   ├── parser/          # File parsing
│   └── kustomize/       # Kustomize integration
├── crd/                 # CRD types
├── observability/       # Metrics and tracing
├── provider/            # Cloud provider implementations
│   ├── gcp/
│   ├── aws/
│   └── azure/
└── runtime/             # Runtime initialization
```

### Module Files

- **Single file modules**: Use `mod_name.rs`
- **Multi-file modules**: Use `mod_name/mod.rs` with submodules in `mod_name/submodule.rs`

**Examples:**
```rust
// Single file module
// src/parser.rs
pub mod parser;

// Multi-file module
// src/reconciler/mod.rs
pub mod reconcile;
pub mod status;
pub mod validation;
```

### Module Documentation

Every module should have a module-level doc comment:

```rust
//! # Module Name
//!
//! Brief description of the module's purpose.
//!
//! ## Overview
//!
//! Detailed explanation of what this module does.
//!
//! ## Usage
//!
//! ```rust
//! use crate::module_name::*;
//! ```
```

**Example:**
```rust
//! # Reconciler
//!
//! Core reconciliation logic for `SecretManagerConfig` resources.
//!
//! The reconciler:
//! - Watches `SecretManagerConfig` resources across all namespaces
//! - Fetches `GitRepository` or `Application` artifacts
//! - Processes application secret files or kustomize builds
//! - Syncs secrets to cloud providers (GCP, AWS, Azure)
//! - Updates resource status with reconciliation results
```

---

## Documentation Standards

### Doc Comments

Use `///` for public items and `//!` for module-level documentation:

```rust
/// Brief description.
///
/// Detailed explanation with multiple paragraphs if needed.
///
/// ## Examples
///
/// ```rust
/// let result = function();
/// ```
pub fn function() -> Result<()> {
    // Implementation
}
```

### Documentation Requirements

**Must document:**
- All public functions
- All public types (structs, enums, traits)
- All public modules
- Complex algorithms or logic
- Error conditions
- Safety requirements (for `unsafe` code)

**Should document:**
- Private functions with complex logic
- Non-obvious implementation details
- Performance considerations
- Thread-safety guarantees

**Examples:**
```rust
/// Create or update a secret in the cloud provider.
///
/// # Arguments
///
/// * `name` - The name of the secret
/// * `value` - The secret value
///
/// # Returns
///
/// Returns `Ok(true)` if the secret was created, `Ok(false)` if it was updated.
///
/// # Errors
///
/// Returns an error if:
/// - The secret name is invalid
/// - The provider API call fails
/// - Authentication fails
pub async fn create_or_update_secret(
    &self,
    name: &str,
    value: &str,
) -> Result<bool> {
    // Implementation
}
```

### Code Examples

Include examples in documentation when helpful:

```rust
/// Parse secrets from a file.
///
/// # Examples
///
/// ```rust
/// use crate::parser::parse_secrets;
///
/// let secrets = parse_secrets("path/to/file.env")?;
/// ```
pub fn parse_secrets(path: &str) -> Result<HashMap<String, String>> {
    // Implementation
}
```

---

## Code Organization

### Imports

Organize imports in the following order:

1. **Standard library**
2. **External crates**
3. **Internal modules**
4. **Crate root**

**Example:**
```rust
// Standard library
use std::collections::HashMap;
use std::sync::Arc;

// External crates
use anyhow::{Context, Result};
use kube::Client;
use tracing::{error, info};

// Internal modules
use crate::crd::SecretManagerConfig;
use crate::controller::reconciler::Reconciler;
```

### Function Organization

Organize functions within a module:

1. **Public functions** (documented)
2. **Private helper functions**
3. **Tests** (in `#[cfg(test)]` module)

**Example:**
```rust
// Public API
pub async fn reconcile(...) -> Result<Action> {
    // Implementation
}

// Private helpers
async fn validate_config(...) -> Result<()> {
    // Implementation
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_reconcile() {
        // Test implementation
    }
}
```

### Error Handling

Follow the [Error Handling Guidelines](./error-handling.md):

- Use `anyhow::Result` for internal functions
- Use `thiserror::Error` for domain errors
- Add context to errors
- Classify errors as transient or permanent

---

## Linting

### Clippy

The project uses `cargo clippy` with workspace-level configuration:

```bash
# Run clippy
cargo clippy --all-targets

# Fix clippy suggestions
cargo clippy --all-targets --fix
```

### Clippy Configuration

The workspace `Cargo.toml` defines clippy lints:

```toml
[workspace.lints.clippy]
cargo = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
suspicious = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "warn"
```

**Key Rules:**
- `unwrap_used = "deny"` - Never use `unwrap()` (use proper error handling)
- `expect_used = "warn"` - Prefer proper error handling over `expect()`
- All other lints are warnings (not errors)

### Common Clippy Fixes

**Avoid `unwrap()`:**
```rust
// Bad
let value = option.unwrap();

// Good
let value = option.context("Option was None")?;
```

**Use `?` operator:**
```rust
// Bad
match result {
    Ok(v) => v,
    Err(e) => return Err(e),
}

// Good
let value = result?;
```

**Prefer `if let`:**
```rust
// Bad
match option {
    Some(v) => process(v),
    None => {},
}

// Good
if let Some(v) = option {
    process(v);
}
```

---

## Type Safety

### Avoid `unwrap()` and `expect()`

**Never use `unwrap()`:**
```rust
// Bad
let value = option.unwrap();

// Good
let value = option.context("Option was None")?;
```

**Minimize `expect()`:**
```rust
// Bad
let value = option.expect("This should never be None");

// Good
let value = option.ok_or_else(|| anyhow::anyhow!("Option was None"))?;
```

### Use Strong Types

Prefer strong types over primitives:

```rust
// Good
pub struct SecretName(String);
pub struct Namespace(String);

// Bad
pub fn create_secret(name: String, namespace: String) -> Result<()>;
```

### Use `Option` and `Result` Appropriately

- **`Option<T>`**: Use when a value may or may not exist
- **`Result<T, E>`**: Use when an operation may fail

```rust
// Good
fn find_secret(name: &str) -> Option<String>;
fn create_secret(name: &str, value: &str) -> Result<()>;

// Bad
fn find_secret(name: &str) -> String; // What if not found?
fn create_secret(name: &str, value: &str) -> (); // What if it fails?
```

---

## Async/Await Patterns

### Async Functions

Use `async fn` for asynchronous operations:

```rust
// Good
pub async fn reconcile(config: Arc<SecretManagerConfig>) -> Result<Action> {
    // Async operations
}

// Bad
pub fn reconcile(config: Arc<SecretManagerConfig>) -> impl Future<Output = Result<Action>> {
    // Manual Future implementation
}
```

### Error Handling in Async

Use `?` operator with async functions:

```rust
// Good
pub async fn process() -> Result<()> {
    let value = fetch_data().await?;
    process_value(value).await?;
    Ok(())
}

// Bad
pub async fn process() -> Result<()> {
    match fetch_data().await {
        Ok(value) => {
            match process_value(value).await {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}
```

### Spawning Tasks

Use `tokio::spawn` for concurrent operations:

```rust
// Good
let handle = tokio::spawn(async {
    process_data().await
});

let result = handle.await??;
```

---

## kube-rs Usage

The project uses `kube-rs` for Kubernetes API interactions. Follow these patterns for consistency.

### Client Creation

Create a Kubernetes client using `Client::try_default()`:

```rust
use kube::Client;

// Good: Create client with default configuration
let client = Client::try_default()
    .await
    .context("Failed to create Kubernetes client")?;
```

**Note:** The client must be created before any async operations that use rustls.

### API Creation

Create API instances for resources:

```rust
use kube::api::Api;
use crate::crd::SecretManagerConfig;

// Cluster-wide resources (watch all namespaces)
let configs: Api<SecretManagerConfig> = Api::all(client.clone());

// Namespaced resources
let secrets: Api<Secret> = Api::namespaced(client.clone(), "default");

// Dynamic objects (for resources without types)
use kube::core::{DynamicObject, GroupVersionKind, ApiResource};

let gvk = GroupVersionKind {
    group: "notification.toolkit.fluxcd.io".to_string(),
    version: "v1beta2".to_string(),
    kind: "Alert".to_string(),
};
let ar = ApiResource::from_gvk(&gvk);
let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
```

### CRD Definition

Use `#[derive(kube::CustomResource)]` for CRD types:

```rust
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(
    kind = "SecretManagerConfig",
    group = "secret-management.microscaler.io",
    version = "v1beta1",
    namespaced,
    status = "SecretManagerConfigStatus",
    shortname = "smc"
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    // ...
}
```

### Controller Pattern

Use `kube_runtime::Controller` for watching resources:

```rust
use kube_runtime::{controller::Action, watcher, Controller};

let controller = Controller::new(
    configs.clone(),
    watcher::Config::default().any_semantic()
)
.shutdown_on_signal()
.run(
    |obj, ctx| reconcile(obj, ctx),
    |obj, error, ctx| handle_error(obj, error, ctx),
    reconciler.clone(),
);
```

**Patterns:**
- Use `watcher::Config::default().any_semantic()` to watch all semantic changes
- Use `shutdown_on_signal()` for graceful shutdown
- Provide reconcile and error handler functions

### Watching Resources

Use `watcher()` for watching resources:

```rust
use kube_runtime::watcher;
use futures::StreamExt;
use pin_utils::pin_mut;

let stream = watcher(api, watcher::Config::default());
pin_mut!(stream);

while let Some(event_result) = stream.next().await {
    match event_result {
        Ok(event) => {
            match event {
                watcher::Event::Apply(obj) => {
                    // Handle create/update
                }
                watcher::Event::Delete(obj) => {
                    // Handle delete
                }
                watcher::Event::Init
                | watcher::Event::InitApply(_)
                | watcher::Event::InitDone => {
                    // Initial watch events - can be ignored if not needed
                }
            }
        }
        Err(e) => {
            warn!("Watch error: {}", e);
            // Continue watching - errors are usually transient
        }
    }
}
```

### Resource Operations

Use API methods for resource operations:

```rust
// Get resource
let config = api.get("my-config").await?;

// List resources
let list = api.list(&ListParams::default()).await?;

// Create resource
let created = api.create(&PostParams::default(), &new_config).await?;

// Update resource (patch)
let patch_params = PatchParams::apply("controller-name").force();
api.patch(
    "my-config",
    &patch_params,
    &Patch::Merge(patch_data),
)
.await?;

// Delete resource
api.delete("my-config", &DeleteParams::default()).await?;
```

### Error Handling

Handle `kube::Error` types appropriately:

```rust
use kube::Error;

match api.get("resource-name").await {
    Ok(resource) => {
        // Resource exists
    }
    Err(Error::Api(error_response)) if error_response.code == 404 => {
        // Resource not found - this is often expected
        warn!("Resource not found: {}", error_response.message);
    }
    Err(e) => {
        // Other errors
        error!("Failed to get resource: {}", e);
        return Err(e.into());
    }
}
```

**Common patterns:**
- Check for 404 errors explicitly (resource may not exist yet)
- Use `.ok()` to convert `Result` to `Option` when 404 is expected
- Convert `kube::Error` to `anyhow::Error` with `.into()` or `.context()`

### Field Selectors

Use field selectors to watch specific resources:

```rust
let watcher_config = watcher::Config::default()
    .fields(&format!("metadata.name={}", resource_name));

let stream = watcher(api, watcher_config);
```

### Best Practices

1. **Reuse clients**: Clone `Client` instances rather than creating new ones
2. **Use typed APIs**: Prefer `Api<SecretManagerConfig>` over `Api<DynamicObject>` when possible
3. **Handle 404s**: Explicitly handle 404 errors (resources may not exist yet)
4. **Watch configuration**: Use appropriate watch configurations (field selectors, semantic changes)
5. **Error context**: Add context to kube errors when converting to `anyhow::Error`
6. **Resource names**: Always handle `Option<String>` for resource names from metadata

**Example:**
```rust
// Good: Handle Option for name
let name = config.metadata.name.as_deref().unwrap_or("unknown");
let namespace = config.metadata.namespace.as_deref().unwrap_or("default");

// Good: Explicit 404 handling
let resource = api.get(name).await
    .context(format!("Failed to get resource: {}", name))?;

// Good: Clone client for reuse
let api: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);
```

## Testing

### Test Organization

- **Unit tests**: In the same file as the code (in `#[cfg(test)]` module)
- **Integration tests**: In `tests/` directory
- **Test modules**: Use `mod tests` within the source file

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_function() {
        // Test implementation
    }
}
```

### Test Naming

- **Test functions**: Use `test_` prefix or `#[test]` attribute
- **Test modules**: Use `tests` or descriptive names

**Example:**
```rust
#[tokio::test]
async fn test_reconcile_success() {
    // Test implementation
}

#[tokio::test]
async fn test_reconcile_validation_error() {
    // Test implementation
}
```

### Test Documentation

Document complex tests:

```rust
/// Test that reconciliation handles missing GitRepository gracefully.
#[tokio::test]
async fn test_reconcile_missing_git_repository() {
    // Test implementation
}
```

---

## Best Practices

### ✅ DO

1. **Format code with `cargo fmt`**
2. **Run `cargo clippy` before committing**
3. **Document all public APIs**
4. **Use descriptive names**
5. **Organize imports logically**
6. **Use `Result` for error handling**
7. **Add context to errors**
8. **Write tests for new code**
9. **Follow module organization patterns**
10. **Use async/await for async operations**

### ❌ DON'T

1. **Don't use `unwrap()`** (use proper error handling)
2. **Don't ignore clippy warnings** (fix them)
3. **Don't commit unformatted code**
4. **Don't use `expect()` without good reason**
5. **Don't skip documentation for public APIs**
6. **Don't use generic names** (`data`, `value`, `result`)
7. **Don't mix error types unnecessarily**
8. **Don't ignore test failures**
9. **Don't use `unsafe` without documentation**
10. **Don't create circular dependencies**

---

## Code Review Checklist

Before submitting code for review, ensure:

- [ ] Code is formatted with `cargo fmt`
- [ ] Clippy passes with no warnings
- [ ] All public APIs are documented
- [ ] Tests pass (`cargo test`)
- [ ] No `unwrap()` calls (use proper error handling)
- [ ] Error handling follows guidelines
- [ ] Logging follows guidelines
- [ ] Module organization is clear
- [ ] Imports are organized correctly
- [ ] Naming conventions are followed

---

## Examples

### Good Code Style

```rust
//! # Provider
//!
//! Cloud provider implementations for secret management.

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{error, info};

use crate::crd::SecretManagerConfig;

/// Create or update a secret in the cloud provider.
///
/// # Arguments
///
/// * `name` - The name of the secret
/// * `value` - The secret value
///
/// # Returns
///
/// Returns `Ok(true)` if the secret was created, `Ok(false)` if it was updated.
///
/// # Errors
///
/// Returns an error if the provider API call fails.
pub async fn create_or_update_secret(
    &self,
    name: &str,
    value: &str,
) -> Result<bool> {
    info!("Creating or updating secret: {}", name);
    
    match self.get_secret(name).await {
        Ok(Some(_)) => {
            self.update_secret(name, value)
                .await
                .context(format!("Failed to update secret: {}", name))?;
            Ok(false)
        }
        Ok(None) => {
            self.create_secret(name, value)
                .await
                .context(format!("Failed to create secret: {}", name))?;
            Ok(true)
        }
        Err(e) => {
            error!("Failed to check secret existence: {}", e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_or_update_secret() {
        // Test implementation
    }
}
```

### Bad Code Style

```rust
// No module documentation
use crate::crd::SecretManagerConfig;
use anyhow::Result;

// No documentation
// Generic function name
// Uses unwrap()
pub async fn process(config: SecretManagerConfig) -> Result<()> {
    let name = config.metadata.name.unwrap();
    let value = get_value().unwrap();
    create_secret(name, value).await.unwrap();
    Ok(())
}
```

---

## Summary

- **Formatting**: Use `cargo fmt` (enforced in pre-commit)
- **Linting**: Use `cargo clippy` (warnings, `unwrap()` denied)
- **Naming**: Follow Rust conventions (PascalCase for types, snake_case for functions)
- **Documentation**: Document all public APIs with `///`
- **Organization**: Organize modules hierarchically, group imports logically
- **Error Handling**: Use `Result`, avoid `unwrap()`, add context
- **Testing**: Write tests, organize in `#[cfg(test)]` modules
- **Async**: Use `async fn` and `await`, prefer `?` operator

Following these guidelines ensures consistent, maintainable, and high-quality code across the project.

