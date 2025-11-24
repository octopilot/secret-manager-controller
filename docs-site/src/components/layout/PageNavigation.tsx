import { Component, createMemo, Show } from 'solid-js';
import { DocCategory, DocSection, userSections, contributorSections } from '../../data/sections';

interface PageNavigationProps {
  category: DocCategory;
  section: string | null;
  page: string | null;
  onNavigate: (category: DocCategory, section: string | null, page: string | null) => void;
}

const PageNavigation: Component<PageNavigationProps> = (props) => {
  const navigation = createMemo(() => {
    const sections = props.category === 'user' ? userSections : contributorSections;
    
    // If on landing page, no navigation
    if (props.page === 'index' && !props.section) {
      return { prev: null, next: null };
    }

    // Flatten all pages with their section info
    const allPages: Array<{ section: DocSection; pageId: string; pageTitle: string }> = [];
    sections.forEach((section) => {
      section.pages.forEach((page) => {
        allPages.push({
          section,
          pageId: page.id,
          pageTitle: page.title,
        });
      });
    });

    // Find current page index
    const currentIndex = allPages.findIndex(
      (p) => p.section.id === props.section && p.pageId === props.page
    );

    if (currentIndex === -1) {
      return { prev: null, next: null };
    }

    const prev = currentIndex > 0 ? allPages[currentIndex - 1] : null;
    const next = currentIndex < allPages.length - 1 ? allPages[currentIndex + 1] : null;

    return { prev, next };
  });

  const nav = navigation();

  return (
    <Show when={nav.prev || nav.next}>
      <nav class="mt-12 pt-8 border-t border-[#e5e3df] flex items-center justify-between" aria-label="Page navigation">
        <Show when={nav.prev}>
          <button
            onClick={() => props.onNavigate(props.category, nav.prev!.section.id, nav.prev!.pageId)}
            class="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748] transition-colors group"
            aria-label={`Previous: ${nav.prev!.pageTitle}`}
          >
            <svg class="w-5 h-5 group-hover:-translate-x-1 transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
            <div class="text-left">
              <div class="text-xs text-[#6b7280] mb-0.5">Previous</div>
              <div class="font-semibold">{nav.prev!.pageTitle}</div>
            </div>
          </button>
        </Show>
        <Show when={!nav.prev}>
          <div></div>
        </Show>
        <Show when={nav.next}>
          <button
            onClick={() => props.onNavigate(props.category, nav.next!.section.id, nav.next!.pageId)}
            class="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748] transition-colors group ml-auto"
            aria-label={`Next: ${nav.next!.pageTitle}`}
          >
            <div class="text-right">
              <div class="text-xs text-[#6b7280] mb-0.5">Next</div>
              <div class="font-semibold">{nav.next!.pageTitle}</div>
            </div>
            <svg class="w-5 h-5 group-hover:translate-x-1 transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </button>
        </Show>
      </nav>
    </Show>
  );
};

export default PageNavigation;

