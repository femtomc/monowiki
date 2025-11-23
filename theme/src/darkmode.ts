/**
 * Dark Mode Toggle
 * Persists preference in localStorage
 */

type Theme = 'light' | 'dark';

class DarkModeManager {
  private readonly STORAGE_KEY = 'monowiki-theme';
  private currentTheme: Theme = 'light';

  init(): void {
    // Load saved preference or detect system preference
    this.currentTheme = this.loadTheme();
    this.applyTheme(this.currentTheme);

    // Create toggle button if not present
    this.setupToggle();

    // Listen for system theme changes
    this.watchSystemTheme();
  }

  private loadTheme(): Theme {
    // Check localStorage first
    const saved = localStorage.getItem(this.STORAGE_KEY);
    if (saved === 'dark' || saved === 'light') {
      return saved;
    }

    // Fall back to system preference
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      return 'dark';
    }

    return 'light';
  }

  private applyTheme(theme: Theme): void {
    const html = document.documentElement;

    if (theme === 'dark') {
      html.classList.add('dark');
    } else {
      html.classList.remove('dark');
    }

    this.currentTheme = theme;
    localStorage.setItem(this.STORAGE_KEY, theme);
  }

  private toggleTheme(): void {
    const newTheme: Theme = this.currentTheme === 'light' ? 'dark' : 'light';
    this.applyTheme(newTheme);
    this.updateToggleButton();
  }

  private setupToggle(): void {
    // Find existing toggle button or create one
    let toggleBtn = document.getElementById('theme-toggle') as HTMLButtonElement;

    if (!toggleBtn) {
      // Create toggle button in nav
      const nav = document.querySelector('nav');
      if (nav) {
        toggleBtn = document.createElement('button');
        toggleBtn.id = 'theme-toggle';
        toggleBtn.className = 'theme-toggle';
        toggleBtn.setAttribute('aria-label', 'Toggle dark mode');
        nav.appendChild(toggleBtn);
      } else {
        return;
      }
    }

    this.updateToggleButton();

    toggleBtn.addEventListener('click', () => {
      this.toggleTheme();
    });
  }

  private updateToggleButton(): void {
    const toggleBtn = document.getElementById('theme-toggle');
    if (!toggleBtn) return;

    // Update button text/icon
    toggleBtn.textContent = this.currentTheme === 'dark' ? '☀️' : '🌙';
    toggleBtn.setAttribute('aria-label',
      this.currentTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'
    );
  }

  private watchSystemTheme(): void {
    if (!window.matchMedia) return;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', (e) => {
      // Only auto-switch if user hasn't set a preference
      const saved = localStorage.getItem(this.STORAGE_KEY);
      if (!saved) {
        this.applyTheme(e.matches ? 'dark' : 'light');
        this.updateToggleButton();
      }
    });
  }
}

// Singleton instance
let darkModeManager: DarkModeManager | null = null;

export function initDarkMode(): void {
  if (!darkModeManager) {
    darkModeManager = new DarkModeManager();
    darkModeManager.init();
  }
}

export type { Theme };
