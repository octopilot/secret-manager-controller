//! # Git Pulls Commands
//!
//! Commands to suspend and resume Git pulls for SecretManagerConfig resources.

use anyhow::{Context, Result};
use controller::crd::SecretManagerConfig;
use kube::{Client, api::Api, api::Patch, api::PatchParams};
use serde_json::json;

/// Suspend Git pulls for a SecretManagerConfig resource
/// Sets suspendGitPulls: true in the spec, which suspends GitRepository pulls but continues reconciliation
pub async fn suspend_git_pulls_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("⏸️  Suspending Git pulls for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already suspended
    if resource.spec.suspend_git_pulls {
        println!("   ℹ️  Git pulls are already suspended");
        return Ok(());
    }

    // Patch the resource to set suspendGitPulls: true
    let patch = json!({
        "spec": {
            "suspendGitPulls": true
        }
    });

    // Use Patch::Merge for spec changes - simpler and more reliable
    let patch_params = PatchParams::default();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| {
            format!("Failed to suspend Git pulls for SecretManagerConfig '{ns}/{name}'")
        })?;

    println!("✅ Git pulls suspended successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Git pulls paused (reconciliation continues with last commit)");
    println!("\nTo resume Git pulls, run:");
    println!("   msmctl resume-git-pulls secretmanagerconfig {name} --namespace {ns}");

    Ok(())
}

/// Resume Git pulls for a SecretManagerConfig resource
/// Sets suspendGitPulls: false in the spec, which resumes GitRepository pulls
pub async fn resume_git_pulls_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("▶️  Resuming Git pulls for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already resumed
    if !resource.spec.suspend_git_pulls {
        println!("   ℹ️  Git pulls are already active");
        return Ok(());
    }

    // Patch the resource to set suspendGitPulls: false
    let patch = json!({
        "spec": {
            "suspendGitPulls": false
        }
    });

    // Use Patch::Merge for spec changes - simpler and more reliable
    let patch_params = PatchParams::default();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| {
            format!("Failed to resume Git pulls for SecretManagerConfig '{ns}/{name}'")
        })?;

    println!("✅ Git pulls resumed successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Git pulls enabled (will fetch new commits)");
    println!("\nThe controller will resume pulling from Git shortly.");

    Ok(())
}
