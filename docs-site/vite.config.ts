import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import path from 'path';

export default defineConfig({
  plugins: [solid()],
  base: '/',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Aggressive code splitting for performance
    rollupOptions: {
      output: {
        manualChunks: (id) => {
          // Split diagram libraries into separate chunks for better lazy loading
          if (id.includes('mermaid')) {
            return 'mermaid';
          }
          if (id.includes('chart.js')) {
            return 'chartjs';
          }
          if (id.includes('d3')) {
            return 'd3';
          }
          // Split math libraries
          if (id.includes('katex')) {
            return 'katex';
          }
          // Split markdown parser
          if (id.includes('marked')) {
            return 'marked';
          }
          // Split by route/page
          if (id.includes('/pages/')) {
            const match = id.match(/\/pages\/([^/]+)/);
            return match ? `page-${match[1]}` : 'pages';
          }
          // Split by section data
          if (id.includes('/data/content/')) {
            return 'content-data';
          }
        },
      },
    },
    chunkSizeWarningLimit: 2000,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 3002, // Different from other services
    open: true,
  },
});

