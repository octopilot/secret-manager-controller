import MiniSearch from 'minisearch';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { userSections, contributorSections } from '../src/data/sections.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

interface SearchDocument {
  id: string;
  category: 'user' | 'contributor';
  section: string;
  sectionTitle: string;
  page: string;
  title: string;
  content: string;
  url: string;
}

/**
 * Strip markdown formatting to extract plain text for search
 */
function stripMarkdown(content: string): string {
  return content
    .replace(/^#+\s+/gm, '') // Remove headers
    .replace(/\[([^\]]+)\]\([^\)]+\)/g, '$1') // Convert links to text
    .replace(/`([^`]+)`/g, '$1') // Remove inline code backticks
    .replace(/\*\*([^\*]+)\*\*/g, '$1') // Remove bold
    .replace(/\*([^\*]+)\*/g, '$1') // Remove italic
    .replace(/```[\s\S]*?```/g, '') // Remove code blocks
    .replace(/^\s*[-*+]\s+/gm, '') // Remove list markers
    .replace(/^\s*\d+\.\s+/gm, '') // Remove numbered list markers
    .replace(/\n{3,}/g, '\n\n') // Normalize multiple newlines
    .trim();
}

async function buildSearchIndex() {
  const documents: SearchDocument[] = [];
  
  // Process user documentation
  for (const section of userSections) {
    for (const page of section.pages) {
      // Skip pages without files (e.g., special component pages like secrets-viewer)
      if (!page.file || page.file.trim() === '') {
        continue;
      }
      const filePath = join(__dirname, '../src/data/content/user', page.file);
      try {
        const content = readFileSync(filePath, 'utf-8');
        const plainText = stripMarkdown(content);
        
        documents.push({
          id: `user-${section.id}-${page.id}`,
          category: 'user',
          section: section.id,
          sectionTitle: section.title,
          page: page.id,
          title: page.title,
          content: plainText,
          url: `#/user/${section.id}/${page.id}`,
        });
      } catch (err) {
        console.warn(`⚠️  Failed to read ${filePath}:`, err);
      }
    }
  }
  
  // Process contributor documentation
  for (const section of contributorSections) {
    for (const page of section.pages) {
      // Skip pages without files (e.g., special component pages)
      if (!page.file || page.file.trim() === '') {
        continue;
      }
      const filePath = join(__dirname, '../src/data/content/contributor', page.file);
      try {
        const content = readFileSync(filePath, 'utf-8');
        const plainText = stripMarkdown(content);
        
        documents.push({
          id: `contributor-${section.id}-${page.id}`,
          category: 'contributor',
          section: section.id,
          sectionTitle: section.title,
          page: page.id,
          title: page.title,
          content: plainText,
          url: `#/contributor/${section.id}/${page.id}`,
        });
      } catch (err) {
        console.warn(`⚠️  Failed to read ${filePath}:`, err);
      }
    }
  }
  
  // Process landing page (index.md)
  try {
    const indexPath = join(__dirname, '../src/data/content/user/index.md');
    const indexContent = readFileSync(indexPath, 'utf-8');
    const plainText = stripMarkdown(indexContent);
    
    documents.push({
      id: 'user-index',
      category: 'user',
      section: '',
      sectionTitle: 'Home',
      page: 'index',
      title: 'Secret Manager Controller',
      content: plainText,
      url: '#/',
    });
  } catch (err) {
    console.warn(`⚠️  Failed to read index.md:`, err);
  }
  
  // Create search index
  const searchIndex = new MiniSearch<SearchDocument>({
    fields: ['title', 'content', 'sectionTitle'], // Fields to index
    storeFields: ['id', 'category', 'section', 'sectionTitle', 'page', 'title', 'url'], // Fields to return
    searchOptions: {
      boost: { title: 3, sectionTitle: 2, content: 1 }, // Boost title matches
      fuzzy: 0.2, // Enable fuzzy matching
      prefix: true, // Match prefixes
    },
  });
  
  // Add all documents to index
  searchIndex.addAll(documents);
  
  // Export index as JSON
  const indexData = searchIndex.toJSON();
  const outputDir = join(__dirname, '../src/data');
  const outputPath = join(outputDir, 'search-index.json');
  
  // Ensure directory exists
  mkdirSync(outputDir, { recursive: true });
  
  writeFileSync(outputPath, JSON.stringify(indexData), 'utf-8');
  
  console.log(`✅ Search index built successfully!`);
  console.log(`   Documents indexed: ${documents.length}`);
  console.log(`   Output: ${outputPath}`);
  console.log(`   Index size: ${(JSON.stringify(indexData).length / 1024).toFixed(2)} KB`);
}

buildSearchIndex().catch((err) => {
  console.error('❌ Failed to build search index:', err);
  process.exit(1);
});

