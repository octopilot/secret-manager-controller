import { Component, createSignal, createEffect, For, Show } from 'solid-js';

interface Heading {
  id: string;
  text: string;
  level: number;
}

interface TableOfContentsProps {
  content: string;
}

interface HeadingNode {
  heading: Heading;
  children: HeadingNode[];
}

const TableOfContents: Component<TableOfContentsProps> = (props) => {
  const [headings, setHeadings] = createSignal<Heading[]>([]);
  const [activeAnchor, setActiveAnchor] = createSignal<string | null>(null);

  // Pattern to detect timestamps (dates, times, ISO dates, etc.)
  const timestampPattern = /\d{4}-\d{2}-\d{2}|\d{1,2}\/\d{1,2}\/\d{2,4}|\d{1,2}:\d{2}(:\d{2})?|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/;

  // Extract headings from DOM (like DCops) - only H1 and H2, filter timestamps
  const extractHeadings = () => {
    // Look for headings in the markdown content area
    const markdownContent = document.querySelector('main .markdown-content');
    const container = markdownContent || document.querySelector('main');
    
    if (!container) {
      return;
    }
    
    // Only get H1 and H2 headings (matching original behavior)
    const headingElements = container.querySelectorAll('h1, h2');
    const extracted: Heading[] = [];
    
    headingElements.forEach((el) => {
      const text = el.textContent || '';
      
      // Filter out headings that look like timestamps
      if (timestampPattern.test(text) && text.length < 30) {
        return;
      }
      
      // Use existing ID or generate one
      const id = el.id || text.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').replace(/-+/g, '-').trim();
      if (id) {
        el.id = id;
        extracted.push({
          id,
          text,
          level: parseInt(el.tagName.charAt(1)),
        });
      }
    });
    
    if (extracted.length > 0) {
      setHeadings(extracted);
    }
  };

  // Extract headings when content changes (using MutationObserver like DCops)
  createEffect(() => {
    // Reset headings when content changes
    setHeadings([]);
    
    if (!props.content) {
      return;
    }
    
    let extractTimeout: ReturnType<typeof setTimeout> | null = null;
    let observer: MutationObserver | null = null;
    
    const scheduleExtraction = () => {
      if (extractTimeout) {
        clearTimeout(extractTimeout);
      }
      extractTimeout = setTimeout(() => {
        extractHeadings();
      }, 50);
    };
    
    // Use MutationObserver to watch for DOM changes
    const mainElement = document.querySelector('main');
    if (!mainElement) {
      // Retry mechanism for initial load
      let retries = 0;
      const maxRetries = 20;
      const retryInterval = setInterval(() => {
        retries++;
        const main = document.querySelector('main');
        if (main) {
          clearInterval(retryInterval);
          scheduleExtraction();
          
          // Set up observer once main exists
          observer = new MutationObserver(() => {
            scheduleExtraction();
          });
          observer.observe(main, {
            childList: true,
            subtree: true,
          });
        } else if (retries >= maxRetries) {
          clearInterval(retryInterval);
        }
      }, 100);
      
      return () => {
        clearInterval(retryInterval);
        if (extractTimeout) clearTimeout(extractTimeout);
        if (observer) observer.disconnect();
      };
    }

    // Extract headings immediately if content is already rendered
    scheduleExtraction();

    // Watch for changes to the main element's children
    observer = new MutationObserver(() => {
      scheduleExtraction();
    });

    observer.observe(mainElement, {
      childList: true,
      subtree: true,
    });

    // Fallback timeouts to catch different render timings
    const timeouts: ReturnType<typeof setTimeout>[] = [];
    [100, 300, 600].forEach((delay) => {
      const timeout = setTimeout(() => {
        extractHeadings();
      }, delay);
      timeouts.push(timeout);
    });

    return () => {
      if (observer) observer.disconnect();
      if (extractTimeout) clearTimeout(extractTimeout);
      timeouts.forEach(clearTimeout);
    };
  });

  // Track active heading on scroll (using DCops pattern - simple and reliable)
  createEffect(() => {
    const handleScroll = () => {
      const markdownContent = document.querySelector('main .markdown-content');
      const container = markdownContent || document.querySelector('main');
      
      if (!container) {
        return;
      }
      
      // Only check H1 and H2 headings (matching our heading extraction)
      const headingElements = container.querySelectorAll('h1, h2');
      let current = '';
      
      headingElements.forEach((el) => {
        // Ensure heading has an ID (generate if missing)
        if (!el.id) {
          const text = el.textContent || '';
          const id = text.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').replace(/-+/g, '-').trim();
          if (id) {
            el.id = id;
          }
        }
        
        const rect = el.getBoundingClientRect();
        // Use 100px threshold like DCops (accounts for sticky header)
        if (rect.top <= 100 && el.id) {
          current = el.id;
        }
      });
      
      setActiveAnchor(current || null);
    };

    window.addEventListener('scroll', handleScroll);
    handleScroll();
    
    return () => window.removeEventListener('scroll', handleScroll);
  });

  const scrollToHeading = (id: string) => {
    const element = document.getElementById(id);
    if (element) {
      const offset = 120;
      const elementPosition = element.getBoundingClientRect().top;
      const offsetPosition = elementPosition + window.pageYOffset - offset;
      window.scrollTo({
        top: offsetPosition,
        behavior: 'smooth',
      });
      setActiveAnchor(id);
    }
  };
    
  // Build a tree structure from headings (only H1 and H2)
  const buildHeadingTree = (): HeadingNode[] => {
    const allHeadings = headings();
    if (allHeadings.length === 0) return [];

    const rootNodes: HeadingNode[] = [];
    const stack: HeadingNode[] = [];

    allHeadings.forEach((heading) => {
      // Only process H1 and H2 (should already be filtered, but double-check)
      if (heading.level > 2) {
        return;
      }

      const node: HeadingNode = { heading, children: [] };

      // If this is an H1, it's always a root node
      if (heading.level === 1) {
        stack.length = 0;
        rootNodes.push(node);
        stack.push(node);
      } else {
        // For H2, find the most recent H1 as parent
        while (stack.length > 0 && stack[stack.length - 1].heading.level >= heading.level) {
          stack.pop();
        }

        if (stack.length === 0) {
          // No parent found - add as root
          rootNodes.push(node);
        } else {
          // Add as child of the top of the stack
          stack[stack.length - 1].children.push(node);
        }

        stack.push(node);
      }
    });

    return rootNodes;
  };

  // Render a heading node and its children
  const renderHeadingNode = (node: HeadingNode, depth: number = 0) => {
    const active = activeAnchor();
    const isActive = active !== null && active === node.heading.id;
    const hasChildren = node.children.length > 0;
    const indentClass = depth === 0 ? 'pl-0' : `pl-${depth * 4}`;

    return (
      <li>
            <button
          onClick={() => scrollToHeading(node.heading.id)}
          class={`w-full text-left px-3 py-2 rounded-md text-sm transition-colors ${
              isActive
                ? 'bg-[#e8f0e9] text-[#2d4a2f] font-medium border-l-2 border-[#5a6c5d]'
                : 'text-[#4a5568] hover:bg-[#f7f6f4] hover:text-[#2d3748]'
          } ${indentClass}`}
            aria-current={isActive ? 'location' : undefined}
            aria-label={`Jump to section: ${node.heading.text}`}
          >
            {node.heading.text}
          </button>
        <Show when={hasChildren}>
          <ul class="ml-4 space-y-1 mt-1">
            <For each={node.children}>
              {(child) => renderHeadingNode(child, depth + 1)}
            </For>
          </ul>
        </Show>
      </li>
    );
  };

  return (
    <Show when={headings().length > 0}>
      <aside 
        id="table-of-contents" 
        class="w-64 bg-white border-l border-[#e5e3df] sticky top-[112px] self-start max-h-[calc(100vh-112px)] overflow-y-auto custom-scrollbar" 
        role="complementary" 
        aria-label="Table of contents"
      >
        <nav class="p-5" aria-label="Page contents">
          <h2 class="text-xs font-semibold text-[#6b7280] uppercase tracking-wider mb-4 px-2">
            On This Page
          </h2>
          <ul class="space-y-1">
            <For each={buildHeadingTree()}>
              {(node) => renderHeadingNode(node, 0)}
            </For>
          </ul>
        </nav>
      </aside>
    </Show>
  );
};

export default TableOfContents;
