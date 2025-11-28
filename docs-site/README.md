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

## Project Structure

```
docs-site/
├── src/
│   ├── components/
│   │   ├── layout/        # Navigation, ContentArea
│   │   └── content/        # MarkdownRenderer
│   ├── data/
│   │   ├── sections.ts     # Documentation structure
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

