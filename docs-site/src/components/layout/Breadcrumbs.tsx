import { Component, For, Show } from 'solid-js';
import { DocCategory, userSections, contributorSections } from '../../data/sections';

interface BreadcrumbsProps {
  category: DocCategory;
  section: string | null;
  page: string | null;
  onNavigate: (category: DocCategory, section: string | null, page: string | null) => void;
}

const Breadcrumbs: Component<BreadcrumbsProps> = (props) => {
  const getBreadcrumbs = () => {
    const crumbs: Array<{ label: string; category?: DocCategory; section?: string | null; page?: string | null }> = [];
    
    // Home
    crumbs.push({ label: 'Home', category: 'user', section: null, page: 'index' });
    
    if (props.page === 'index' && !props.section) {
      return crumbs;
    }
    
    // Category
    const categoryLabel = props.category === 'user' ? 'User Docs' : 'Contributor Docs';
    crumbs.push({ label: categoryLabel, category: props.category, section: null, page: 'index' });
    
    if (!props.section) {
      return crumbs;
    }
    
    // Section
    const sections = props.category === 'user' ? userSections : contributorSections;
    const sectionObj = sections.find(s => s.id === props.section);
    if (sectionObj) {
      crumbs.push({ label: sectionObj.title, category: props.category, section: props.section, page: null });
    }
    
    // Page
    if (props.page && sectionObj) {
      const pageObj = sectionObj.pages.find(p => p.id === props.page);
      if (pageObj) {
        crumbs.push({ label: pageObj.title, category: props.category, section: props.section, page: props.page });
      }
    }
    
    return crumbs;
  };

  const breadcrumbs = () => getBreadcrumbs();

  return (
    <nav aria-label="Breadcrumb">
      <ol class="flex items-center space-x-2 text-sm text-[#6b7280]" itemscope itemtype="https://schema.org/BreadcrumbList">
        <For each={breadcrumbs()}>
          {(crumb, index) => (
            <li 
              class="flex items-center"
              itemprop="itemListElement" 
              itemscope 
              itemtype="https://schema.org/ListItem"
            >
              <Show when={index() < breadcrumbs().length - 1}>
                <button
                  onClick={() => {
                    if (crumb.category !== undefined && crumb.section !== undefined && crumb.page !== undefined) {
                      props.onNavigate(crumb.category, crumb.section, crumb.page);
                    }
                  }}
                  class="hover:text-[#2d3748] transition-colors"
                  itemprop="item"
                >
                  <span itemprop="name">{crumb.label}</span>
                </button>
                <meta itemprop="position" content={String(index() + 1)} />
                <span class="mx-2 text-[#d1d5db]">/</span>
              </Show>
              <Show when={index() === breadcrumbs().length - 1}>
                <span class="text-[#2d3748] font-medium" itemprop="name">{crumb.label}</span>
                <meta itemprop="position" content={String(index() + 1)} />
              </Show>
            </li>
          )}
        </For>
      </ol>
    </nav>
  );
};

export default Breadcrumbs;

