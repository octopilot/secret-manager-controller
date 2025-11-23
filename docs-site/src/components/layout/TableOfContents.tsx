import { Component, createSignal, createEffect, For, Show, onMount, onCleanup } from 'solid-js';

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
  let observer: IntersectionObserver | null = null;

  // Extract headings from markdown content
  createEffect(() => {
    if (!props.content) {
      setHeadings([]);
      return;
    }

    // Parse markdown headings (lines starting with #)
    const lines = props.content.split('\n');
    const extractedHeadings: Heading[] = [];
    
    // Pattern to detect timestamps (dates, times, ISO dates, etc.)
    // Matches: YYYY-MM-DD, MM/DD/YYYY, HH:MM, ISO dates, and timestamps with colons
    const timestampPattern = /\d{4}-\d{2}-\d{2}|\d{1,2}\/\d{1,2}\/\d{2,4}|\d{1,2}:\d{2}(:\d{2})?|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/;
    
    lines.forEach((line) => {
      const match = line.match(/^(#{1,6})\s+(.+)$/);
      if (match) {
        const level = match[1].length;
        const text = match[2].trim();
        
        // Only include H1 and H2 headings (exclude H3 and below)
        if (level > 2) {
          return;
        }
        
        // Filter out headings that look like timestamps (check if text is primarily a timestamp)
        // If the text matches timestamp pattern and is short, it's likely a timestamp
        if (timestampPattern.test(text) && text.length < 30) {
          return;
        }
        
        // Generate ID from heading text (lowercase, replace spaces with hyphens, remove special chars)
        const id = text
          .toLowerCase()
          .replace(/[^\w\s-]/g, '')
          .replace(/\s+/g, '-')
          .replace(/-+/g, '-')
          .trim();
        
        extractedHeadings.push({ id, text, level });
      }
    });

    setHeadings(extractedHeadings);
  });

  // Set up Intersection Observer to track active heading (simplified like PriceWhisperer FTE)
  const setupObserver = () => {
    if (headings().length === 0) {
      return;
    }

    // Clean up existing observer
    if (observer) {
      observer.disconnect();
      observer = null;
    }

    // Find the scrollable main content area
    const mainContent = document.querySelector('main.flex-1.overflow-y-auto') as HTMLElement | null;

    // Create observer with options (simplified from FTE pattern)
    const observerOptions = {
      root: mainContent,
      rootMargin: '-112px 0px -70% 0px', // Account for header (112px), trigger when heading is near top
      threshold: [0, 0.25, 0.5, 0.75, 1],
    };

    observer = new IntersectionObserver((entries) => {
      // Find the entry that's most visible and closest to the top
      let mostVisible: IntersectionObserverEntry | null = null;
      let highestRatio = 0;
      let closestToTop = Infinity;

      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const ratio = entry.intersectionRatio;
          const top = entry.boundingClientRect.top;

          // Prefer entries that are:
          // 1. More visible (higher ratio)
          // 2. Closer to the top of the viewport
          if (ratio > highestRatio || (ratio === highestRatio && top < closestToTop)) {
            mostVisible = entry;
            highestRatio = ratio;
            closestToTop = top;
          }
        }
      });

      // If we found a visible heading, set it as active
      if (mostVisible) {
        const anchor = mostVisible.target.id;
        if (anchor) {
          setActiveAnchor(anchor);
        }
      } else {
        // If no heading is intersecting, find the one that's just above the viewport
        entries.forEach((entry) => {
          const rect = entry.boundingClientRect;
          if (rect.top < 120 && rect.bottom > 0) {
            const anchor = entry.target.id;
            if (anchor) {
              setActiveAnchor(anchor);
            }
          }
        });
      }
    }, observerOptions);

    // Function to observe headings
    const observeHeadings = () => {
      if (!observer) return;

      // Observe all headings that are in our headings list
      headings().forEach((heading) => {
        const element = document.getElementById(heading.id);
        if (element) {
          observer!.observe(element);
        }
      });

      // Also observe all headings with IDs as a fallback (only H1 and H2)
      const allHeadings = document.querySelectorAll('h1[id], h2[id]');
      allHeadings.forEach((heading) => {
        if (!heading.id || !headings().some(h => h.id === heading.id)) {
          observer!.observe(heading);
        }
      });
    };

    // Try to observe immediately
    observeHeadings();

    // Also try after delays to catch async-loaded content (headings get IDs after render)
    setTimeout(observeHeadings, 100);
    setTimeout(observeHeadings, 500);
  };

  onMount(() => {
    setupObserver();
  });

  // Re-initialize observer when content or headings change
  createEffect(() => {
    // Track headings to trigger effect
    headings().length;
    // Reset active anchor
    setActiveAnchor(null);
    // Re-setup observer after a short delay to allow content to load
    setTimeout(() => {
      setupObserver();
    }, 100);
  });

  onCleanup(() => {
    if (observer) {
      observer.disconnect();
      observer = null;
    }
  });

  const scrollToHeading = (id: string) => {
    // Try to find element
    let element = document.getElementById(id);
    
    if (!element) {
      // Try querySelector
      element = document.querySelector(`#${id}`) as HTMLElement;
    }
    
    if (!element) {
      // Try finding headings with matching text (fallback - only H1 and H2)
      const allHeadings = document.querySelectorAll('h1, h2');
      for (const heading of Array.from(allHeadings)) {
        const headingText = heading.textContent || heading.innerText || '';
        const headingId = headingText
          .toLowerCase()
          .replace(/[^\w\s-]/g, '')
          .replace(/\s+/g, '-')
          .replace(/-+/g, '-')
          .trim();
        if (headingId === id || heading.id === id) {
          element = heading as HTMLElement;
          if (!element.id || element.id !== id) {
            element.id = id;
          }
          break;
        }
      }
    }
    
    // If still not found, wait a bit and retry (for async content loading)
    if (!element) {
      setTimeout(() => {
        let retryElement = document.getElementById(id);
        if (retryElement) {
          scrollToElement(retryElement);
        }
      }, 200);
      return;
    }
    
    scrollToElement(element);
  };

  const scrollToElement = (element: HTMLElement) => {
    // Account for sticky header offset
    const headerOffset = 112; // Height of sticky header with breadcrumbs
    const elementPosition = element.getBoundingClientRect().top + window.pageYOffset;
    const offsetPosition = elementPosition - headerOffset;
    
    window.scrollTo({
      top: offsetPosition,
      behavior: 'smooth'
    });
    
    // Also scroll the main content area if it's scrollable
    const mainContent = document.querySelector('main.flex-1.overflow-y-auto') as HTMLElement;
    if (mainContent) {
      const mainRect = mainContent.getBoundingClientRect();
      const elementRect = element.getBoundingClientRect();
      const relativeTop = elementRect.top - mainRect.top + mainContent.scrollTop - headerOffset;
      mainContent.scrollTo({
        top: Math.max(0, relativeTop),
        behavior: 'smooth'
      });
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
    const isActive = activeAnchor() === node.heading.id;
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
