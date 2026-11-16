#!/usr/bin/env rust-script
//! Debug program to decrypt SOPS files locally
//!
//! Usage:
//!   cargo run --bin debug-sops -- deployment-configuration/profiles/tilt/application.secrets.env
//!
//! Or with a key (an age identity or an armored GPG private key):
//!   SOPS_PRIVATE_KEY="$(cat ~/.config/sops/age/keys.txt)" cargo run --bin debug-sops -- deployment-configuration/profiles/tilt/application.secrets.env
//!   SOPS_PRIVATE_KEY="$(cat path/to/key.asc)" cargo run --bin debug-sops -- deployment-configuration/profiles/tilt/application.secrets.env
//!
//! This runs the controller's own decryption code, so what decrypts here decrypts there.
//!
//! # Optimal Output Type for Cloud Secret Stores
//!
//! The output type matches the input type to preserve format for parsing:
//!
//! ## .env / application.secrets.env files
//! - **Input type**: `dotenv`
//! - **Output type**: `dotenv` (matches input)
//! - **Why**: Decrypted content is parsed line-by-line as `KEY=VALUE` pairs
//! - **Storage**: Each key-value pair stored as individual secret in cloud provider
//!
//! ## .yaml / application.secrets.yaml files
//! - **Input type**: `yaml`
//! - **Output type**: `yaml` (matches input)
//! - **Why**: Decrypted content is parsed with `serde_yaml`, then flattened
//! - **Storage**: Flattened key-value pairs stored as individual secrets in cloud provider
//!
//! ## Cloud Provider Storage
//! - **GCP Secret Manager**: Stores secrets as strings
//! - **AWS Secrets Manager**: Stores secrets as strings (or JSON strings)
//! - **Azure Key Vault**: Stores secrets as strings
//!
//! **Key Insight**: Format preservation is only needed for parsing, not storage.
//! After parsing, all secrets are stored as strings in cloud providers.
//! The output type must match input type so the parser receives the expected format.

use anyhow::{Context, Result};
use controller::controller::parser::decrypt_sops_content;
use controller::controller::parser::sops::gpg::is_age_key;
use std::env;
use std::fs;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Get file path from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-sops-file>", args[0]);
        eprintln!(
            "Example: {} deployment-configuration/profiles/tilt/application.secrets.env",
            args[0]
        );
        eprintln!("\nOptional: Set SOPS_PRIVATE_KEY environment variable to provide GPG key");
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);

    if !file_path.exists() {
        eprintln!("❌ File not found: {}", file_path.display());
        std::process::exit(1);
    }

    println!("🔍 Debug SOPS Decryption");
    println!("📄 File: {}", file_path.display());
    println!();

    // Read encrypted file
    println!("📖 Reading encrypted file...");
    let encrypted_content = fs::read_to_string(&file_path)
        .context(format!("Failed to read file: {}", file_path.display()))?;

    println!("   File size: {} bytes", encrypted_content.len());
    println!(
        "   First 100 chars: {}",
        &encrypted_content.chars().take(100).collect::<String>()
    );
    println!();

    // Key from the environment (optional): an age identity or an armored GPG key
    let sops_private_key = env::var("SOPS_PRIVATE_KEY").ok();
    match sops_private_key.as_deref() {
        Some(key) if is_age_key(key) => {
            println!(
                "🔑 age identity provided via SOPS_PRIVATE_KEY ({} bytes)",
                key.len()
            );
        }
        Some(key) => {
            println!(
                "🔑 GPG key provided via SOPS_PRIVATE_KEY ({} bytes)",
                key.len()
            );
        }
        None => {
            println!("⚠️  No key provided - sops will use its own defaults (keyring, key file)");
            println!("   Set SOPS_PRIVATE_KEY to an age identity or an armored GPG private key");
        }
    }
    println!();

    // The controller's own decryption
    println!("🔓 Decrypting with SOPS binary...");
    let decrypted = decrypt_sops_content(
        &encrypted_content,
        Some(&file_path),
        sops_private_key.as_deref(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("SOPS decryption failed: {e}"))?;

    println!("✅ Decryption successful!");
    println!("   Decrypted size: {} bytes", decrypted.len());
    println!();
    println!("📋 Decrypted content:");
    println!("{}", "─".repeat(80));
    println!("{}", decrypted);
    println!("{}", "─".repeat(80));

    Ok(())
}
