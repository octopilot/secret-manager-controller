import { Component, createEffect, createSignal } from 'solid-js';
import { marked } from 'marked';
import mermaid from 'mermaid';

interface MarkdownRendererProps {
  content: string;
}

const MarkdownRenderer: Component<MarkdownRendererProps> = (props) => {
  const [html, setHtml] = createSignal<string>('');
  let containerRef: HTMLDivElement | undefined;

  // Watch for content changes and re-render
  createEffect(() => {
    if (!props.content) {
      setHtml('');
      return;
    }

    // Configure marked
    marked.setOptions({
      breaks: true,
      gfm: true,
    });

    // Render markdown to HTML
    const rendered = marked.parse(props.content);
    setHtml(rendered as string);

    // Initialize Mermaid diagrams after a short delay to ensure DOM is ready
    setTimeout(() => {
      if (containerRef) {
        mermaid.initialize({ 
          startOnLoad: false, 
          theme: 'default',
          securityLevel: 'loose',
        });
        const mermaidElements = containerRef.querySelectorAll('.language-mermaid');
        mermaidElements.forEach((el) => {
          const code = el.textContent || '';
          const id = `mermaid-${Math.random().toString(36).substr(2, 9)}`;
          mermaid.render(id, code).then((result) => {
            el.outerHTML = result.svg;
          }).catch((err) => {
            console.error('Mermaid rendering error:', err);
          });
        });
      }
    }, 100);
  });

  return (
    <div
      ref={containerRef}
      class="prose prose-lg max-w-none prose-headings:text-[#2d3748] prose-headings:font-semibold prose-h1:text-4xl prose-h1:mb-6 prose-h1:mt-0 prose-h1:border-b prose-h1:border-[#e5e3df] prose-h1:pb-3 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-4 prose-h2:text-[#374151] prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-3 prose-h3:text-[#4a5568] prose-p:text-[#4a5568] prose-p:leading-7 prose-p:mb-4 prose-a:text-[#5a6c5d] prose-a:no-underline prose-a:font-medium hover:prose-a:underline prose-strong:text-[#2d3748] prose-strong:font-semibold prose-code:text-[#c05621] prose-code:bg-[#f7f6f4] prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:font-mono prose-pre:bg-[#0d1117] prose-pre:border prose-pre:border-[#00ff4120] prose-pre:rounded-lg prose-pre:shadow-lg prose-pre:max-w-[80ch] prose-pre:whitespace-pre-wrap prose-pre:break-words prose-pre:code:text-[#00ff41] prose-pre:code:bg-transparent prose-pre:code:p-0 prose-blockquote:border-l-4 prose-blockquote:border-[#5a6c5d] prose-blockquote:pl-4 prose-blockquote:italic prose-blockquote:text-[#6b7280] prose-ul:list-disc prose-ul:pl-6 prose-ul:my-4 prose-ol:list-decimal prose-ol:pl-6 prose-ol:my-4 prose-li:text-[#4a5568] prose-li:my-2 prose-li:leading-7 prose-hr:border-[#e5e3df] prose-table:border-collapse prose-th:bg-[#f7f6f4] prose-th:border prose-th:border-[#e5e3df] prose-th:px-4 prose-th:py-2 prose-th:text-left prose-th:text-[#2d3748] prose-th:font-semibold prose-td:border prose-td:border-[#e5e3df] prose-td:px-4 prose-td:py-2 prose-td:text-[#4a5568]"
      innerHTML={html()}
    />
  );
};

export default MarkdownRenderer;

