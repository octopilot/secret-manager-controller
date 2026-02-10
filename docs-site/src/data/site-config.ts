/**
 * Single source of truth for GitHub org and docs URLs.
 * When the GitHub organization changes, update these values and rebuild the docs site.
 * Also update any hardcoded GitHub URLs in markdown under src/data/content/ (e.g. installation.md).
 */
export const GITHUB_ORG = 'octopilot';
export const GITHUB_REPO = `${GITHUB_ORG}/secret-manager-controller`;
export const GITHUB_REPO_URL = `https://github.com/${GITHUB_REPO}`;
export const DOCS_BASE_URL = `https://${GITHUB_ORG}.github.io/secret-manager-controller`;
