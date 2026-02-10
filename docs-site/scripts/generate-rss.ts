import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { userSections, contributorSections } from '../src/data/sections.js';
import { DOCS_BASE_URL } from '../src/data/site-config';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const BASE_URL = process.env.DOCS_BASE_URL || DOCS_BASE_URL;
const OUTPUT_DIR = join(__dirname, '../public');
const OUTPUT_FILE = join(OUTPUT_DIR, 'feed.xml');

interface RSSItem {
  title: string;
  link: string;
  description: string;
  pubDate: string;
  category?: string;
}

function generateRSS(): void {
  const now = new Date();
  const buildDate = now.toUTCString();
  
  const items: RSSItem[] = [];

  // Landing page
  items.push({
    title: 'Secret Manager Controller - Documentation',
    link: `${BASE_URL}/`,
    description: 'Unified secret management for Kubernetes and serverless platforms. Unlock serverless migration and deliver massive FinOps savings.',
    pubDate: buildDate,
    category: 'Documentation',
  });

  // User documentation pages
  userSections.forEach((section) => {
    section.pages.forEach((page) => {
      items.push({
        title: `${page.title} - ${section.title}`,
        link: `${BASE_URL}/#/user/${section.id}/${page.id}`,
        description: `${page.title} documentation in the ${section.title} section of Secret Manager Controller user documentation.`,
        pubDate: buildDate,
        category: `User Docs / ${section.title}`,
      });
    });
  });

  // Contributor documentation pages
  contributorSections.forEach((section) => {
    section.pages.forEach((page) => {
      items.push({
        title: `${page.title} - ${section.title}`,
        link: `${BASE_URL}/#/contributor/${section.id}/${page.id}`,
        description: `${page.title} documentation in the ${section.title} section of Secret Manager Controller contributor documentation.`,
        pubDate: buildDate,
        category: `Contributor Docs / ${section.title}`,
      });
    });
  });

  // Generate RSS XML
  const rss = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>Secret Manager Controller Documentation</title>
    <link>${BASE_URL}</link>
    <description>Documentation for Secret Manager Controller - Unified secret management for Kubernetes and serverless platforms</description>
    <language>en-us</language>
    <lastBuildDate>${buildDate}</lastBuildDate>
    <pubDate>${buildDate}</pubDate>
    <ttl>60</ttl>
    <atom:link href="${BASE_URL}/feed.xml" rel="self" type="application/rss+xml" />
    ${items
      .map(
        (item) => `    <item>
      <title>${escapeXml(item.title)}</title>
      <link>${escapeXml(item.link)}</link>
      <description>${escapeXml(item.description)}</description>
      <pubDate>${item.pubDate}</pubDate>
      ${item.category ? `<category>${escapeXml(item.category)}</category>` : ''}
      <guid isPermaLink="true">${escapeXml(item.link)}</guid>
    </item>`
      )
      .join('\n')}
  </channel>
</rss>`;

  // Write to file
  writeFileSync(OUTPUT_FILE, rss, 'utf-8');
  console.log(`✅ RSS feed generated: ${OUTPUT_FILE}`);
  console.log(`   Total items: ${items.length}`);
}

function escapeXml(unsafe: string): string {
  return unsafe
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  generateRSS();
}

export { generateRSS };

