//! # Kustomize Build Execution
//!
//! Handles execution of `kustomize build` command.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::{error, info};

/// Run kustomize build on the specified path
/// Returns the YAML output as a string
pub fn run_kustomize_build(artifact_path: &Path, kustomize_path: &str) -> Result<String> {
    let full_path = artifact_path.join(kustomize_path);

    // Validate path exists
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Kustomize path does not exist: {}",
            full_path.display()
        ));
    }

    // Check if kustomization.yaml exists
    let kustomization_file = full_path.join("kustomization.yaml");
    if !kustomization_file.exists() {
        return Err(anyhow::anyhow!(
            "kustomization.yaml not found at: {}",
            kustomization_file.display()
        ));
    }

    info!("Running kustomize build on path: {}", full_path.display());

    // Run kustomize build
    let mut command = Command::new("kustomize");
    command
        .arg("build")
        .arg(&full_path)
        .current_dir(artifact_path);
    let output = output_with_timeout(command, KUSTOMIZE_TIMEOUT)
        .context("Failed to execute kustomize build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Kustomize build failed: {}", stderr);
        return Err(anyhow::anyhow!("Kustomize build failed: {stderr}"));
    }

    let yaml_output =
        String::from_utf8(output.stdout).context("Failed to decode kustomize output as UTF-8")?;

    Ok(yaml_output)
}

/// Longest a `kustomize build` may run (remote bases make it wait on the network).
const KUSTOMIZE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Run `command` to completion and collect its output, killing it if it runs longer than
/// `timeout`.
fn output_with_timeout(
    mut command: Command,
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Drain both pipes on their own threads: a child blocked on a full pipe would never
    // exit, and never be seen to exit.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut buffer);
            }
            buffer
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "did not finish within {}s and was stopped",
                timeout.as_secs()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };

    Ok(std::process::Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_collected_from_a_command_that_finishes() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo out; echo err >&2; exit 3"]);
        let output = output_with_timeout(command, std::time::Duration::from_secs(10)).expect("ran");
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "out\n");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "err\n");
    }

    #[test]
    fn a_command_that_overruns_is_stopped() {
        let mut command = Command::new("sleep");
        command.arg("30");
        let started = std::time::Instant::now();
        let result = output_with_timeout(command, std::time::Duration::from_millis(200));
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn output_larger_than_a_pipe_buffer_does_not_stall() {
        let mut command = Command::new("sh");
        command.args(["-c", "head -c 300000 /dev/zero | tr '\\0' x"]);
        let output = output_with_timeout(command, std::time::Duration::from_secs(10)).expect("ran");
        assert_eq!(output.stdout.len(), 300_000);
    }
}
