# secret-manager-controller Helm chart

Published by the Octopilot pipeline with every release, as an OCI chart:

    oci://ghcr.io/octopilot/secret-manager-controller-chart    (tag vX.Y.Z)

    helm install smc oci://ghcr.io/octopilot/secret-manager-controller-chart --version vX.Y.Z

Chart `X.Y.Z` deploys image `ghcr.io/octopilot/secret-manager-controller:vX.Y.Z`.
With Flux, an OCIRepository with `ref.tag: vX.Y.Z` and
`layerSelector.mediaType: application/vnd.cncf.helm.chart.content.v1.tar+gzip`
(`operation: copy`).

The CRD ships in `crds/`. Helm installs it once and never upgrades it; with Flux set
`install.crds` and `upgrade.crds` to `CreateReplace` on the HelmRelease.

With Flux sources set `flux.sourceControllerAccess.enabled: true`: Flux's own
NetworkPolicies admit only its namespace to source-controller, and the controller
downloads source artifacts from it.

The controller needs a cloud identity, set through `serviceAccount.annotations`
(Workload Identity, IRSA), and the SOPS private key as a Secret named
`sops-private-key` (key `private-key`) in the namespace of each SecretManagerConfig.

`helm test` checks that the controller is up, ready and serving metrics.
See `values.yaml` for every setting.
