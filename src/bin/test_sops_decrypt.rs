//! Test binary for SOPS decryption validation.
//!
//! This binary decrypts test files and prints their content to console.
//! TEMPORARY: For testing purposes only - will be removed after validation.
//!
//! **SECURITY**: Uses `parser::decrypt_sops_content()` which:
//! - Pipes encrypted content to SOPS stdin (no disk writes)
//! - Captures decrypted output from stdout (only in memory)
//! - Never writes decrypted content to disk

use anyhow::{Context, Result};
use secret_manager_controller::controller::parser;
use std::env;
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get test files directory from environment or use default
    let test_dir =
        env::var("TEST_FILES_DIR").unwrap_or_else(|_| "/tmp/test-sops-files".to_string());
    let test_dir = PathBuf::from(test_dir);

    // Get SOPS private key from environment (should be set in container)
    let sops_private_key = env::var("SOPS_PRIVATE_KEY").ok();

    if sops_private_key.is_none() {
        eprintln!("⚠️  Warning: SOPS_PRIVATE_KEY not set, decryption may fail");
    }

    println!("🔓 SOPS Decryption Test");
    println!("{}", "=".repeat(80));
    println!("Test directory: {}", test_dir.display());
    println!("SOPS key available: {}", sops_private_key.is_some());
    println!();

    // Files to decrypt
    let files = vec![
        ("application.properties", "Properties file"),
        ("application.secrets.env", "Secrets ENV file"),
        ("application.secrets.yaml", "Secrets YAML file"),
    ];

    for (filename, description) in files {
        let file_path = test_dir.join(filename);

        println!("📄 {}: {}", description, filename);
        println!("{}", "-".repeat(80));

        if !file_path.exists() {
            println!("❌ File not found: {}", file_path.display());
            println!();
            continue;
        }

        // Read file content
        let content = tokio::fs::read_to_string(&file_path)
            .await
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // Check if file is SOPS-encrypted
        let is_sops_encrypted = content.contains("sops:") || content.contains("ENC[");

        if is_sops_encrypted {
            println!("🔐 File is SOPS-encrypted, decrypting...");

            // Decrypt using parser::decrypt_sops_content()
            // SECURITY: This uses stdin/stdout pipes - no decrypted content written to disk
            // Decrypted content exists only in memory (the `decrypted` String)
            match parser::decrypt_sops_content(&content, sops_private_key.as_deref()).await {
                Ok(decrypted) => {
                    println!("✅ Decryption successful!");
                    println!();
                    println!("Decrypted content:");
                    println!("{}", "=".repeat(80));
                    // NOTE: Printing decrypted content to console for testing purposes only
                    // In production, this would never be logged or printed
                    println!("{}", decrypted);
                    println!("{}", "=".repeat(80));
                    // Also log via tracing for container logs (but not the actual secret values)
                    info!("Decrypted {} successfully", filename);
                    info!("Decrypted content length: {} bytes", decrypted.len());
                }
                Err(e) => {
                    println!("❌ Decryption failed: {}", e);
                    // Only show encrypted content preview (safe to display)
                    println!("Raw encrypted content (first 200 chars):");
                    println!("{}", &content.chars().take(200).collect::<String>());
                }
            }
        } else {
            println!("ℹ️  File is not SOPS-encrypted, showing raw content:");
            println!("{}", "=".repeat(80));
            println!("{}", content);
            println!("{}", "=".repeat(80));
        }

        println!();
    }

    println!("✅ Decryption test completed");
    Ok(())
}
