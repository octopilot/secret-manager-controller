import { Component, For, createSignal, Show } from 'solid-js';
import { DocCategory, DocSection, userSections, contributorSections } from '../../data/sections';

interface NavigationProps {
  category: DocCategory;
  currentSection: string | null;
  currentPage: string | null;
  onNavigate: (category: DocCategory, section: string | null, page: string | null) => void;
}

const Navigation: Component<NavigationProps> = (props) => {
  const [isMobileOpen, setIsMobileOpen] = createSignal<boolean>(false);
  const sections = () => 
    props.category === 'user' ? userSections : contributorSections;

  const isLandingPage = () => 
    props.currentPage === 'index' && !props.currentSection;

  return (
    <>
      {/* Mobile menu button */}
      <button
        onClick={() => setIsMobileOpen(!isMobileOpen())}
        class="lg:hidden fixed top-[112px] left-0 z-30 p-3 bg-white border-r border-b border-[#e5e3df] rounded-br-lg"
        aria-label="Toggle navigation menu"
        aria-expanded={isMobileOpen()}
      >
        <svg class="w-6 h-6 text-[#4a5568]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <Show when={isMobileOpen()} fallback={
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
          }>
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </Show>
        </svg>
      </button>

      {/* Mobile overlay */}
      <Show when={isMobileOpen()}>
        <div
          class="lg:hidden fixed inset-0 bg-black bg-opacity-50 z-20"
          onClick={() => setIsMobileOpen(false)}
        />
      </Show>

      {/* Navigation sidebar */}
      <aside
        class={`w-64 bg-white border-r border-[#e5e3df] overflow-y-auto custom-scrollbar h-[calc(100vh-112px)] z-30 transition-transform duration-300 ${
          isMobileOpen() ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'
        } fixed lg:sticky lg:top-[112px]`}
        id="navigation"
        role="complementary"
        aria-label="Documentation navigation"
      >
      <nav class="p-5" aria-label="Table of contents">
        {/* Landing page link */}
        <div class="mb-6">
        <button
          onClick={() => {
            props.onNavigate(props.category, null, 'index');
            setIsMobileOpen(false);
          }}
          class={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
            isLandingPage()
              ? 'bg-[#e8f0e9] text-[#2d4a2f] font-medium border-l-2 border-[#5a6c5d]'
              : 'text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748]'
          }`}
          aria-current={isLandingPage() ? 'page' : undefined}
        >
          Home
        </button>
        </div>
        <For each={sections()}>
          {(section: DocSection) => (
            <div class="mb-8">
              <h2 class="text-xs font-semibold text-[#6b7280] uppercase tracking-wider mb-3 px-2">
                {section.title}
              </h2>
              <ul class="space-y-0.5">
                <For each={section.pages}>
                  {(page) => {
                    const isActive = 
                      props.currentSection === section.id && 
                      props.currentPage === page.id;
                    
                    return (
                      <li>
                        <button
                          onClick={() => {
                            props.onNavigate(props.category, section.id, page.id);
                            setIsMobileOpen(false);
                          }}
                          class={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
                            isActive
                              ? 'bg-[#e8f0e9] text-[#2d4a2f] font-medium border-l-2 border-[#5a6c5d]'
                              : 'text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748]'
                          }`}
                          aria-current={isActive ? 'page' : undefined}
                          aria-label={`Navigate to ${page.title}`}
                        >
                          {page.title}
                        </button>
                      </li>
                    );
                  }}
                </For>
              </ul>
            </div>
          )}
        </For>
      </nav>
    </aside>
    </>
  );
};

export default Navigation;

