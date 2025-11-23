import { Component, For } from 'solid-js';
import { DocCategory, DocSection, userSections, contributorSections } from '../../data/sections';

interface NavigationProps {
  category: DocCategory;
  currentSection: string | null;
  currentPage: string | null;
  onNavigate: (category: DocCategory, section: string, page: string | null) => void;
}

const Navigation: Component<NavigationProps> = (props) => {
  const sections = () => 
    props.category === 'user' ? userSections : contributorSections;

  return (
    <aside class="w-64 bg-white border-r border-[#e5e3df] overflow-y-auto custom-scrollbar sticky top-[73px] h-[calc(100vh-73px)]">
      <nav class="p-5">
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
                          onClick={() => props.onNavigate(props.category, section.id, page.id)}
                          class={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
                            isActive
                              ? 'bg-[#e8f0e9] text-[#2d4a2f] font-medium border-l-2 border-[#5a6c5d]'
                              : 'text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748]'
                          }`}
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
  );
};

export default Navigation;

