/**
 * Make Typst math SVGs copyable by copying the original math source
 */

export function initMathCopy() {
  const mathNodes = new Set<HTMLElement>();

  const attach = (mathEl: Element) => {
    if (!(mathEl instanceof HTMLElement) || mathNodes.has(mathEl)) return;
    mathNodes.add(mathEl);

    mathEl.style.userSelect = 'text';
    mathEl.style.cursor = 'text';

    const handleCopy = (e: ClipboardEvent) => {
      const mathSource = mathEl.getAttribute('data-math') || '';
      if (!mathSource || !selectionIntersects(mathEl)) return;

      e.preventDefault();
      copyText(mathSource).then();
    };

    mathEl.addEventListener('copy', handleCopy);
  };

  document.querySelectorAll('.typst-math').forEach(attach);

  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => {
        if (!(node instanceof Element)) return;
        if (node.classList?.contains('typst-math')) {
          attach(node);
        }
        node.querySelectorAll?.('.typst-math').forEach(attach);
      });
    }
  });

  observer.observe(document.body, { childList: true, subtree: true });

  document.addEventListener('selectionchange', () => {
    const sel = window.getSelection();
    mathNodes.forEach((el) => {
      if (sel && !sel.isCollapsed && selectionIntersects(el)) {
        el.classList.add('selecting');
      } else {
        el.classList.remove('selecting');
      }
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

function selectionIntersects(el: HTMLElement): boolean {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) return false;
  try {
    const range = sel.getRangeAt(0);
    return range.intersectsNode(el);
  } catch {
    return false;
  }
}
