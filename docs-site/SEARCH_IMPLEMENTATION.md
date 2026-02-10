# Full-Text Search Implementation Guide

## Recommendation: MiniSearch

**MiniSearch** is the recommended solution for client-side full-text search in the documentation site.

### Why MiniSearch?

1. **Fast**: In-memory search with efficient indexing
2. **TypeScript Support**: Full TypeScript types included
3. **Small Bundle Size**: ~20KB minified + gzipped
4. **Build-Time Indexing**: Can build search index at build time
5. **Flexible**: Supports fuzzy search, filtering, field boosting
6. **No Backend Required**: Works entirely client-side
7. **Active Maintenance**: Well-maintained and modern

### Alternatives Considered

- **FlexSearch**: Faster but larger bundle size (~40KB), more complex API
- **Lunr.js**: Mature but older API, larger bundle size
- **Fuse.js**: Simple but slower, no indexing (searches all content each time)
- **Pagefind**: Designed for static sites, not ideal for CSR

## Implementation Approach

### Option 1: Build-Time Indexing (Recommended)

Build the search index at build time and load it client-side.

**Pros:**
- Fast initial search (index pre-built)
- Smaller runtime bundle (index is separate)
- Better performance for large content

**Cons:**
- Index must be rebuilt when content changes
- Slightly larger initial bundle (index file)

### Option 2: Runtime Indexing

Build the search index when the app loads.

**Pros:**
- Always up-to-date (no rebuild needed)
- Smaller initial bundle

**Cons:**
- Slower initial load (must index all content)
- Indexing happens on every page load

**Recommendation:** Use **Option 1 (Build-Time Indexing)** for better performance.

## Implementation Steps

### Step 1: Install MiniSearch

```bash
cd docs-site
yarn add minisearch
```

### Step 2: Create Search Index Builder

Create `scripts/build-search-index.ts`:

```typescript
import MiniSearch from 'minisearch';
import { readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { glob } from 'glob';
import { userSections, contributorSections } from '../src/data/sections';

interface SearchDocument {
  id: string;
  category: 'user' | 'contributor';
  section: string;
  sectionTitle: string;
  page: string;
  title: string;
  content: string;
  url: string;
}

async function buildSearchIndex() {
  const documents: SearchDocument[] = [];
  
  // Process user documentation
  for (const section of userSections) {
    for (const page of section.pages) {
      const filePath = join(__dirname, '../src/data/content/user', page.file);
      try {
        const content = readFileSync(filePath, 'utf-8');
        // Strip markdown formatting for search (keep text only)
        const plainText = content
          .replace(/^#+\s+/gm, '') // Remove headers
          .replace(/\[([^\]]+)\]\([^\)]+\)/g, '$1') // Convert links to text
          .replace(/`([^`]+)`/g, '$1') // Remove code backticks
          .replace(/\*\*([^\*]+)\*\*/g, '$1') // Remove bold
          .replace(/\*([^\*]+)\*/g, '$1') // Remove italic
          .replace(/```[\s\S]*?```/g, '') // Remove code blocks
          .trim();
        
        documents.push({
          id: `user-${section.id}-${page.id}`,
          category: 'user',
          section: section.id,
          sectionTitle: section.title,
          page: page.id,
          title: page.title,
          content: plainText,
          url: `#/user/${section.id}/${page.id}`,
        });
      } catch (err) {
        console.warn(`Failed to read ${filePath}:`, err);
      }
    }
  }
  
  // Process contributor documentation
  for (const section of contributorSections) {
    for (const page of section.pages) {
      const filePath = join(__dirname, '../src/data/content/contributor', page.file);
      try {
        const content = readFileSync(filePath, 'utf-8');
        const plainText = content
          .replace(/^#+\s+/gm, '')
          .replace(/\[([^\]]+)\]\([^\)]+\)/g, '$1')
          .replace(/`([^`]+)`/g, '$1')
          .replace(/\*\*([^\*]+)\*\*/g, '$1')
          .replace(/\*([^\*]+)\*/g, '$1')
          .replace(/```[\s\S]*?```/g, '')
          .trim();
        
        documents.push({
          id: `contributor-${section.id}-${page.id}`,
          category: 'contributor',
          section: section.id,
          sectionTitle: section.title,
          page: page.id,
          title: page.title,
          content: plainText,
          url: `#/contributor/${section.id}/${page.id}`,
        });
      } catch (err) {
        console.warn(`Failed to read ${filePath}:`, err);
      }
    }
  }
  
  // Create search index
  const searchIndex = new MiniSearch<SearchDocument>({
    fields: ['title', 'content', 'sectionTitle'], // Fields to index
    storeFields: ['id', 'category', 'section', 'sectionTitle', 'page', 'title', 'url'], // Fields to return
    searchOptions: {
      boost: { title: 3, sectionTitle: 2, content: 1 }, // Boost title matches
      fuzzy: 0.2, // Enable fuzzy matching
      prefix: true, // Match prefixes
    },
  });
  
  // Add all documents to index
  searchIndex.addAll(documents);
  
  // Export index as JSON
  const indexData = searchIndex.toJSON();
  const outputPath = join(__dirname, '../src/data/search-index.json');
  writeFileSync(outputPath, JSON.stringify(indexData), 'utf-8');
  
  console.log(`✅ Search index built: ${documents.length} documents indexed`);
  console.log(`   Output: ${outputPath}`);
}

buildSearchIndex().catch(console.error);
```

### Step 3: Add Build Script

Update `package.json`:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "yarn build:search-index && vite build",
    "build:search-index": "tsx scripts/build-search-index.ts",
    "preview": "vite preview"
  },
  "devDependencies": {
    "tsx": "^4.7.0",
    "glob": "^10.3.10"
  }
}
```

### Step 4: Create Search Component

Create `src/components/search/SearchModal.tsx`:

```typescript
import { Component, createSignal, createEffect, Show, For } from 'solid-js';
import MiniSearch from 'minisearch';
import { userSections, contributorSections } from '../../data/sections';
import searchIndexData from '../../data/search-index.json?raw';

interface SearchResult {
  id: string;
  category: 'user' | 'contributor';
  section: string;
  sectionTitle: string;
  page: string;
  title: string;
  url: string;
  score: number;
}

interface SearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onNavigate: (category: 'user' | 'contributor', section: string, page: string) => void;
}

const SearchModal: Component<SearchModalProps> = (props) => {
  const [query, setQuery] = createSignal<string>('');
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [searchIndex, setSearchIndex] = createSignal<MiniSearch<SearchResult> | null>(null);
  const [selectedIndex, setSelectedIndex] = createSignal<number>(0);

  // Initialize search index
  createEffect(() => {
    if (!searchIndex()) {
      const index = MiniSearch.loadJSON<SearchResult>(JSON.parse(searchIndexData), {
        fields: ['title', 'content', 'sectionTitle'],
        storeFields: ['id', 'category', 'section', 'sectionTitle', 'page', 'title', 'url'],
        searchOptions: {
          boost: { title: 3, sectionTitle: 2, content: 1 },
          fuzzy: 0.2,
          prefix: true,
        },
      });
      setSearchIndex(index);
    }
  });

  // Perform search
  createEffect(() => {
    const q = query().trim();
    if (!q || !searchIndex()) {
      setResults([]);
      setSelectedIndex(0);
      return;
    }

    const searchResults = searchIndex()!.search(q, {
      fuzzy: 0.2,
      prefix: true,
    }) as SearchResult[];

    setResults(searchResults.slice(0, 10)); // Limit to 10 results
    setSelectedIndex(0);
  });

  // Handle keyboard navigation
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, results().length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter' && results().length > 0) {
      e.preventDefault();
      const selected = results()[selectedIndex()];
      if (selected) {
        props.onNavigate(selected.category, selected.section, selected.page);
        props.onClose();
      }
    } else if (e.key === 'Escape') {
      props.onClose();
    }
  };

  // Navigate to result
  const navigateToResult = (result: SearchResult) => {
    props.onNavigate(result.category, result.section, result.page);
    props.onClose();
  };

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-start justify-center pt-20"
        onClick={props.onClose}
      >
        <div
          class="bg-white rounded-lg shadow-xl w-full max-w-2xl mx-4"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Search Input */}
          <div class="p-4 border-b border-[#e5e3df]">
            <input
              type="text"
              placeholder="Search documentation..."
              value={query()}
              onInput={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={handleKeyDown}
              autofocus
              class="w-full px-4 py-2 border border-[#e5e3df] rounded-lg focus:outline-none focus:ring-2 focus:ring-[#5a6c5d] focus:border-transparent"
            />
          </div>

          {/* Results */}
          <div class="max-h-96 overflow-y-auto">
            <Show
              when={query().trim() && results().length > 0}
              fallback={
                <div class="p-8 text-center text-[#6b7280]">
                  {query().trim() ? 'No results found' : 'Start typing to search...'}
                </div>
              }
            >
              <For each={results()}>
                {(result, index) => (
                  <button
                    onClick={() => navigateToResult(result)}
                    class={`w-full text-left px-4 py-3 hover:bg-[#f7f6f4] transition-colors border-b border-[#e5e3df] ${
                      index() === selectedIndex() ? 'bg-[#e8f0e9]' : ''
                    }`}
                  >
                    <div class="flex items-start justify-between">
                      <div class="flex-1">
                        <div class="font-semibold text-[#2d3748] mb-1">{result.title}</div>
                        <div class="text-sm text-[#6b7280]">
                          {result.sectionTitle} • {result.category === 'user' ? 'User' : 'Contributor'} Docs
                        </div>
                      </div>
                      <div class="ml-4 text-xs text-[#9ca3af]">
                        {result.category === 'user' ? 'U' : 'C'}
                      </div>
                    </div>
                  </button>
                )}
              </For>
            </Show>
          </div>

          {/* Footer */}
          <div class="p-3 border-t border-[#e5e3df] text-xs text-[#6b7280] text-center">
            <kbd class="px-2 py-1 bg-[#f7f6f4] rounded">↑↓</kbd> Navigate{' '}
            <kbd class="px-2 py-1 bg-[#f7f6f4] rounded">Enter</kbd> Select{' '}
            <kbd class="px-2 py-1 bg-[#f7f6f4] rounded">Esc</kbd> Close
          </div>
        </div>
      </div>
    </Show>
  );
};

export default SearchModal;
```

### Step 5: Integrate Search into App

Update `src/App.tsx`:

```typescript
import SearchModal from './components/search/SearchModal';
import { createSignal } from 'solid-js';

// Add search state
const [isSearchOpen, setIsSearchOpen] = createSignal(false);

// Add keyboard shortcut (Cmd/Ctrl + K)
onMount(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      setIsSearchOpen(true);
    }
  };
  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
});

// Add search button to header
<button
  onClick={() => setIsSearchOpen(true)}
  class="px-4 py-2 rounded-lg text-sm font-medium bg-[#f1f0ed] text-[#4a5568] hover:bg-[#e5e3df] flex items-center gap-2"
>
  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
  </svg>
  Search
  <kbd class="px-1.5 py-0.5 text-xs bg-white border border-[#e5e3df] rounded">⌘K</kbd>
</button>

// Add SearchModal component
<SearchModal
  isOpen={isSearchOpen()}
  onClose={() => setIsSearchOpen(false)}
  onNavigate={(category, section, page) => {
    setCurrentCategory(category);
    setCurrentSection(section);
    setCurrentPage(page);
    window.location.hash = `#/${category}/${section}/${page}`;
  }}
/>
```

## Alternative: Runtime Indexing

If you prefer runtime indexing (simpler but slower):

```typescript
// In SearchModal component
const [searchIndex, setSearchIndex] = createSignal<MiniSearch<SearchResult> | null>(null);

// Load and index content at runtime
createEffect(async () => {
  if (!searchIndex()) {
    const index = new MiniSearch<SearchResult>({
      fields: ['title', 'content', 'sectionTitle'],
      storeFields: ['id', 'category', 'section', 'sectionTitle', 'page', 'title', 'url'],
    });
    
    // Load all markdown files
    const contentModules = import.meta.glob('../../data/content/**/*.md', { 
      eager: true,
      as: 'raw' 
    });
    
    const documents: SearchResult[] = [];
    // Process and add documents...
    
    index.addAll(documents);
    setSearchIndex(index);
  }
});
```

## Performance Considerations

### Index Size

For ~40 markdown files (~5,500 lines total):
- **Index size**: ~200-300KB (JSON)
- **Load time**: < 100ms
- **Search time**: < 10ms for typical queries

### Optimization Tips

1. **Lazy Load Index**: Load search index only when search is opened
2. **Limit Results**: Show top 10 results (already implemented)
3. **Debounce Search**: Debounce search input (optional, for very fast typers)
4. **Code Splitting**: Put search index in separate chunk

## Summary

**Recommended Solution:** MiniSearch with build-time indexing

**Benefits:**
- Fast search (< 10ms)
- Small bundle size (~20KB library + ~300KB index)
- TypeScript support
- No backend required
- Easy to implement

**Implementation:**
1. Install `minisearch`
2. Create build script to generate index
3. Create SearchModal component
4. Integrate into App with keyboard shortcut (⌘K)

This provides a fast, client-side search experience without requiring a database or backend service.

