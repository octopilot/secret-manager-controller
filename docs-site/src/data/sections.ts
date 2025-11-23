export type DocCategory = 'user' | 'contributor';

export interface DocSection {
  id: string;
  title: string;
  pages: DocPage[];
}

export interface DocPage {
  id: string;
  title: string;
  file: string;
}

export const userSections: DocSection[] = [
  {
    id: 'getting-started',
    title: 'Getting Started',
    pages: [
      { id: 'installation', title: 'Installation', file: 'getting-started/installation.md' },
      { id: 'quick-start', title: 'Quick Start', file: 'getting-started/quick-start.md' },
      { id: 'configuration', title: 'Configuration', file: 'getting-started/configuration.md' },
    ],
  },
  {
    id: 'architecture',
    title: 'Architecture',
    pages: [
      { id: 'overview', title: 'Overview', file: 'architecture/overview.md' },
      { id: 'components', title: 'Components', file: 'architecture/components.md' },
      { id: 'serverless-integration', title: 'Serverless Integration', file: 'architecture/serverless-integration.md' },
    ],
  },
  {
    id: 'guides',
    title: 'Guides',
    pages: [
      { id: 'aws-setup', title: 'AWS Setup', file: 'guides/aws-setup.md' },
      { id: 'azure-setup', title: 'Azure Setup', file: 'guides/azure-setup.md' },
      { id: 'gcp-setup', title: 'GCP Setup', file: 'guides/gcp-setup.md' },
      { id: 'gitops-integration', title: 'GitOps Integration', file: 'guides/gitops-integration.md' },
      { id: 'sops-setup', title: 'SOPS Setup', file: 'guides/sops-setup.md' },
      { id: 'application-files', title: 'Application Files', file: 'guides/application-files.md' },
      { id: 'msmctl-cli', title: 'MSMCTL CLI', file: 'guides/msmctl-cli.md' },
    ],
  },
  {
    id: 'api-reference',
    title: 'API Reference',
    pages: [
      { id: 'crd-reference', title: 'CRD Reference', file: 'api-reference/crd-reference.md' },
      { id: 'provider-apis', title: 'Provider APIs', file: 'api-reference/provider-apis.md' },
      { id: 'configuration-options', title: 'Configuration Options', file: 'api-reference/configuration-options.md' },
    ],
  },
  {
    id: 'tutorials',
    title: 'Tutorials',
    pages: [
      { id: 'basic-usage', title: 'Basic Usage', file: 'tutorials/basic-usage.md' },
      { id: 'advanced-scenarios', title: 'Advanced Scenarios', file: 'tutorials/advanced-scenarios.md' },
      { id: 'troubleshooting', title: 'Troubleshooting', file: 'tutorials/troubleshooting.md' },
    ],
  },
  {
    id: 'monitoring',
    title: 'Monitoring & Observability',
    pages: [
      { id: 'observability-guide', title: 'Observability Guide', file: 'monitoring/observability-guide.md' },
      { id: 'metrics', title: 'Prometheus Metrics', file: 'monitoring/metrics.md' },
      { id: 'tracing', title: 'Logging & Tracing', file: 'monitoring/tracing.md' },
      { id: 'opentelemetry', title: 'OpenTelemetry', file: 'monitoring/opentelemetry.md' },
      { id: 'datadog', title: 'Datadog APM', file: 'monitoring/datadog.md' },
    ],
  },
];

export const contributorSections: DocSection[] = [
  {
    id: 'development',
    title: 'Development',
    pages: [
      { id: 'setup', title: 'Development Setup', file: 'development/setup.md' },
      { id: 'tilt-integration', title: 'Tilt Integration', file: 'development/tilt-integration.md' },
      { id: 'kind-cluster-setup', title: 'Kind Cluster Setup', file: 'development/kind-cluster-setup.md' },
    ],
  },
  {
    id: 'testing',
    title: 'Testing',
    pages: [
      { id: 'testing-guide', title: 'Testing Guide', file: 'testing/testing-guide.md' },
      { id: 'pact-overview', title: 'Pact Testing Overview', file: 'testing/pact-testing/overview.md' },
      { id: 'pact-architecture', title: 'Pact Testing Architecture', file: 'testing/pact-testing/architecture.md' },
      { id: 'pact-setup', title: 'Pact Testing Setup', file: 'testing/pact-testing/setup.md' },
      { id: 'pact-writing-tests', title: 'Writing Pact Tests', file: 'testing/pact-testing/writing-tests.md' },
      { id: 'integration-testing', title: 'Integration Testing', file: 'testing/integration-testing.md' },
    ],
  },
  {
    id: 'architecture',
    title: 'Architecture',
    pages: [
      { id: 'design-decisions', title: 'Design Decisions', file: 'architecture/design-decisions.md' },
      { id: 'crd-design', title: 'CRD Design', file: 'architecture/crd-design.md' },
      { id: 'implementation-details', title: 'Implementation Details', file: 'architecture/implementation-details.md' },
    ],
  },
  {
    id: 'guidelines',
    title: 'Guidelines',
    pages: [
      { id: 'conventional-commits', title: 'Conventional Commits', file: 'guidelines/conventional-commits.md' },
      { id: 'error-handling', title: 'Error Handling', file: 'guidelines/error-handling.md' },
      { id: 'logging', title: 'Logging', file: 'guidelines/logging.md' },
      { id: 'code-style', title: 'Code Style', file: 'guidelines/code-style.md' },
    ],
  },
  {
    id: 'contributing',
    title: 'Contributing',
    pages: [
      { id: 'contributing-guide', title: 'Contributing Guide', file: 'contributing/contributing-guide.md' },
    ],
  },
];

