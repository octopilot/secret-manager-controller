import { Component, createSignal, createEffect, Show } from 'solid-js';
import { DocCategory } from '../../data/sections';
import MarkdownRenderer from '../content/MarkdownRenderer';

interface ContentAreaProps {
  category: DocCategory;
  section: string | null;
  page: string | null;
}

const ContentArea: Component<ContentAreaProps> = (props) => {
  const [content, setContent] = createSignal<string>('');
  const [loading, setLoading] = createSignal<boolean>(false);
  const [error, setError] = createSignal<string | null>(null);

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
    if (!props.section || !props.page) {
      setContent('# Welcome\n\nSelect a page from the navigation to get started.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Construct the path to the markdown file
      const filePath = `../../data/content/${props.category}/${props.section}/${props.page}.md`;
      
      // Try to find the module
      const module = contentModules[filePath];
      
      if (module) {
        const text = await module();
        setContent(text as string);
      } else {
        // Placeholder content
        setContent(`# ${props.page.replace(/-/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}\n\nContent for this page is coming soon.\n\n**Category:** ${props.category}\n**Section:** ${props.section}\n**Page:** ${props.page}\n\nThis page will be populated with documentation content.`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load content');
      // Fallback placeholder
      setContent(`# ${props.page.replace(/-/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}\n\nContent for this page is coming soon.\n\n**Category:** ${props.category}\n**Section:** ${props.section}\n**Page:** ${props.page}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <main class="flex-1 overflow-y-auto bg-white custom-scrollbar">
      <div class="max-w-4xl mx-auto px-8 py-10">
        <Show when={loading()}>
          <div class="text-center py-16">
            <div class="animate-spin rounded-full h-12 w-12 border-2 border-[#e5e3df] border-t-[#5a6c5d] mx-auto mb-4"></div>
            <p class="text-[#6b7280]">Loading content...</p>
          </div>
        </Show>
        
        <Show when={!loading() && error()}>
          <div class="bg-[#fef2f2] border border-[#fecaca] rounded-lg p-4 mb-6">
            <p class="text-[#991b1b]">{error()}</p>
          </div>
        </Show>

        <Show when={!loading() && !error()}>
          <MarkdownRenderer content={content()} />
        </Show>
      </div>
    </main>
  );
};

export default ContentArea;

