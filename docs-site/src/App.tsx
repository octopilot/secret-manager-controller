import { Component, createSignal, onMount, Show } from 'solid-js';
import Navigation from './components/layout/Navigation';
import ContentArea from './components/layout/ContentArea';

type DocCategory = 'user' | 'contributor';

const App: Component = () => {
  const [currentCategory, setCurrentCategory] = createSignal<DocCategory>('user');
  const [currentSection, setCurrentSection] = createSignal<string | null>('getting-started');
  const [currentPage, setCurrentPage] = createSignal<string | null>('installation');

  // Handle hash-based routing
  onMount(() => {
    const handleHashChange = () => {
      const hash = window.location.hash;
      
      if (hash.startsWith('#/user/')) {
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
        // Default: show user getting started
        setCurrentCategory('user');
        setCurrentSection('getting-started');
        setCurrentPage('installation');
        // Use replaceState to avoid adding to history
        if (window.location.hash !== '#/user/getting-started/installation') {
          window.location.hash = '#/user/getting-started/installation';
        }
      }
    };

    // Initial load
    handleHashChange();
    
    // Listen for hash changes
    window.addEventListener('hashchange', handleHashChange);
    
    return () => {
      window.removeEventListener('hashchange', handleHashChange);
    };
  });

  return (
    <div class="min-h-screen bg-[#faf9f7] flex flex-col">
      <header class="bg-white border-b border-[#e5e3df] shadow-sm px-6 py-4 sticky top-0 z-10">
        <div class="max-w-7xl mx-auto flex items-center justify-between">
          <h1 class="text-2xl font-semibold text-[#2d3748] tracking-tight">
            Secret Manager Controller
          </h1>
          <nav class="flex gap-3">
            <button
              onClick={() => {
                setCurrentCategory('user');
                window.location.hash = '#/user/getting-started/installation';
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
            window.location.hash = `#/${category}/${section}${page ? `/${page}` : ''}`;
          }}
        />
        <ContentArea
          category={currentCategory()}
          section={currentSection()}
          page={currentPage()}
        />
      </div>
    </div>
  );
};

export default App;

