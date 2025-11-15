fn main() {
    // Always use environment variables if provided (from Tiltfile)
    // This ensures we get the exact timestamp from the build process
    let timestamp = if let Ok(ts) = std::env::var("BUILD_TIMESTAMP") {
        ts.parse::<u64>().unwrap_or_else(|_| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        })
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    };

    let datetime = std::env::var("BUILD_DATETIME").unwrap_or_else(|_| {
        chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string()
    });

    let git_hash = std::env::var("BUILD_GIT_HASH")
        .unwrap_or_else(|_| get_git_hash().unwrap_or_else(|| "unknown".to_string()));

    println!("cargo:rustc-env=BUILD_TIMESTAMP={timestamp}");
    println!("cargo:rustc-env=BUILD_DATETIME={datetime}");
    println!("cargo:rustc-env=BUILD_GIT_HASH={git_hash}");

    // Force rebuild by always rerunning (timestamp changes every build)
    // This ensures the binary hash changes even if source code is identical
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BUILD_TIMESTAMP");
}

fn get_git_hash() -> Option<String> {
    // Always use command-line git to avoid OpenSSL dependency issues
    // This works for both native and cross-compilation builds
    use std::process::Command;

    // Get git hash using command-line git
    let hash_output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !hash_output.status.success() {
        return None;
    }
    let hash = String::from_utf8(hash_output.stdout).ok()?;
    let short_hash = hash.trim();

    // Check if working directory is dirty
    let diff_output = Command::new("git").args(["diff", "--quiet"]).output().ok();
    let is_dirty = diff_output.is_some_and(|output| !output.status.success());

    let suffix = if is_dirty { "-dirty" } else { "" };
    Some(format!("{short_hash}{suffix}"))
}
