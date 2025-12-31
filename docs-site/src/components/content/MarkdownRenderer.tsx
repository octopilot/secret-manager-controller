import { Component, createEffect, createSignal, onCleanup } from 'solid-js';
import { marked } from 'marked';
import mermaid from 'mermaid';

interface MarkdownRendererProps {
  content: string;
}

const MarkdownRenderer: Component<MarkdownRendererProps> = (props) => {
  const [html, setHtml] = createSignal<string>('');
  let containerRef: HTMLDivElement | undefined;
  let headingTimeout: ReturnType<typeof setTimeout> | null = null;
  let mermaidTimeout: ReturnType<typeof setTimeout> | null = null;

  // Watch for content changes and re-render
  createEffect(() => {
    if (!props.content) {
      setHtml('');
      return;
    }

    // Clear any pending timeouts
    if (headingTimeout) {
      clearTimeout(headingTimeout);
      headingTimeout = null;
    }
    if (mermaidTimeout) {
      clearTimeout(mermaidTimeout);
      mermaidTimeout = null;
    }

    // Configure marked
    marked.setOptions({
      breaks: true,
      gfm: true,
    });

    // Render markdown to HTML
    const rendered = marked.parse(props.content);
    setHtml(rendered as string);
    
    // Add IDs to headings after DOM is updated
    headingTimeout = setTimeout(() => {
      const container = containerRef;
      if (container) {
        const headings = container.querySelectorAll('h1, h2, h3, h4, h5, h6');
        headings.forEach((heading) => {
          if (!heading.id) {
            const text = heading.textContent || '';
            const id = text
              .toLowerCase()
              .replace(/[^\w\s-]/g, '')
              .replace(/\s+/g, '-')
              .replace(/-+/g, '-')
              .trim();
            heading.id = id;
          }
        });
      }
      headingTimeout = null;
    }, 50);

    // Initialize Mermaid diagrams after a short delay to ensure DOM is ready
    mermaidTimeout = setTimeout(() => {
      const container = containerRef;
      if (container) {
        mermaid.initialize({ 
          startOnLoad: false, 
          theme: 'default',
          securityLevel: 'loose',
        });
        const mermaidElements = container.querySelectorAll('.language-mermaid');
        mermaidElements.forEach((el) => {
          const code = el.textContent || '';
          const id = `mermaid-${Math.random().toString(36).substr(2, 9)}`;
          mermaid.render(id, code).then((result) => {
            // Wrap the SVG in a div with mermaid class (matching GitHub's approach)
            const wrapper = document.createElement('div');
            wrapper.className = 'mermaid';
            wrapper.innerHTML = result.svg;
            
            // Set SVG background to transparent and remove dark background rectangles
            const svg = wrapper.querySelector('svg');
            if (svg) {
              // Set SVG background to transparent
              svg.style.backgroundColor = 'transparent';
              
              // Find and replace all dark background rectangles
              const allRects = svg.querySelectorAll('rect');
              allRects.forEach((rect) => {
                const fill = rect.getAttribute('fill') || '';
                const style = rect.getAttribute('style') || '';
                const computedStyle = window.getComputedStyle(rect);
                const actualFill = fill || computedStyle.fill || '';
                
                // Check if this is a background rectangle (large, at origin, dark color)
                const width = parseFloat(rect.getAttribute('width') || '0');
                const height = parseFloat(rect.getAttribute('height') || '0');
                const x = parseFloat(rect.getAttribute('x') || '0');
                const y = parseFloat(rect.getAttribute('y') || '0');
                
                // Dark colors to replace
                const darkColors = [
                  '#1f2328', '#0d1117', '#161b22', '#21262d', '#000000',
                  '#1a1a1a', '#2d2d2d', '#333333', '#1e1e1e', '#0a0a0a'
                ];
                
                const isDarkBackground = darkColors.some(color => 
                  actualFill.toLowerCase() === color.toLowerCase() ||
                  actualFill.toLowerCase() === color.toLowerCase().replace('#', '')
                );
                
                // If it's a large rectangle at the origin with dark fill, make it transparent
                if (isDarkBackground && width > 200 && height > 100 && 
                    Math.abs(x) < 10 && Math.abs(y) < 10) {
                  rect.setAttribute('fill', 'transparent');
                  rect.setAttribute('style', (style + '; fill: transparent !important;').replace(/fill:[^;]+;?/gi, ''));
                }
              });
            }
            
            // Replace the element
            const parent = el.parentNode;
            if (parent) {
              el.replaceWith(wrapper);
              
              // If parent is a pre element, add a class to mark it as containing mermaid
              if (parent.tagName === 'PRE') {
                parent.classList.add('contains-mermaid');
              }
            } else {
              el.replaceWith(wrapper);
            }
          }).catch((err) => {
            console.error('Mermaid rendering error:', err);
          });
        });

        // Add copy buttons to code blocks
        const codeBlocks = container.querySelectorAll('pre code');
        codeBlocks.forEach((codeEl) => {
          const pre = codeEl.parentElement as HTMLElement;
          // Skip if already has copy button
          if (pre.querySelector('.copy-code-button')) return;
          
          // Skip mermaid blocks (they're replaced with diagrams)
          if (codeEl.classList.contains('language-mermaid')) return;

          const code = codeEl.textContent || '';
          const copyButton = document.createElement('button');
          copyButton.className = 'copy-code-button absolute top-2 right-2 px-3 py-1.5 text-xs font-medium bg-[#5a6c5d] text-white rounded hover:bg-[#4a5a4c] transition-colors opacity-0 group-hover:opacity-100 focus:opacity-100';
          copyButton.setAttribute('aria-label', 'Copy code to clipboard');
          copyButton.textContent = 'Copy';
          
          copyButton.addEventListener('click', async () => {
            try {
              await navigator.clipboard.writeText(code);
              const originalText = copyButton.textContent;
              copyButton.textContent = 'Copied!';
              copyButton.classList.add('bg-[#2d4a2f]');
              const resetTimeout = setTimeout(() => {
                copyButton.textContent = originalText;
                copyButton.classList.remove('bg-[#2d4a2f]');
              }, 2000);
              // Store timeout ID on button for cleanup if needed
              (copyButton as any)._resetTimeout = resetTimeout;
            } catch (err) {
              console.error('Failed to copy:', err);
              copyButton.textContent = 'Failed';
              const resetTimeout = setTimeout(() => {
                copyButton.textContent = 'Copy';
              }, 2000);
              (copyButton as any)._resetTimeout = resetTimeout;
            }
          });

          // Make pre relative and add group class for hover
          pre.classList.add('relative', 'group');
          pre.appendChild(copyButton);
        });
      }
    }, 100);
  });

  return (
    <div
      ref={containerRef}
      class="markdown-content prose prose-lg max-w-none prose-headings:text-[#2d3748] prose-headings:font-semibold prose-h1:text-4xl prose-h1:mb-6 prose-h1:mt-0 prose-h1:border-b prose-h1:border-[#e5e3df] prose-h1:pb-3 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-4 prose-h2:text-[#374151] prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-3 prose-h3:text-[#4a5568] prose-p:text-[#4a5568] prose-p:leading-7 prose-p:mb-4 prose-a:text-[#5a6c5d] prose-a:no-underline prose-a:font-medium hover:prose-a:underline prose-strong:text-[#2d3748] prose-strong:font-semibold prose-code:text-[#c05621] prose-code:bg-[#f7f6f4] prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-sm prose-code:font-mono prose-pre:bg-[#0d1117] prose-pre:border prose-pre:border-[#00ff4120] prose-pre:rounded-lg prose-pre:shadow-lg prose-pre:max-w-[80ch] prose-pre:whitespace-pre-wrap prose-pre:break-words prose-pre:code:text-[#00ff41] prose-pre:code:bg-transparent prose-pre:code:p-0 prose-blockquote:border-l-4 prose-blockquote:border-[#5a6c5d] prose-blockquote:pl-4 prose-blockquote:italic prose-blockquote:text-[#6b7280] prose-ul:list-disc prose-ul:pl-6 prose-ul:my-4 prose-ol:list-decimal prose-ol:pl-6 prose-ol:my-4 prose-li:text-[#4a5568] prose-li:my-2 prose-li:leading-7 prose-hr:border-[#e5e3df] prose-table:border-collapse prose-th:bg-[#f7f6f4] prose-th:border prose-th:border-[#e5e3df] prose-th:px-4 prose-th:py-2 prose-th:text-left prose-th:text-[#2d3748] prose-th:font-semibold prose-td:border prose-td:border-[#e5e3df] prose-td:px-4 prose-td:py-2 prose-td:text-[#4a5568]"
      innerHTML={html()}
    />
  );
};

export default MarkdownRenderer;

