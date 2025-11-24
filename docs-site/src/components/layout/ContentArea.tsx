import { Component, createSignal, createEffect, Show, createMemo } from 'solid-js';
import { DocCategory, userSections, contributorSections } from '../../data/sections';
import MarkdownRenderer from '../content/MarkdownRenderer';
import PageNavigation from './PageNavigation';

interface ContentAreaProps {
  category: DocCategory;
  section: string | null;
  page: string | null;
  onContentChange?: (content: string) => void;
  onNavigate: (category: DocCategory, section: string | null, page: string | null) => void;
}

const ContentArea: Component<ContentAreaProps> = (props) => {
  const [content, setContent] = createSignal<string>('');
  const [loading, setLoading] = createSignal<boolean>(false);
  const [error, setError] = createSignal<string | null>(null);

  // Calculate reading time (average reading speed: 200 words per minute)
  const readingTime = createMemo(() => {
    const text = content();
    if (!text) return 0;
    const words = text.split(/\s+/).filter(word => word.length > 0).length;
    const minutes = Math.ceil(words / 200);
    return minutes;
  });

  // Use Vite's glob import to load markdown files
  const contentModules = import.meta.glob('../../data/content/**/*.md', { 
    eager: false,
    as: 'raw' 
  });

  // Watch for prop changes and reload content
  createEffect(() => {
    loadContent();
  });

  const loadContent = async () => {
    // Handle landing page (index)
    if (props.page === 'index' && !props.section) {
      setLoading(true);
      setError(null);
      try {
        const filePath = `../../data/content/${props.category}/index.md`;
        const module = contentModules[filePath];
        if (module) {
          const text = await module();
          setContent(text as string);
          props.onContentChange?.(text as string);
        } else {
          const placeholder = '# Welcome\n\nSelect a page from the navigation to get started.';
          setContent(placeholder);
          props.onContentChange?.(placeholder);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load content');
        const placeholder = '# Welcome\n\nSelect a page from the navigation to get started.';
        setContent(placeholder);
        props.onContentChange?.(placeholder);
      } finally {
        setLoading(false);
      }
      return;
    }

    if (!props.section || !props.page) {
      const placeholder = '# Welcome\n\nSelect a page from the navigation to get started.';
      setContent(placeholder);
      props.onContentChange?.(placeholder);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Look up the page definition to get the file path
      const sections = props.category === 'user' ? userSections : contributorSections;
      const section = sections.find(s => s.id === props.section);
      const pageDef = section?.pages.find(p => p.id === props.page);
      
      // Use the file path from the page definition, or construct it as fallback
      const filePath = pageDef 
        ? `../../data/content/${props.category}/${pageDef.file}`
        : `../../data/content/${props.category}/${props.section}/${props.page}.md`;
      
      // Try to find the module
      const module = contentModules[filePath];
      
      if (module) {
        const text = await module();
        setContent(text as string);
        props.onContentChange?.(text as string);
      } else {
        // Placeholder content
        const placeholder = `# ${props.page.replace(/-/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}\n\nContent for this page is coming soon.\n\n**Category:** ${props.category}\n**Section:** ${props.section}\n**Page:** ${props.page}\n\nThis page will be populated with documentation content.`;
        setContent(placeholder);
        props.onContentChange?.(placeholder);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load content');
      // Fallback placeholder
      const placeholder = `# ${props.page.replace(/-/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}\n\nContent for this page is coming soon.\n\n**Category:** ${props.category}\n**Section:** ${props.section}\n**Page:** ${props.page}`;
      setContent(placeholder);
      props.onContentChange?.(placeholder);
    } finally {
      setLoading(false);
    }
  };

  return (
    <main id="main-content" class="flex-1 overflow-y-auto bg-white custom-scrollbar" role="main">
      <div class="max-w-4xl mx-auto px-8 py-10">
          <Show when={loading()}>
            <div class="text-center py-16" role="status" aria-live="polite">
              <div class="animate-spin rounded-full h-12 w-12 border-2 border-[#e5e3df] border-t-[#5a6c5d] mx-auto mb-4" aria-hidden="true"></div>
              <p class="text-[#6b7280]">Loading content...</p>
            </div>
          </Show>
          
          <Show when={!loading() && error()}>
            <div class="bg-[#fef2f2] border border-[#fecaca] rounded-lg p-4 mb-6" role="alert">
              <p class="text-[#991b1b]">{error()}</p>
            </div>
          </Show>

        <Show when={!loading() && !error()}>
          <div>
            {/* Reading time */}
            <Show when={readingTime() > 0 && props.page !== 'index'}>
              <div class="mb-6 text-sm text-[#6b7280] flex items-center gap-2">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span>{readingTime()} {readingTime() === 1 ? 'minute' : 'minutes'} read</span>
              </div>
            </Show>
            <MarkdownRenderer content={content()} />
            <PageNavigation
              category={props.category}
              section={props.section}
              page={props.page}
              onNavigate={props.onNavigate}
            />
          </div>
        </Show>
      </div>
    </main>
  );
};

export default ContentArea;

