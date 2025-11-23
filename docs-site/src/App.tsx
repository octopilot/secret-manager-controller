import { Component, createSignal, onMount, Show, onCleanup } from 'solid-js';
import Navigation from './components/layout/Navigation';
import ContentArea from './components/layout/ContentArea';
import TableOfContents from './components/layout/TableOfContents';
import SearchModal from './components/search/SearchModal';
import MetaTags from './components/seo/MetaTags';
import SkipLinks from './components/layout/SkipLinks';
import Breadcrumbs from './components/layout/Breadcrumbs';
import KeyboardShortcutsModal from './components/layout/KeyboardShortcutsModal';

type DocCategory = 'user' | 'contributor';

const App: Component = () => {
  const [currentCategory, setCurrentCategory] = createSignal<DocCategory>('user');
  const [currentSection, setCurrentSection] = createSignal<string | null>(null);
  const [currentPage, setCurrentPage] = createSignal<string | null>('index');
  const [isSearchOpen, setIsSearchOpen] = createSignal<boolean>(false);
  const [isShortcutsOpen, setIsShortcutsOpen] = createSignal<boolean>(false);
  const [content, setContent] = createSignal<string>('');

  // Handle hash-based routing
  onMount(() => {
    const handleHashChange = () => {
      const hash = window.location.hash;
      
      if (hash === '' || hash === '#' || hash === '#/') {
        // Landing page: show index
        setCurrentCategory('user');
        setCurrentSection(null);
        setCurrentPage('index');
        if (window.location.hash !== '#/') {
          window.history.replaceState(null, '', '#/');
        }
      } else if (hash.startsWith('#/user/')) {
        setCurrentCategory('user');
        const path = hash.replace('#/user/', '');
        const parts = path.split('/').filter(p => p);
        if (parts.length >= 2) {
          setCurrentSection(parts[0]);
          setCurrentPage(parts.slice(1).join('/'));
        } else if (parts.length === 1 && parts[0]) {
          setCurrentSection(parts[0]);
          setCurrentPage(null);
        }
      } else if (hash.startsWith('#/contributor/')) {
        setCurrentCategory('contributor');
        const path = hash.replace('#/contributor/', '');
        const parts = path.split('/').filter(p => p);
        if (parts.length >= 2) {
          setCurrentSection(parts[0]);
          setCurrentPage(parts.slice(1).join('/'));
        } else if (parts.length === 1 && parts[0]) {
          setCurrentSection(parts[0]);
          setCurrentPage(null);
        }
      } else {
        // Default: show landing page
        setCurrentCategory('user');
        setCurrentSection(null);
        setCurrentPage('index');
        // Use replaceState to avoid adding to history
        window.history.replaceState(null, '', '#/');
      }
    };

    // Initial load
    handleHashChange();
    
    // Listen for hash changes
    window.addEventListener('hashchange', handleHashChange);
    
    // Keyboard shortcuts
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger shortcuts when typing in inputs
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setIsSearchOpen(true);
      } else if (e.key === '?' && !isSearchOpen() && !isShortcutsOpen()) {
        e.preventDefault();
        setIsShortcutsOpen(true);
      } else if (e.key === 'Escape') {
        if (isSearchOpen()) {
          setIsSearchOpen(false);
        } else if (isShortcutsOpen()) {
          setIsShortcutsOpen(false);
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    
    return () => {
      window.removeEventListener('hashchange', handleHashChange);
      window.removeEventListener('keydown', handleKeyDown);
    };
  });

  return (
    <div class="min-h-screen bg-[#faf9f7] flex flex-col">
      <MetaTags
        category={currentCategory()}
        section={currentSection()}
        page={currentPage()}
      />
      <SkipLinks />
      <header class="bg-white border-b border-[#e5e3df] shadow-sm sticky top-0 z-10" role="banner">
        <div class="max-w-7xl mx-auto">
          {/* Top row: Title and navigation buttons */}
          <div class="px-6 py-4 flex items-center justify-between">
            <button
              onClick={() => {
                setCurrentCategory('user');
                setCurrentSection(null);
                setCurrentPage('index');
                window.location.hash = '#/';
              }}
              class="text-2xl font-semibold text-[#2d3748] tracking-tight hover:text-[#5a6c5d] transition-colors cursor-pointer"
              aria-label="Go to home page"
            >
              Secret Manager Controller
            </button>
            <nav class="flex gap-3 items-center" aria-label="Main navigation">
              <button
                onClick={() => setIsShortcutsOpen(true)}
                class="hidden md:flex px-3 py-2 rounded-lg text-sm font-medium bg-[#f1f0ed] text-[#4a5568] hover:bg-[#e5e3df] transition-colors items-center gap-2"
                aria-label="Show keyboard shortcuts"
                title="Keyboard shortcuts (?)"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
                <kbd class="px-1.5 py-0.5 text-xs bg-white border border-[#e5e3df] rounded font-mono">?</kbd>
              </button>
              <button
                onClick={() => setIsSearchOpen(true)}
                class="px-4 py-2 rounded-lg text-sm font-medium bg-[#f1f0ed] text-[#4a5568] hover:bg-[#e5e3df] transition-colors flex items-center gap-2"
                aria-label="Open search dialog"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <span class="hidden sm:inline">Search</span>
                <kbd class="px-1.5 py-0.5 text-xs bg-white border border-[#e5e3df] rounded font-mono">
                  {navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'}K
                </kbd>
              </button>
              <button
                onClick={() => {
                  setCurrentCategory('user');
                  setCurrentSection(null);
                  setCurrentPage('index');
                  window.location.hash = '#/';
                }}
                class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
                  currentCategory() === 'user'
                    ? 'bg-[#5a6c5d] text-white shadow-sm'
                    : 'bg-[#f1f0ed] text-[#4a5568] hover:bg-[#e5e3df]'
                }`}
              >
                User Docs
              </button>
              <button
                onClick={() => {
                  setCurrentCategory('contributor');
                  window.location.hash = '#/contributor/development/setup';
                }}
                class={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
                  currentCategory() === 'contributor'
                    ? 'bg-[#5a6c5d] text-white shadow-sm'
                    : 'bg-[#f1f0ed] text-[#4a5568] hover:bg-[#e5e3df]'
                }`}
              >
                Contributor Docs
              </button>
            </nav>
          </div>
          {/* Bottom row: Breadcrumbs */}
          <div class="px-6 py-2 border-t border-[#e5e3df] bg-[#faf9f7]">
            <Breadcrumbs
              category={currentCategory()}
              section={currentSection()}
              page={currentPage()}
              onNavigate={(category, section, page) => {
                setCurrentCategory(category);
                setCurrentSection(section);
                setCurrentPage(page);
                if (page === 'index' && !section) {
                  window.location.hash = '#/';
                } else {
                  window.location.hash = `#/${category}/${section}${page ? `/${page}` : ''}`;
                }
              }}
            />
          </div>
        </div>
      </header>

      <div class="flex-1 flex">
        <Navigation
          category={currentCategory()}
          currentSection={currentSection()}
          currentPage={currentPage()}
          onNavigate={(category, section, page) => {
            setCurrentCategory(category);
            setCurrentSection(section);
            setCurrentPage(page);
            if (page === 'index' && !section) {
              window.location.hash = '#/';
            } else {
              window.location.hash = `#/${category}/${section}${page ? `/${page}` : ''}`;
            }
          }}
        />
        <ContentArea
          category={currentCategory()}
          section={currentSection()}
          page={currentPage()}
          onContentChange={setContent}
          onNavigate={(category, section, page) => {
            setCurrentCategory(category);
            setCurrentSection(section);
            setCurrentPage(page);
            if (page === 'index' && !section) {
              window.location.hash = '#/';
            } else {
              window.location.hash = `#/${category}/${section}${page ? `/${page}` : ''}`;
            }
          }}
        />
        <TableOfContents content={content()} />
      </div>

      <SearchModal
        isOpen={isSearchOpen()}
        onClose={() => setIsSearchOpen(false)}
        onNavigate={(category, section, page) => {
          setCurrentCategory(category);
          setCurrentSection(section);
          setCurrentPage(page);
          if (page === 'index' && !section) {
            window.location.hash = '#/';
          } else {
            window.location.hash = `#/${category}/${section}${page ? `/${page}` : ''}`;
          }
        }}
      />
      <KeyboardShortcutsModal
        isOpen={isShortcutsOpen()}
        onClose={() => setIsShortcutsOpen(false)}
      />
    </div>
  );
};

export default App;

