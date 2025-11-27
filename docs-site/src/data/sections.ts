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

/**
 * Check if the Pact Mock Secrets Viewer should be enabled.
 * - In Node.js (build scripts): checks process.env.ENABLE_PACT_VIEWER
 * - In browser (app): checks import.meta.env.VITE_ENABLE_PACT_VIEWER (via type assertion)
 * - Defaults to true (enabled) unless explicitly set to 'false'
 */
function isPactViewerEnabled(): boolean {
  // Check if we're in a Node.js environment (build scripts)
  if (typeof process !== 'undefined' && process.env) {
    const envValue = process.env.ENABLE_PACT_VIEWER;
    if (envValue !== undefined) {
      return envValue !== 'false' && envValue !== '0';
    }
  }
  
  // Check if we're in a browser/Vite environment
  // Access import.meta via type assertion to avoid parse errors in Node.js contexts
  // Only check this if we're not in a Node.js environment (process is undefined)
  if (typeof process === 'undefined') {
    try {
      // Use a type assertion to access import.meta safely
      // This works in Vite/browser but won't cause parse errors in Node.js
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const metaEnv = (import.meta as any)?.env;
      if (metaEnv) {
        const viteValue = metaEnv.VITE_ENABLE_PACT_VIEWER;
        if (viteValue !== undefined) {
          return viteValue !== 'false' && viteValue !== '0';
        }
        // In production builds, disable by default unless explicitly enabled
        if (metaEnv.PROD) {
          return false;
        }
      }
    } catch {
      // If import.meta is not available, fall through to default
    }
  }
  
  // Default: enabled (for local development/Tilt)
  return true;
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
      { id: 'hot-reload', title: 'Hot Reload', file: 'guides/hot-reload.md' },
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
      { id: 'postgres-manager', title: 'Postgres Manager', file: 'development/postgres-manager.md' },
      { id: 'pact-mocks-manager', title: 'Pact Mocks Manager', file: 'development/pact-mocks-manager.md' },
      { id: 'python-scripts', title: 'Python Scripts', file: 'development/python-scripts.md' },
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
      // Conditionally include Pact Mock Secrets Viewer (development/testing only, not in production builds)
      ...(isPactViewerEnabled() ? [{ id: 'secrets-viewer', title: 'Pact Mock Secrets Viewer', file: '' }] : []),
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

