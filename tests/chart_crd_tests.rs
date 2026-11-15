//! The Helm chart ships its own copy of the CRD (helm/secret-manager-controller/crds),
//! because a chart can only package files under its own directory. It must be the
//! file crdgen writes to config/crd, which pr.yml checks against the Rust types.

#[test]
fn chart_crd_is_the_generated_crd() {
    let root = env!("CARGO_MANIFEST_DIR");
    let generated = std::fs::read_to_string(format!("{root}/config/crd/secretmanagerconfig.yaml"))
        .expect("read config/crd/secretmanagerconfig.yaml");
    let chart = std::fs::read_to_string(format!(
        "{root}/helm/secret-manager-controller/crds/secretmanagerconfig.yaml"
    ))
    .expect("read the chart's CRD");
    assert!(
        generated == chart,
        "helm/secret-manager-controller/crds/secretmanagerconfig.yaml differs from config/crd: \
         copy the regenerated CRD into the chart"
    );
}
