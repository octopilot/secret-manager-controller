import { Component } from 'solid-js';

const SkipLinks: Component = () => {
  return (
    <div class="skip-links">
      <a href="#main-content" class="skip-link">
        Skip to main content
      </a>
      <a href="#navigation" class="skip-link">
        Skip to navigation
      </a>
      <a href="#table-of-contents" class="skip-link">
        Skip to table of contents
      </a>
    </div>
  );
};

export default SkipLinks;

