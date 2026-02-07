import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { userSections, contributorSections } from '../src/data/sections.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const BASE_URL = process.env.DOCS_BASE_URL || 'https://octopilot.github.io/secret-manager-controller';
const OUTPUT_DIR = join(__dirname, '../public');
const OUTPUT_FILE = join(OUTPUT_DIR, 'sitemap.xml');

interface SitemapUrl {
  loc: string;
  lastmod?: string;
  changefreq?: 'always' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'yearly' | 'never';
  priority?: number;
}

function generateSitemap(): void {
  const urls: SitemapUrl[] = [];
  const now = new Date().toISOString().split('T')[0]; // YYYY-MM-DD format

  // Landing page
  urls.push({
    loc: `${BASE_URL}/`,
    lastmod: now,
    changefreq: 'weekly',
    priority: 1.0,
  });

  // User documentation pages
  userSections.forEach((section) => {
    section.pages.forEach((page) => {
      urls.push({
        loc: `${BASE_URL}/#/user/${section.id}/${page.id}`,
        lastmod: now,
        changefreq: 'monthly',
        priority: 0.8,
      });
    });
  });

  // Contributor documentation pages
  contributorSections.forEach((section) => {
    section.pages.forEach((page) => {
      urls.push({
        loc: `${BASE_URL}/#/contributor/${section.id}/${page.id}`,
        lastmod: now,
        changefreq: 'monthly',
        priority: 0.7,
      });
    });
  });

  // Generate XML
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls
  .map(
    (url) => `  <url>
    <loc>${escapeXml(url.loc)}</loc>
    ${url.lastmod ? `    <lastmod>${url.lastmod}</lastmod>` : ''}
    ${url.changefreq ? `    <changefreq>${url.changefreq}</changefreq>` : ''}
    ${url.priority !== undefined ? `    <priority>${url.priority}</priority>` : ''}
  </url>`
  )
  .join('\n')}
</urlset>`;

  // Write to file
  writeFileSync(OUTPUT_FILE, xml, 'utf-8');
  console.log(`✅ Sitemap generated: ${OUTPUT_FILE}`);
  console.log(`   Total URLs: ${urls.length}`);
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
  generateSitemap();
}

export { generateSitemap };

