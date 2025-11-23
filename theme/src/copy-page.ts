/**
 * Copy the current page's raw markdown source.
 */

export function initCopyPageSource() {
  const button = document.getElementById('copy-page-source');
  const source = readSourcePayload();

  if (!(button instanceof HTMLButtonElement)) return;

  const defaultLabel = button.textContent || 'Copy page source';

  if (!source) {
    button.disabled = true;
    button.title = 'Source unavailable';
    return;
  }

  button.addEventListener('click', async () => {
    const success = await copyText(source);
    showStatus(button, success ? 'Copied!' : 'Copy failed', defaultLabel);
  });
}

function readSourcePayload(): string | null {
  const el = document.getElementById('page-source-data');
  if (!el) return null;
  try {
    const raw = el.textContent || '';
    return JSON.parse(raw);
  } catch (err) {
    console.warn('Failed to parse page source payload', err);
    return null;
  }
}

async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (err) {
    console.warn('Navigator clipboard copy failed, falling back', err);
  }

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'absolute';
  textarea.style.left = '-9999px';
  document.body.appendChild(textarea);
  textarea.select();

  let success = false;
  try {
    success = document.execCommand('copy');
  } catch (err) {
    console.warn('Fallback copy failed', err);
  } finally {
    document.body.removeChild(textarea);
  }

  return success;
}

function showStatus(button: HTMLButtonElement, status: string, defaultLabel: string) {
  const prev = button.textContent;
  button.textContent = status;
  button.dataset.state = status.toLowerCase().includes('copied') ? 'copied' : 'error';

  setTimeout(() => {
    button.textContent = prev || defaultLabel;
    button.dataset.state = '';
  }, 1200);
}
