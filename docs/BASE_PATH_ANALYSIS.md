# Base Path Handling Analysis

## Current Implementation

### How Base Path Works

1. **Configuration (main.rs:54-55)**
   - `base_path` defaults to `"microservices"` (line 61-63)
   - Can be overridden in `SecretManagerConfig` spec
   - Used to locate the root directory for searching application files

2. **Path Construction (reconciler.rs:124, parser.rs:25)**
   ```rust
   let search_path = artifact_path.join(base_path);
   ```
   - `artifact_path`: Flux artifact cache path (e.g., `/tmp/flux-source-flux-system-repo-abc123`)
   - `base_path`: From config spec (default: `"microservices"`)
   - Result: `/tmp/flux-source-.../microservices` (monolith) or `/tmp/flux-source-.../.` (single service)

3. **File Discovery (parser.rs:36-73)**
   - Walks directory tree starting at `search_path`
   - Looks for directories named `deployment-configuration`
   - Extracts service name from parent of `deployment-configuration`
   - Searches subdirectories with `max_depth(2)` for application files

### Current Expected Structure

**Monolith:**
```
{artifact_path}/microservices/{service}/deployment-configuration/profiles/{env}/
  ├── application.secrets.env
  ├── application.secrets.yaml
  └── application.properties
```

**Single Service:**
```
{artifact_path}/deployment-configuration/profiles/{env}/
  ├── application.secrets.env
  ├── application.secrets.yaml
  └── application.properties
```

## Issues Identified

### 1. Service Name Extraction (parser.rs:49-55)

**Problem:** For single service structure, the service name extraction fails:
- Monolith: `parent` of `microservices/idam/deployment-configuration` = `idam` ✅
- Single Service: `parent` of `./deployment-configuration` = `.` (artifact root) ❌
  - Results in `service_name = "unknown"` or incorrect name

**Impact:** Secret prefix will be wrong for single service deployments.

### 2. Profiles Directory Not Explicitly Handled

**Current Behavior:**
- Code uses `max_depth(2)` which happens to work with `profiles/{env}/`
- But it doesn't explicitly look for `profiles/` directory
- Could match other directory structures unintentionally

**Risk:** If someone has `deployment-configuration/{env}/` (without profiles), it might still work but is not the intended structure.

### 3. Base Path Handling for Single Service

**Current:**
- `base_path = "."` or `base_path = ""` should work but isn't explicitly documented
- Empty string might cause issues with `PathBuf.join()`

**Recommendation:** Support both `"."` and empty string, normalize to empty string internally.

## Required Changes

### 1. Update Parser to Explicitly Handle Profiles Directory

**Current Code (parser.rs:57-70):**
```rust
// Look for application files in deployment-configuration subdirectories
for env_dir in WalkDir::new(path)
    .max_depth(2)
    .into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| e.file_type().is_dir())
{
    let env_path = env_dir.path();
    let app_files = find_files_in_directory(&service_name, env_path).await?;
    // ...
}
```

**Should Be:**
```rust
// Look for profiles directory, then environment subdirectories
let profiles_path = path.join("profiles");
if profiles_path.exists() {
    for env_dir in WalkDir::new(&profiles_path)
        .max_depth(1)  // Only go one level deep (profiles/{env})
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let env_path = env_dir.path();
        let app_files = find_files_in_directory(&service_name, env_path).await?;
        // ...
    }
}
```

### 2. Fix Service Name Extraction for Single Service

**Current Code (parser.rs:49-55):**
```rust
if let Some(parent) = path.parent() {
    // Extract service name (parent of deployment-configuration)
    let service_name = parent
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());
```

**Should Handle:**
- If `parent == base_path` (single service case), use a fallback:
  - Use `secret_prefix` from config if provided
  - Or extract from GitRepository name
  - Or use a default like `"default-service"`

### 3. Normalize Base Path Handling

**Add helper function:**
```rust
fn normalize_base_path(base_path: &str) -> &str {
    match base_path {
        "." | "" => "",
        _ => base_path,
    }
}
```

**Use in path construction:**
```rust
let normalized_base = normalize_base_path(base_path);
let search_path = if normalized_base.is_empty() {
    artifact_path.to_path_buf()
} else {
    artifact_path.join(normalized_base)
};
```

## Testing Scenarios

### Scenario 1: Monolith Structure
- **Base Path:** `"microservices"`
- **Structure:** `microservices/idam/deployment-configuration/profiles/dev/`
- **Expected:** Service name = `"idam"`, finds files correctly

### Scenario 2: Single Service Structure
- **Base Path:** `"."` or `""`
- **Structure:** `deployment-configuration/profiles/dev/`
- **Expected:** Service name = from `secret_prefix` or fallback, finds files correctly

### Scenario 3: Custom Base Path
- **Base Path:** `"services"`
- **Structure:** `services/my-service/deployment-configuration/profiles/prod/`
- **Expected:** Service name = `"my-service"`, finds files correctly

## Recommendations

1. **Update parser.rs** to explicitly look for `profiles/` directory
2. **Fix service name extraction** to handle single service case
3. **Normalize base path** handling for empty/root paths
4. **Update documentation** in README.md and examples to show both structures
5. **Add validation** to ensure `profiles/` directory exists (or make it optional for backward compatibility)

## Backward Compatibility

**Consideration:** Some existing deployments might use:
- `deployment-configuration/{env}/` (without profiles)

**Options:**
1. **Strict:** Require `profiles/` directory (breaking change)
2. **Flexible:** Support both `profiles/{env}/` and `{env}/` (preferred)
3. **Configurable:** Add `profiles_enabled: bool` flag (overkill)

**Recommendation:** Option 2 - Check for `profiles/` first, fallback to direct subdirectories.

