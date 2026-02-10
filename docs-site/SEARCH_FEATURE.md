# Full-Text Search Feature

## Overview

The documentation site now includes full-text search powered by **MiniSearch**, a fast, client-side search engine.

## Features

- **Fast Search**: Sub-10ms search times for typical queries
- **Fuzzy Matching**: Finds results even with typos
- **Prefix Matching**: Matches partial words
- **Relevance Ranking**: Title matches ranked higher than content matches
- **Keyboard Navigation**: Full keyboard support (↑↓ arrows, Enter, Esc)
- **Keyboard Shortcut**: Press `⌘K` (Mac) or `Ctrl+K` (Windows/Linux) to open search

## Usage

### Opening Search

1. **Keyboard Shortcut**: Press `⌘K` (Mac) or `Ctrl+K` (Windows/Linux)
2. **Search Button**: Click the "Search" button in the header

### Searching

1. Type your query in the search box
2. Results appear instantly as you type
3. Use arrow keys (↑↓) to navigate results
4. Press `Enter` to select a result
5. Press `Esc` to close the search modal

### Search Results

- Results show page title, section, and category
- Limited to top 10 most relevant results
- Results are ranked by relevance (title matches > section matches > content matches)

## Implementation Details

### Build Process

The search index is built at build time:

```bash
yarn build:search-index
```

This script:
1. Reads all markdown files from `src/data/content/`
2. Strips markdown formatting to extract plain text
3. Creates a search index using MiniSearch
4. Saves the index to `src/data/search-index.json`

### Search Index

- **Location**: `src/data/search-index.json`
- **Size**: ~183 KB (for 42 documents)
- **Format**: JSON (MiniSearch serialized format)
- **Indexed Fields**: `title`, `content`, `sectionTitle`
- **Stored Fields**: `id`, `category`, `section`, `sectionTitle`, `page`, `title`, `url`

### Components

- **SearchModal** (`src/components/search/SearchModal.tsx`): Main search UI component
- **Build Script** (`scripts/build-search-index.ts`): Generates search index

### Integration

Search is integrated into `App.tsx`:
- Search button in header
- Keyboard shortcut handler
- Navigation integration

## Performance

- **Index Size**: ~183 KB (compressed in production)
- **Load Time**: < 100ms (lazy loaded when search opens)
- **Search Time**: < 10ms for typical queries
- **Bundle Impact**: ~20 KB (MiniSearch library)

## Updating Search Index

The search index is automatically rebuilt when you run:

```bash
yarn build
```

The build process runs `build:search-index` before building the site.

To manually rebuild the index:

```bash
yarn build:search-index
```

## Customization

### Search Options

Edit `scripts/build-search-index.ts` to customize:

- **Fuzzy Matching**: Adjust `fuzzy: 0.2` (0 = exact, 1 = very fuzzy)
- **Boost Values**: Change `boost: { title: 3, sectionTitle: 2, content: 1 }`
- **Result Limit**: Change `slice(0, 10)` in `SearchModal.tsx`

### Styling

Search modal styling is in `SearchModal.tsx` using Tailwind CSS classes. Colors match the site's design system.

## Troubleshooting

### Search Not Working

1. **Check Index Exists**: Verify `src/data/search-index.json` exists
2. **Rebuild Index**: Run `yarn build:search-index`
3. **Check Console**: Look for errors in browser console

### No Results Found

- Try different search terms
- Check for typos (fuzzy matching helps but may not catch everything)
- Search is case-insensitive

### Index Not Updating

- Make sure you run `yarn build:search-index` after adding new pages
- Check that new pages are added to `sections.ts`
- Verify markdown files are in the correct location

## Future Enhancements

Potential improvements:

- **Search History**: Remember recent searches
- **Search Analytics**: Track popular searches
- **Highlight Matches**: Highlight matching text in results
- **Search Suggestions**: Autocomplete suggestions
- **Advanced Filters**: Filter by category, section, etc.

## Dependencies

- **minisearch**: ^7.2.0 - Client-side search engine
- **tsx**: ^4.20.6 - TypeScript execution (for build script)
- **glob**: ^13.0.0 - File pattern matching (for build script)

## References

- [MiniSearch Documentation](https://lucaong.github.io/minisearch/)
- [Search Implementation Guide](./SEARCH_IMPLEMENTATION.md)

