import { Component, createSignal, createEffect, Show, For, onMount, onCleanup } from 'solid-js';
import MiniSearch from 'minisearch';
import searchIndexData from '../../data/search-index.json?raw';

interface SearchResult {
  id: string;
  category: 'user' | 'contributor';
  section: string;
  sectionTitle: string;
  page: string;
  title: string;
  url: string;
  score?: number;
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
  const [isLoading, setIsLoading] = createSignal<boolean>(true);
  let searchInputRef: HTMLInputElement | undefined;

  // Initialize search index
  createEffect(() => {
    if (!searchIndex() && props.isOpen) {
      try {
        // searchIndexData from ?raw import is a string
        // MiniSearch.loadJSON expects a JSON string (not a parsed object)
        const index = MiniSearch.loadJSON<SearchResult>(searchIndexData, {
          fields: ['title', 'content', 'sectionTitle'],
          storeFields: ['id', 'category', 'section', 'sectionTitle', 'page', 'title', 'url'],
        });
        
        setSearchIndex(index);
        setIsLoading(false);
      } catch (err) {
        console.error('Failed to load search index:', err);
        setIsLoading(false);
      }
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

    try {
      const searchResults = searchIndex()!.search(q, {
        boost: { title: 3, sectionTitle: 2, content: 1 },
        fuzzy: 0.2,
        prefix: true,
      }) as SearchResult[];

      setResults(searchResults.slice(0, 10)); // Limit to 10 results
      setSelectedIndex(0);
    } catch (err) {
      console.error('Search error:', err);
      setResults([]);
    }
  });

  // Handle keyboard navigation
  const handleKeyDown = (e: KeyboardEvent) => {
    if (!props.isOpen) return;

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
      e.preventDefault();
      props.onClose();
    }
  };

  // Set up keyboard listeners
  onMount(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onCleanup(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });

  // Navigate to result
  const navigateToResult = (result: SearchResult) => {
    props.onNavigate(result.category, result.section, result.page);
    props.onClose();
  };

  // Reset query when modal closes and focus input when modal opens
  createEffect(() => {
    if (!props.isOpen) {
      setQuery('');
      setResults([]);
      setSelectedIndex(0);
    } else {
      // Focus the input when modal opens
      // Use setTimeout to ensure the DOM is fully rendered
      const focusTimeout = setTimeout(() => {
        searchInputRef?.focus();
      }, 0);
      
      // Cleanup function for this effect
      return () => {
        clearTimeout(focusTimeout);
      };
    }
  });

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-start justify-center pt-20"
        onClick={props.onClose}
      >
        <div
          class="bg-white rounded-lg shadow-xl w-full max-w-2xl mx-4 max-h-[80vh] flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Search Input */}
          <div class="p-4 border-b border-[#e5e3df]">
            <div class="relative">
              <svg
                class="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-[#6b7280]"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
              <input
                ref={searchInputRef}
                type="text"
                placeholder="Search documentation..."
                value={query()}
                onInput={(e) => setQuery(e.currentTarget.value)}
                class="w-full pl-10 pr-4 py-2 border border-[#e5e3df] rounded-lg focus:outline-none focus:ring-2 focus:ring-[#5a6c5d] focus:border-transparent"
              />
            </div>
          </div>

          {/* Results */}
          <div class="flex-1 overflow-y-auto custom-scrollbar">
            <Show
              when={!isLoading()}
              fallback={
                <div class="p-8 text-center text-[#6b7280]">
                  <div class="animate-spin rounded-full h-8 w-8 border-2 border-[#e5e3df] border-t-[#5a6c5d] mx-auto mb-4"></div>
                  Loading search index...
                </div>
              }
            >
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
                      class={`w-full text-left px-4 py-3 hover:bg-[#f7f6f4] transition-colors border-b border-[#e5e3df] last:border-b-0 ${
                        index() === selectedIndex() ? 'bg-[#e8f0e9]' : ''
                      }`}
                    >
                      <div class="flex items-start justify-between">
                        <div class="flex-1 min-w-0">
                          <div class="font-semibold text-[#2d3748] mb-1 truncate">
                            {result.title}
                          </div>
                          <div class="text-sm text-[#6b7280] truncate">
                            {result.sectionTitle}
                            {result.sectionTitle && ' • '}
                            {result.category === 'user' ? 'User' : 'Contributor'} Docs
                          </div>
                        </div>
                        <div class="ml-4 flex-shrink-0">
                          <span
                            class={`inline-flex items-center px-2 py-1 rounded text-xs font-medium ${
                              result.category === 'user'
                                ? 'bg-[#e8f0e9] text-[#2d4a2f]'
                                : 'bg-[#f0f4f8] text-[#1e3a5f]'
                            }`}
                          >
                            {result.category === 'user' ? 'U' : 'C'}
                          </span>
                        </div>
                      </div>
                    </button>
                  )}
                </For>
              </Show>
            </Show>
          </div>

          {/* Footer */}
          <div class="p-3 border-t border-[#e5e3df] bg-[#faf9f7]">
            <div class="flex items-center justify-center gap-4 text-xs text-[#6b7280]">
              <div class="flex items-center gap-1">
                <kbd class="px-1.5 py-0.5 bg-white border border-[#e5e3df] rounded text-xs">↑</kbd>
                <kbd class="px-1.5 py-0.5 bg-white border border-[#e5e3df] rounded text-xs">↓</kbd>
                <span>Navigate</span>
              </div>
              <div class="flex items-center gap-1">
                <kbd class="px-1.5 py-0.5 bg-white border border-[#e5e3df] rounded text-xs">Enter</kbd>
                <span>Select</span>
              </div>
              <div class="flex items-center gap-1">
                <kbd class="px-1.5 py-0.5 bg-white border border-[#e5e3df] rounded text-xs">Esc</kbd>
                <span>Close</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default SearchModal;

