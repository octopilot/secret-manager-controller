import { Component, Show, For } from 'solid-js';

interface KeyboardShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

interface Shortcut {
  keys: string[];
  description: string;
}

const KeyboardShortcutsModal: Component<KeyboardShortcutsModalProps> = (props) => {
  const isMac = () => navigator.platform.includes('Mac');

  const shortcuts: Shortcut[] = [
    {
      keys: [isMac() ? '⌘' : 'Ctrl', 'K'],
      description: 'Open search',
    },
    {
      keys: ['Esc'],
      description: 'Close search or modal',
    },
    {
      keys: ['↑', '↓'],
      description: 'Navigate search results',
    },
    {
      keys: ['Enter'],
      description: 'Select search result',
    },
    {
      keys: ['?'],
      description: 'Show keyboard shortcuts',
    },
  ];

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center p-4"
        onClick={props.onClose}
      >
        <div
          class="bg-white rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] overflow-y-auto"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div class="p-6 border-b border-[#e5e3df] flex items-center justify-between">
            <h2 class="text-2xl font-semibold text-[#2d3748]">Keyboard Shortcuts</h2>
            <button
              onClick={props.onClose}
              class="p-2 rounded-lg hover:bg-[#f7f6f4] transition-colors"
              aria-label="Close keyboard shortcuts"
            >
              <svg class="w-5 h-5 text-[#6b7280]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Shortcuts list */}
          <div class="p-6">
            <For each={shortcuts}>
              {(shortcut) => (
                <div class="flex items-center justify-between py-3 border-b border-[#e5e3df] last:border-b-0">
                  <span class="text-[#4a5568]">{shortcut.description}</span>
                  <div class="flex items-center gap-1">
                    <For each={shortcut.keys}>
                      {(key, index) => (
                        <>
                          <kbd class="px-2 py-1 text-xs font-mono bg-[#f7f6f4] border border-[#e5e3df] rounded">
                            {key}
                          </kbd>
                          {index() < shortcut.keys.length - 1 && (
                            <span class="text-[#6b7280] mx-1">+</span>
                          )}
                        </>
                      )}
                    </For>
                  </div>
                </div>
              )}
            </For>
          </div>

          {/* Footer */}
          <div class="p-4 border-t border-[#e5e3df] bg-[#faf9f7] text-center text-sm text-[#6b7280]">
            Press <kbd class="px-1.5 py-0.5 text-xs bg-white border border-[#e5e3df] rounded font-mono">Esc</kbd> to close
          </div>
        </div>
      </div>
    </Show>
  );
};

export default KeyboardShortcutsModal;

