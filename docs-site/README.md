# Secret Manager Controller Documentation Site

Documentation site for the Secret Manager Controller project, built with SolidJS, Vite, and Tailwind CSS.

## Development

```bash
# Install dependencies
yarn install

# Start development server
yarn dev

# Build for production
yarn build

# Preview production build
yarn preview
```

## GitHub org and docs URL

The docs base URL and GitHub repo links are defined in **`src/data/site-config.ts`**. When the GitHub organization or repo changes, update `GITHUB_ORG` (and optionally the repo name) there, then run `yarn build` so sitemap, RSS, and meta tags use the new URLs. Also update any hardcoded `github.com/...` or `*.github.io/...` links in `src/data/content/**/*.md` (e.g. [installation.md](src/data/content/user/getting-started/installation.md)).

## Project Structure

```
docs-site/
├── src/
│   ├── components/
│   │   ├── layout/        # Navigation, ContentArea
│   │   └── content/        # MarkdownRenderer
│   ├── data/
│   │   ├── sections.ts     # Documentation structure
│   │   ├── site-config.ts  # GitHub org and docs base URL (single source of truth)
│   │   └── content/        # Markdown content files
│   ├── App.tsx
│   └── index.tsx
├── package.json
├── vite.config.ts
└── tailwind.config.js
```

## Documentation Organization

Documentation is organized into two main categories:

- **User Documentation** - For end users
- **Contributor Documentation** - For developers

Content is stored in `src/data/content/` organized by category and section.

## Tech Stack

- **SolidJS** - Reactive UI framework
- **Vite** - Build tool
- **Tailwind CSS** - Styling
- **Marked** - Markdown parsing
- **Mermaid** - Diagram rendering

