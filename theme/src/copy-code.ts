/**
 * Handle copy button clicks for code blocks
 */

export function initCopyCode() {
  document.querySelectorAll('.copy-code-btn').forEach((button) => {
    button.addEventListener('click', async () => {
      const codeBlock = button.closest('.code-block');
      const pre = codeBlock?.querySelector('pre');
      if (!pre) return;

      const code = pre.textContent || '';
      const success = await copyText(code);
      showStatus(button as HTMLButtonElement, success);
    });
  });
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

function showStatus(button: HTMLButtonElement, success: boolean) {
  const originalText = button.textContent;
  button.textContent = success ? 'Copied!' : 'Failed';
  button.dataset.state = success ? 'copied' : 'error';

  setTimeout(() => {
    button.textContent = originalText;
    button.dataset.state = '';
  }, 1200);
}
