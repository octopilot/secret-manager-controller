//! # GPG Key Management
//!
//! Handles importing GPG private keys into temporary keyrings for SOPS decryption.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// The private keys held in one SOPS key value, by kind.
///
/// A key value may hold an age identity, an armored GPG private key, or both: a
/// repository in the middle of a move from PGP to age (or one that keeps both on
/// purpose) has files encrypted either way, and one controller must decrypt them all.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SopsKeyMaterial {
    /// age identities (`AGE-SECRET-KEY-1...`), one per line.
    pub age: Option<String>,
    /// Armored GPG private key blocks.
    pub gpg: Option<String>,
}

/// Separate the age identities from the GPG private key blocks in a SOPS key value.
///
/// Text that is neither (and is not blank or a comment) is handed to gpg as it always
/// was, so a key in a form this does not recognise still gets its chance to import.
#[must_use]
pub fn split_sops_keys(private_key: &str) -> SopsKeyMaterial {
    const BEGIN: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----";
    const END: &str = "-----END PGP PRIVATE KEY BLOCK-----";

    let mut age = Vec::new();
    let mut gpg = Vec::new();
    let mut other = Vec::new();
    let mut in_block = false;
    for line in private_key.lines() {
        let trimmed = line.trim();
        if in_block {
            gpg.push(line.trim_end());
            in_block = !trimmed.starts_with(END);
        } else if trimmed.starts_with(BEGIN) {
            gpg.push(line.trim_end());
            in_block = true;
        } else if trimmed.starts_with("AGE-SECRET-KEY-") {
            age.push(trimmed);
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            other.push(line.trim_end());
        }
    }

    let gpg = if !gpg.is_empty() {
        Some(gpg.join("\n") + "\n")
    } else if age.is_empty() && !other.is_empty() {
        Some(other.join("\n") + "\n")
    } else {
        None
    };
    SopsKeyMaterial {
        age: (!age.is_empty()).then(|| age.join("\n")),
        gpg,
    }
}

/// Longest a gpg key import may run.
const GPG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// True when the SOPS private key is an age identity (`AGE-SECRET-KEY-1...`, with or
/// without the comment lines of an age key file) rather than an armored GPG key.
pub fn is_age_key(private_key: &str) -> bool {
    private_key
        .lines()
        .any(|line| line.trim_start().starts_with("AGE-SECRET-KEY-"))
}

/// Import GPG private key into a temporary GPG home directory
pub(crate) async fn import_gpg_key(private_key: &str) -> Result<Option<PathBuf>> {
    let gpg_path = match which::which("gpg") {
        Ok(path) => path,
        Err(_) => {
            warn!(
                "gpg binary not found - SOPS decryption may fail if key is not in system keyring"
            );
            return Ok(None);
        }
    };

    let temp_dir = std::env::temp_dir();
    let gpg_home = temp_dir.join(format!("gpg-home-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&gpg_home)
        .await
        .context("Failed to create temporary GPG home directory")?;

    debug!("Created temporary GPG home: {:?}", gpg_home);

    let gpg_path_for_trust = gpg_path.clone();
    let mut cmd = tokio::process::Command::new(&gpg_path);
    cmd.env("GNUPGHOME", &gpg_home)
        .arg("--batch")
        .arg("--yes")
        .arg("--pinentry-mode")
        .arg("loopback")
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().context("Failed to spawn gpg import command")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(private_key.as_bytes())
            .await
            .context("Failed to write GPG private key to stdin")?;
        stdin.shutdown().await.context("Failed to close stdin")?;
    }

    let waited = tokio::time::timeout(GPG_TIMEOUT, child.wait_with_output()).await;
    let Ok(output) = waited else {
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        anyhow::bail!(
            "gpg --import did not finish within {}s and was stopped",
            GPG_TIMEOUT.as_secs()
        );
    };
    let output = output.context("Failed to wait for gpg import command")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        info!("✅ GPG key import command succeeded");
        debug!("GPG import stdout: {}", stdout);
        if !stderr.is_empty() {
            debug!("GPG import stderr: {}", stderr);
        }

        let gpg_home_clone = gpg_home.clone();
        let trust_output = tokio::process::Command::new(&gpg_path_for_trust)
            .env("GNUPGHOME", &gpg_home_clone)
            .arg("--list-keys")
            .arg("--with-colons")
            .arg("--fingerprint")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let mut imported_fingerprint: Option<String> = None;
        if let Ok(list_output) = trust_output {
            if list_output.status.success() {
                let output_str = String::from_utf8_lossy(&list_output.stdout);
                for line in output_str.lines() {
                    if line.starts_with("fpr:") {
                        // GPG colon format: fpr:::::::::FINGERPRINT:
                        // Fingerprint is in the 10th field (index 9)
                        if let Some(fpr) = line.split(':').nth(9) {
                            if !fpr.is_empty() {
                                imported_fingerprint = Some(fpr.to_string());

                                // Set trust level for this key
                                let trust_cmd = tokio::process::Command::new(&gpg_path_for_trust)
                                    .env("GNUPGHOME", &gpg_home_clone)
                                    .arg("--batch")
                                    .arg("--yes")
                                    .arg("--import-ownertrust")
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::piped())
                                    .stderr(Stdio::piped())
                                    .spawn();

                                if let Ok(mut trust_child) = trust_cmd {
                                    let trust_input = format!("{}:6:\n", fpr);
                                    if let Some(mut stdin) = trust_child.stdin.take() {
                                        let _ = stdin.write_all(trust_input.as_bytes()).await;
                                        let _ = stdin.shutdown().await;
                                    }
                                    let _ = trust_child.wait_with_output().await;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let Some(ref fingerprint) = imported_fingerprint {
            info!(
                "✅ Successfully imported GPG private key (fingerprint: {}) into temporary keyring: {:?}",
                fingerprint, gpg_home
            );
        } else {
            warn!(
                "⚠️  Successfully imported GPG private key but could not extract fingerprint - keyring: {:?}",
                gpg_home
            );
        }

        Ok(Some(gpg_home))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        warn!("Failed to import GPG private key");
        warn!("GPG stderr: {}", error_msg);
        warn!("GPG stdout: {}", stdout);
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        Err(anyhow::anyhow!(
            "Failed to import GPG private key: {error_msg}"
        ))
    }
}

#[cfg(test)]
mod key_material_tests {
    use super::{SopsKeyMaterial, split_sops_keys};

    const AGE: &str = "AGE-SECRET-KEY-1EXAMPLEEXAMPLEEXAMPLE";
    const GPG: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nlQOYBGexample\n=abcd\n-----END PGP PRIVATE KEY BLOCK-----";

    #[test]
    fn an_age_key_file_yields_its_identities_only() {
        let material = split_sops_keys(&format!("# created: today\n# public key: age1x\n{AGE}\n"));
        assert_eq!(
            material,
            SopsKeyMaterial {
                age: Some(AGE.to_string()),
                gpg: None
            }
        );
    }

    #[test]
    fn an_armored_gpg_key_yields_the_block_only() {
        let material = split_sops_keys(GPG);
        assert_eq!(material.age, None);
        assert_eq!(material.gpg.as_deref(), Some(format!("{GPG}\n").as_str()));
    }

    /// The mixed case: both kinds in one value, in either order, with Windows line ends.
    #[test]
    fn both_kinds_in_one_value_are_separated() {
        for value in [
            format!("{AGE}\n{GPG}\n"),
            format!("{GPG}\n# public key: age1x\n{AGE}"),
            format!("{AGE}\r\n{}\r\n", GPG.replace('\n', "\r\n")),
        ] {
            let material = split_sops_keys(&value);
            assert_eq!(material.age.as_deref(), Some(AGE), "{value:?}");
            assert_eq!(
                material.gpg.as_deref(),
                Some(format!("{GPG}\n").as_str()),
                "{value:?}"
            );
        }
    }

    #[test]
    fn several_identities_and_several_blocks_are_all_kept() {
        let second = "AGE-SECRET-KEY-1SECONDSECONDSECOND";
        let material = split_sops_keys(&format!("{AGE}\n{GPG}\n{second}\n{GPG}\n"));
        assert_eq!(material.age, Some(format!("{AGE}\n{second}")));
        assert_eq!(material.gpg, Some(format!("{GPG}\n{GPG}\n")));
    }

    #[test]
    fn unrecognised_text_alone_is_left_to_gpg() {
        let material = split_sops_keys("not a key in any form we know\n");
        assert_eq!(material.age, None);
        assert_eq!(
            material.gpg.as_deref(),
            Some("not a key in any form we know\n")
        );
    }

    #[test]
    fn nothing_yields_nothing() {
        assert_eq!(
            split_sops_keys("\n  \n# only a comment\n"),
            SopsKeyMaterial::default()
        );
    }
}
