//! # Explicit secret files (`spec.secrets.files`)
//!
//! A SecretManagerConfig may name its secret files outright instead of relying on the
//! `deployment-configuration/profiles/{env}/application.secrets.*` discovery.

use controller::controller::parser::{explicit_application_files, parse_secrets};
use std::fs;
use std::path::{Path, PathBuf};

/// A scratch source tree, removed when dropped.
struct Source(PathBuf);

impl Source {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn source(test: &str) -> Source {
    let dir = Source(
        std::env::temp_dir().join(format!("smc-explicit-files-{}-{test}", std::process::id())),
    );
    let _ = fs::remove_dir_all(dir.path());
    let nested = dir
        .path()
        .join("deployment-configuration/profiles/dev/app/broker");
    fs::create_dir_all(&nested).expect("create dirs");
    fs::write(
        nested.join("ib-credentials.secrets.env"),
        "IB_USER=alice\nIB_PASSWORD=s3cret\n",
    )
    .expect("write env");
    fs::write(nested.join("application.properties"), "not=read\n").expect("write properties");
    fs::write(
        nested.join("gate.secret.yaml"),
        "apiVersion: v1\nkind: Secret\nmetadata:\n  name: gate\n  namespace: edge\ntype: Opaque\n\
         data:\n  token: aHVudGVyMg==\nstringData:\n  user: preview\n",
    )
    .expect("write yaml");
    dir
}

#[tokio::test]
async fn named_env_file_is_read_whatever_its_name_or_depth() {
    let dir = source("env");
    let files = [
        "deployment-configuration/profiles/dev/app/broker/ib-credentials.secrets.env".to_string(),
    ];
    let found = explicit_application_files(dir.path(), &files, "broker").expect("found");
    assert_eq!(found.len(), 1);
    assert!(found[0].secrets_env.is_some());
    assert!(
        found[0].properties.is_none(),
        "properties are not picked up"
    );

    let secrets = parse_secrets(&found[0], None).await.expect("parse");
    assert_eq!(secrets.get("IB_USER").map(String::as_str), Some("alice"));
    assert_eq!(
        secrets.get("IB_PASSWORD").map(String::as_str),
        Some("s3cret")
    );
    assert_eq!(secrets.len(), 2);
}

#[tokio::test]
async fn kubernetes_secret_manifest_yields_its_data_and_string_data() {
    let dir = source("manifest");
    let files = ["deployment-configuration/profiles/dev/app/broker/gate.secret.yaml".to_string()];
    let found = explicit_application_files(dir.path(), &files, "edge").expect("found");
    assert!(found[0].secrets_yaml.is_some());

    let secrets = parse_secrets(&found[0], None).await.expect("parse");
    assert_eq!(secrets.get("token").map(String::as_str), Some("hunter2"));
    assert_eq!(secrets.get("user").map(String::as_str), Some("preview"));
    assert_eq!(secrets.len(), 2, "metadata is not a secret: {secrets:?}");
}

#[test]
fn files_outside_the_source_missing_or_of_unknown_type_are_refused() {
    let dir = source("refused");
    for bad in [
        "../outside.secrets.env",
        "/etc/passwd",
        "",
        "deployment-configuration/profiles/dev/app/broker/absent.secrets.env",
        "deployment-configuration/profiles/dev/app/broker/application.properties",
    ] {
        let result = explicit_application_files(dir.path(), &[bad.to_string()], "x");
        assert!(result.is_err(), "'{bad}' must be refused");
    }
}
