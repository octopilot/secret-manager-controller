import { Component, createEffect } from 'solid-js';
import { userSections, contributorSections, DocCategory } from '../../data/sections';

interface MetaTagsProps {
  category: DocCategory;
  section: string | null;
  page: string | null;
  baseUrl?: string;
}

const MetaTags: Component<MetaTagsProps> = (props) => {
  const baseUrl = () => props.baseUrl || 'https://octopilot.github.io/secret-manager-controller';

  const getPageMetadata = () => {
    // Landing page
    if (props.page === 'index' && !props.section) {
      return {
        title: 'Secret Manager Controller - Unified Secret Management for Kubernetes and Serverless',
        description: 'Unlock serverless migration and deliver massive FinOps savings. Bridge SOPS-encrypted secrets from Git to cloud-native secret stores. Move workloads to serverless, shrink Kubernetes footprint, and cut cloud costs.',
        keywords: 'secret management, kubernetes, serverless, SOPS, GitOps, FinOps, cloud secrets, AWS Secrets Manager, Azure Key Vault, GCP Secret Manager',
      };
    }

    // Find page in sections
    const sections = props.category === 'user' ? userSections : contributorSections;
    const sectionObj = props.section ? sections.find(s => s.id === props.section) : null;
    const pageObj = sectionObj && props.page ? sectionObj.pages.find(p => p.id === props.page) : null;

    if (pageObj && sectionObj) {
      const categoryLabel = props.category === 'user' ? 'User' : 'Contributor';
      return {
        title: `${pageObj.title} - ${sectionObj.title} | Secret Manager Controller ${categoryLabel} Docs`,
        description: `Learn about ${pageObj.title.toLowerCase()} in the Secret Manager Controller ${categoryLabel} documentation. ${sectionObj.title} guide for managing secrets across Kubernetes and serverless platforms.`,
        keywords: `secret manager controller, ${pageObj.title.toLowerCase()}, ${sectionObj.title.toLowerCase()}, ${props.category} documentation, kubernetes secrets, serverless secrets`,
      };
    }

    // Fallback
    return {
      title: 'Secret Manager Controller - Documentation',
      description: 'Documentation for Secret Manager Controller - Unified secret management for Kubernetes and serverless platforms.',
      keywords: 'secret management, kubernetes, serverless, documentation',
    };
  };

  const metadata = () => getPageMetadata();
  const canonicalUrl = () => {
    if (props.page === 'index' && !props.section) {
      return `${baseUrl()}/`;
    }
    const path = `/${props.category}/${props.section}${props.page ? `/${props.page}` : ''}`;
    return `${baseUrl()}${path}`;
  };

  createEffect(() => {
    const meta = metadata();
    const url = canonicalUrl();

    // Update document title
    document.title = meta.title;

    // Update or create meta tags
    const updateMetaTag = (name: string, content: string, isProperty = false) => {
      const selector = isProperty ? `meta[property="${name}"]` : `meta[name="${name}"]`;
      let tag = document.querySelector(selector) as HTMLMetaElement;
      if (!tag) {
        tag = document.createElement('meta');
        if (isProperty) {
          tag.setAttribute('property', name);
        } else {
          tag.setAttribute('name', name);
        }
        document.head.appendChild(tag);
      }
      tag.setAttribute('content', content);
    };

    // Basic meta tags
    updateMetaTag('description', meta.description);
    updateMetaTag('keywords', meta.keywords);

    // Open Graph tags
    updateMetaTag('og:title', meta.title, true);
    updateMetaTag('og:description', meta.description, true);
    updateMetaTag('og:url', url, true);
    updateMetaTag('og:type', 'website', true);
    updateMetaTag('og:site_name', 'Secret Manager Controller', true);

    // Twitter Card tags
    updateMetaTag('twitter:card', 'summary_large_image', true);
    updateMetaTag('twitter:title', meta.title, true);
    updateMetaTag('twitter:description', meta.description, true);

    // Canonical URL
    let canonical = document.querySelector('link[rel="canonical"]') as HTMLLinkElement;
    if (!canonical) {
      canonical = document.createElement('link');
      canonical.setAttribute('rel', 'canonical');
      document.head.appendChild(canonical);
    }
    canonical.setAttribute('href', url);
  });

  // JSON-LD structured data
  createEffect(() => {
    // Remove existing structured data
    const existingScript = document.querySelector('script[type="application/ld+json"]');
    if (existingScript) {
      existingScript.remove();
    }

    const meta = metadata();
    const structuredData = {
      '@context': 'https://schema.org',
      '@type': 'TechArticle',
      headline: meta.title,
      description: meta.description,
      url: canonicalUrl(),
      author: {
        '@type': 'Organization',
        name: 'Microscaler',
      },
      publisher: {
        '@type': 'Organization',
        name: 'Microscaler',
      },
      mainEntityOfPage: {
        '@type': 'WebPage',
        '@id': canonicalUrl(),
      },
    };

    const script = document.createElement('script');
    script.type = 'application/ld+json';
    script.textContent = JSON.stringify(structuredData);
    document.head.appendChild(script);
  });

  return null; // This component doesn't render anything
};

export default MetaTags;

