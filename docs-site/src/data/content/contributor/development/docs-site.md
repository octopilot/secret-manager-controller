# Documentation Site Development

Guide to developing and maintaining the Secret Manager Controller documentation site.

## Overview

The documentation site is a client-side rendered (CSR) application built with:

- **SolidJS** - Reactive UI framework
- **Vite** - Fast build tool with hot module replacement
- **Tailwind CSS** - Utility-first CSS framework
- **Marked** - Markdown parsing
- **Mermaid** - Diagram rendering
- **KaTeX** - Math formula rendering (optional)

**Architecture Decision:** Client-side rendering for fast development iteration (< 1 second vs 30 minutes for static site generators).

## Project Structure

```
docs-site/
├── src/
│   ├── components/
│   │   ├── layout/           # Navigation, ContentArea
│   │   └── content/           # MarkdownRenderer
│   ├── data/
│   │   ├── sections.ts        # Documentation structure (navigation)
│   │   └── content/           # Markdown content files
│   │       ├── user/          # User documentation
│   │       └── contributor/   # Contributor documentation
│   ├── App.tsx                # Main application component
│   └── index.tsx              # Entry point
├── dist/                      # Production build output
├── package.json
├── vite.config.ts             # Vite configuration
├── tailwind.config.js         # Tailwind CSS configuration
└── tsconfig.json              # TypeScript configuration
```

## Getting Started

### Prerequisites

- **Node.js** 18+ and npm
- Basic familiarity with Markdown, TypeScript, and React-like frameworks

### Installation

```bash
cd docs-site
yarn install
```

### Development Server

Start the development server with hot module replacement:

```bash
yarn dev
```

The site will be available at `http://localhost:3002` (port configured in `vite.config.ts`).

**Features:**
- Hot module replacement (instant updates)
- Fast refresh (preserves component state)
- Source maps for debugging

### Production Build

Build the site for production:

```bash
yarn build
```

Output is written to `dist/` directory.

**Build Features:**
- Code splitting (diagrams, markdown parser, content in separate chunks)
- Minification and optimization
- Asset optimization

### Preview Production Build

Preview the production build locally:

```bash
yarn preview
```

---

## Adding New Pages

### Step 1: Create Markdown File

Create a new markdown file in the appropriate directory:

**User Documentation:**
```
docs-site/src/data/content/user/{section}/{page-name}.md
```

**Contributor Documentation:**
```
docs-site/src/data/content/contributor/{section}/{page-name}.md
```

**Example:**
```bash
# Create a new guide
touch docs-site/src/data/content/user/guides/new-feature.md
```

### Step 2: Add to Navigation

Edit `docs-site/src/data/sections.ts` to add the page to navigation:

```typescript
export const userSections: DocSection[] = [
  // ... existing sections ...
  {
    id: 'guides',
    title: 'Guides',
    pages: [
      // ... existing pages ...
      { id: 'new-feature', title: 'New Feature', file: 'guides/new-feature.md' },
    ],
  },
];
```

**Fields:**
- `id` (string): Unique identifier for the page (used in URLs)
- `title` (string): Display name in navigation
- `file` (string): Path to markdown file relative to `src/data/content/{category}/`

### Step 3: Write Content

Write your markdown content. The site supports:

- **Standard Markdown**: Headings, paragraphs, lists, links, etc.
- **Code Blocks**: Syntax highlighting for various languages
- **Mermaid Diagrams**: Flowcharts, sequence diagrams, etc.
- **Tables**: Standard markdown tables
- **Math**: KaTeX for mathematical formulas

**Example:**
```markdown
# New Feature Guide

This guide explains how to use the new feature.

## Overview

The new feature provides...

## Usage

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
spec:
  # Configuration here
```

## Architecture

\`\`\`mermaid
graph TB
    A[Start] --> B[Process]
    B --> C[End]
\`\`\`
```

### Step 4: Test Locally

1. Start the development server: `yarn dev`
2. Navigate to the new page in the browser
3. Verify content renders correctly
4. Check navigation appears correctly

---

## Mermaid Diagram Syntax

The documentation site supports Mermaid diagrams for flowcharts, sequence diagrams, and more.

### Basic Flowchart

```markdown
\`\`\`mermaid
graph TB
    A[Start] --> B[Process]
    B --> C{Decision}
    C -->|Yes| D[Action 1]
    C -->|No| E[Action 2]
    D --> F[End]
    E --> F
\`\`\`
```

### Sequence Diagram

```markdown
\`\`\`mermaid
sequenceDiagram
    participant User
    participant Controller
    participant Provider
    
    User->>Controller: Create SecretManagerConfig
    Controller->>Provider: Create Secret
    Provider-->>Controller: Secret Created
    Controller-->>User: Status Updated
\`\`\`
```

### State Diagram

```markdown
\`\`\`mermaid
stateDiagram-v2
    [*] --> Pending
    Pending --> Started: Reconciliation Triggered
    Started --> Cloning: Get Artifact
    Cloning --> Updating: Process Secrets
    Updating --> Ready: Success
    Updating --> Failed: Error
    Failed --> Started: Retry
    Ready --> [*]
\`\`\`
```

### Class Diagram

```markdown
\`\`\`mermaid
classDiagram
    class SecretManagerConfig {
        +sourceRef: SourceRef
        +provider: ProviderConfig
        +secrets: SecretsConfig
    }
    
    class ProviderConfig {
        +gcp: GcpConfig
        +aws: AwsConfig
        +azure: AzureConfig
    }
    
    SecretManagerConfig --> ProviderConfig
\`\`\`
```

**Important:** Use triple backticks with `mermaid` language tag. The renderer automatically detects and renders Mermaid diagrams.

---

## Code Blocks

### Syntax Highlighting

Code blocks support syntax highlighting for various languages:

```markdown
\`\`\`yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
spec:
  sourceRef:
    kind: GitRepository
\`\`\`

\`\`\`rust
pub struct SecretManagerConfig {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
}
\`\`\`

\`\`\`bash
kubectl apply -f config.yaml
\`\`\`
```

### Terminal Blocks (Matrix Style)

Terminal/code blocks are styled with a "Matrix" retro theme:

- **Background**: Dark (`#0d1117`)
- **Text Color**: Green (`#00ff41`)
- **Border**: Green glow effect
- **Width**: 80 columns with wrap-around

**Example:**
```markdown
\`\`\`bash
$ kubectl get secretmanagerconfig
NAME              PHASE    READY
my-secrets        Ready    True
\`\`\`
```

### Inline Code

Inline code uses warm orange styling:

```markdown
Use the `SecretManagerConfig` CRD to configure secrets.
```

---

## Navigation Structure

### Two-Level Hierarchy

The documentation is organized into:

1. **Categories**: `user` or `contributor`
2. **Sections**: Logical groupings (e.g., "Getting Started", "Guides", "API Reference")
3. **Pages**: Individual documentation pages

### Navigation Configuration

Navigation is defined in `src/data/sections.ts`:

```typescript
export const userSections: DocSection[] = [
  {
    id: 'getting-started',
    title: 'Getting Started',
    pages: [
      { id: 'installation', title: 'Installation', file: 'getting-started/installation.md' },
      { id: 'quick-start', title: 'Quick Start', file: 'getting-started/quick-start.md' },
    ],
  },
];
```

### Adding a New Section

1. Create a new section object in `sections.ts`
2. Add pages to the section
3. Create corresponding markdown files
4. Navigation will automatically update

**Example:**
```typescript
{
  id: 'new-section',
  title: 'New Section',
  pages: [
    { id: 'page-1', title: 'Page 1', file: 'new-section/page-1.md' },
    { id: 'page-2', title: 'Page 2', file: 'new-section/page-2.md' },
  ],
}
```

---

## Styling

### Color Palette

The site uses a warm, easy-on-the-eyes color palette:

- **Primary**: Sage green (`#5a6c5d`)
- **Background**: Warm off-white (`#faf9f7`)
- **Text**: Dark gray (`#2d3748`)
- **Links**: Sage green with hover underline
- **Code**: Warm orange (`#c05621`) for inline, green (`#00ff41`) for blocks

### Typography

- **Headings**: Semibold, with increasing sizes (h1: 4xl, h2: 2xl, h3: xl)
- **Body**: Leading 7, comfortable reading
- **Code**: Monospace font stack (SF Mono, Monaco, etc.)

### Custom Styles

Custom styles are defined in:
- `src/index.css` - Global styles, terminal block styling
- `tailwind.config.js` - Tailwind configuration, color palette
- Component classes - Inline Tailwind classes

---

## Content Organization

### User Documentation

Located in `src/data/content/user/`:

- **Getting Started**: Installation, quick start, configuration
- **Architecture**: Overview, components, serverless integration
- **Guides**: Provider setup, GitOps integration, SOPS, CLI
- **API Reference**: CRD reference, provider APIs, configuration options
- **Tutorials**: Basic usage, advanced scenarios, troubleshooting
- **Monitoring**: Metrics, tracing, OpenTelemetry, Datadog

### Contributor Documentation

Located in `src/data/content/contributor/`:

- **Development**: Setup, Kind cluster, Tilt integration
- **Testing**: Testing guide, Pact testing, integration testing
- **Architecture**: Design decisions, CRD design, implementation details
- **Guidelines**: Conventional commits, error handling, logging, code style
- **Contributing**: Contributing guide

---

## Markdown Features

### Supported Markdown

The site uses `marked` with GitHub Flavored Markdown (GFM) enabled:

- **Headings**: `#`, `##`, `###`, etc.
- **Paragraphs**: Standard text blocks
- **Lists**: Ordered and unordered
- **Links**: `[text](url)` and `[text](#anchor)`
- **Images**: `![alt](url)`
- **Code**: Inline `` `code` `` and fenced blocks
- **Tables**: Standard markdown tables
- **Blockquotes**: `> quote`
- **Horizontal Rules**: `---`

### Extended Features

- **Line Breaks**: Enabled (single line break creates new line)
- **GFM**: GitHub Flavored Markdown features (tables, strikethrough, etc.)
- **Mermaid Diagrams**: Automatic detection and rendering
- **KaTeX Math**: Mathematical formulas (if needed)

---

## Building and Deployment

### Local Development

```bash
# Install dependencies
yarn install

# Start dev server
yarn dev

# Build for production
yarn build

# Preview production build
yarn preview
```

### Docker Build

The documentation site is containerized for deployment:

```bash
# Build Docker image
docker build -f dockerfiles/Dockerfile.docs-site -t docs-site .

# Run container
docker run -p 8080:80 docs-site
```

**Dockerfile:** `dockerfiles/Dockerfile.docs-site`

**Features:**
- Multi-stage build (build + nginx serve)
- Optimized production build
- nginx for static file serving

### Tilt Integration

The documentation site is integrated into Tilt for local development:

```python
# Build documentation site Docker image
docker_build(
    'docs-site',
    '.',
    dockerfile='./dockerfiles/Dockerfile.docs-site',
    only=[
        './docs-site',
        './dockerfiles/Dockerfile.docs-site',
    ],
)

# Documentation site service
k8s_resource(
    'docs-site',
    port_forwards='8800:80',
    labels=['docs'],
)
```

**Access:** `http://localhost:8800` when running Tilt

---

## Troubleshooting

### Mermaid Diagrams Not Rendering

**Symptoms:** Diagrams appear as code blocks instead of rendered diagrams.

**Solutions:**
1. Ensure code block uses `\`\`\`mermaid` (not `\`\`\` mermaid` with space)
2. Check browser console for Mermaid errors
3. Verify Mermaid syntax is correct
4. Wait for page to fully load (diagrams render after a short delay)

### Navigation Not Showing New Page

**Symptoms:** New page doesn't appear in navigation menu.

**Solutions:**
1. Verify page is added to `sections.ts` with correct `file` path
2. Check file path matches actual file location
3. Ensure `id` is unique
4. Restart dev server if needed

### Styling Issues

**Symptoms:** Content doesn't match expected styling.

**Solutions:**
1. Check Tailwind classes are correct
2. Verify `index.css` custom styles are applied
3. Clear browser cache
4. Rebuild: `yarn build`

### Build Errors

**Symptoms:** `yarn build` fails.

**Solutions:**
1. Check TypeScript errors: `yarn build` shows errors
2. Verify all imports are correct
3. Check file paths in `sections.ts` match actual files
4. Ensure all dependencies are installed: `yarn install`

---

## Best Practices

### Content Writing

1. **Clear Headings**: Use descriptive headings that summarize the section
2. **Short Paragraphs**: Keep paragraphs concise and focused
3. **Code Examples**: Include working code examples
4. **Diagrams**: Use Mermaid diagrams for complex concepts
5. **Links**: Link to related pages for better navigation

### File Organization

1. **Logical Grouping**: Group related pages in the same section
2. **Consistent Naming**: Use kebab-case for file names (`my-page.md`)
3. **Descriptive IDs**: Use clear, descriptive IDs in `sections.ts`
4. **File Location**: Place files in appropriate category (`user/` or `contributor/`)

### Navigation

1. **Logical Order**: Order pages logically (getting started → guides → reference)
2. **Clear Titles**: Use clear, descriptive titles in navigation
3. **Consistent Structure**: Maintain consistent structure across sections

### Diagrams

1. **Keep Simple**: Don't overcomplicate diagrams
2. **Clear Labels**: Use descriptive node and edge labels
3. **Test Rendering**: Verify diagrams render correctly in browser
4. **Documentation**: Include diagram descriptions in surrounding text

---

## Code Splitting

The build process uses aggressive code splitting for performance:

- **Mermaid**: Separate chunk for diagram library
- **Marked**: Separate chunk for markdown parser
- **KaTeX**: Separate chunk for math rendering
- **Content Data**: All markdown content in separate chunk
- **Pages**: Route-based splitting

This ensures:
- Fast initial page load
- Lazy loading of heavy libraries
- Better caching (content changes don't invalidate library chunks)

---

## Development Workflow

### Adding Documentation

1. **Create Markdown File**: Add new `.md` file in appropriate directory
2. **Update Navigation**: Add page to `sections.ts`
3. **Write Content**: Write markdown content with examples
4. **Test Locally**: Run `yarn dev` and verify
5. **Commit**: Commit with conventional commit message

### Updating Existing Pages

1. **Edit Markdown**: Modify the `.md` file
2. **Test Changes**: Verify in dev server
3. **Commit**: Commit with conventional commit message

### Reviewing Changes

1. **Build Locally**: Run `yarn build` to check for errors
2. **Preview Build**: Run `yarn preview` to see production build
3. **Check Navigation**: Verify navigation still works correctly
4. **Test Links**: Check all internal links work

---

## Examples

### Complete Page Example

**File:** `docs-site/src/data/content/user/guides/example.md`

```markdown
# Example Guide

This is an example documentation page.

## Overview

The example feature provides...

## Usage

\`\`\`yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: example
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
\`\`\`

## Architecture

\`\`\`mermaid
graph TB
    A[Start] --> B[Process]
    B --> C[End]
\`\`\`

## See Also

- [Installation](../getting-started/installation.md)
- [Configuration](../getting-started/configuration.md)
```

**Navigation Entry:** `src/data/sections.ts`

```typescript
{
  id: 'guides',
  title: 'Guides',
  pages: [
    // ... existing pages ...
    { id: 'example', title: 'Example', file: 'guides/example.md' },
  ],
}
```

---

## Summary

The documentation site provides:

- **Fast Development**: Hot module replacement, instant updates
- **Easy Content Management**: Markdown files, simple navigation structure
- **Rich Features**: Mermaid diagrams, syntax highlighting, math support
- **Professional Styling**: Warm, easy-on-the-eyes design
- **Performance**: Code splitting, lazy loading, optimized builds

For questions or issues, see the [Contributing Guide](../contributing/contributing-guide.md) or open an issue.

