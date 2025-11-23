/**
 * Table of Contents Component
 * Generates floating TOC with active section highlighting
 */

interface TocItem {
  id: string;
  text: string;
  level: number;
  element: HTMLHeadingElement;
}

class TOCManager {
  private tocContainer: HTMLElement | null = null;
  private tocItems: TocItem[] = [];
  private activeId: string | null = null;

  init(): void {
    // Find TOC container (if present)
    this.tocContainer = document.getElementById('toc');
    if (!this.tocContainer) return;

    // Check if TOC already exists (rendered by backend)
    const existingToc = this.tocContainer.querySelector('.toc-nav');
    if (existingToc) {
      // TOC already rendered by backend, just set up scroll spy
      this.extractHeadingsFromExistingTOC();
      this.setupScrollSpy();
      return;
    }

    // Extract headings from content
    this.extractHeadings();

    if (this.tocItems.length === 0) {
      this.tocContainer.style.display = 'none';
      return;
    }

    // Render TOC
    this.renderTOC();

    // Set up scroll spy
    this.setupScrollSpy();
  }

  private extractHeadings(): void {
    const content = document.querySelector('article, main, .content');
    if (!content) return;

    const headings = content.querySelectorAll('h2, h3, h4');
    headings.forEach((heading) => {
      const h = heading as HTMLHeadingElement;
      const level = parseInt(h.tagName.substring(1));

      // Ensure heading has an ID
      if (!h.id) {
        h.id = this.slugify(h.textContent || '');
      }

      this.tocItems.push({
        id: h.id,
        text: h.textContent || '',
        level: level,
        element: h,
      });
    });
  }

  private extractHeadingsFromExistingTOC(): void {
    if (!this.tocContainer) return;

    const links = this.tocContainer.querySelectorAll('.toc-list a');
    links.forEach((link) => {
      const href = link.getAttribute('href');
      if (!href || !href.startsWith('#')) return;

      const id = href.substring(1);
      const heading = document.getElementById(id) as HTMLHeadingElement;
      if (!heading) return;

      const level = parseInt(heading.tagName.substring(1));

      // Find or create the parent li element
      const li = link.closest('li');
      if (li) {
        li.dataset.id = id;
      }

      this.tocItems.push({
        id: id,
        text: link.textContent || '',
        level: level,
        element: heading,
      });
    });
  }

  private renderTOC(): void {
    if (!this.tocContainer) return;

    const nav = document.createElement('nav');
    nav.className = 'toc-nav';

    const title = document.createElement('h3');
    title.textContent = 'Contents';
    nav.appendChild(title);

    const list = document.createElement('ul');
    list.className = 'toc-list';

    this.tocItems.forEach(item => {
      const li = document.createElement('li');
      li.className = `toc-level-${item.level}`;
      li.dataset.id = item.id;

      const link = document.createElement('a');
      link.href = `#${item.id}`;
      link.textContent = item.text;
      link.addEventListener('click', (e) => {
        e.preventDefault();
        this.scrollToHeading(item.id);
      });

      li.appendChild(link);
      list.appendChild(li);
    });

    nav.appendChild(list);
    this.tocContainer.appendChild(nav);
  }

  private setupScrollSpy(): void {
    // Intersection Observer for active section highlighting
    const options = {
      rootMargin: '-100px 0px -66%',
      threshold: 1.0,
    };

    const observer = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          const id = entry.target.id;
          this.setActive(id);
        }
      });
    }, options);

    this.tocItems.forEach(item => {
      observer.observe(item.element);
    });
  }

  private setActive(id: string): void {
    if (this.activeId === id) return;

    // Remove previous active
    if (this.activeId) {
      const prevItem = this.tocContainer?.querySelector(`[data-id="${this.activeId}"]`);
      prevItem?.classList.remove('active');
    }

    // Set new active
    this.activeId = id;
    const newItem = this.tocContainer?.querySelector(`[data-id="${id}"]`);
    newItem?.classList.add('active');
  }

  private scrollToHeading(id: string): void {
    const heading = document.getElementById(id);
    if (!heading) return;

    heading.scrollIntoView({ behavior: 'smooth', block: 'start' });
    window.history.pushState(null, '', `#${id}`);
  }

  private slugify(text: string): string {
    return text
      .toLowerCase()
      .replace(/[^\w\s-]/g, '')
      .replace(/\s+/g, '-')
      .replace(/-+/g, '-')
      .trim();
  }
}

// Singleton instance
let tocManager: TOCManager | null = null;

export function initTOC(): void {
  if (!tocManager) {
    tocManager = new TOCManager();
    tocManager.init();
  }
}

export type { TocItem };
