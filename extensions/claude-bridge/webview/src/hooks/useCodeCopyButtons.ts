// Global delegated click handler for `.hljs-copy-btn` buttons rendered
// inside markdown code blocks (see utils/render-markdown.ts). One listener
// covers every code block in the chat — N×preact onClick listeners would
// re-bind on every JSONL entry re-render, the document-level handler stays
// alive for the lifetime of the renderer.
import { useEffect } from 'preact/hooks'
import { showToast } from '../signals/toasts'
import { writeClipboardText } from '../lib/clipboard'

export function useCodeCopyButtons(): void {
  useEffect(() => {
    const onClick = (e: MouseEvent): void => {
      const target = e.target as HTMLElement | null
      if (!target) return
      const btn = target.closest('.hljs-copy-btn') as HTMLButtonElement | null
      if (!btn) return
      const pre = btn.closest('pre.hljs-block')
      const code = pre?.querySelector<HTMLElement>('code.hljs')
      if (!code) return
      e.preventDefault()
      e.stopPropagation()
      const text = code.innerText
      void writeClipboardText(text).then(() => {
        const icon = btn.querySelector('i')
        const original = icon?.className ?? ''
        if (icon) icon.className = 'fas fa-check'
        btn.classList.add('copied')
        const charCount = text.length
        const lineCount = text.split('\n').length
        showToast({
          type: 'success',
          title: 'Copied to clipboard',
          message: `${lineCount} line${lineCount === 1 ? '' : 's'}, ${charCount} char${charCount === 1 ? '' : 's'}`,
          duration: 2000,
        })
        setTimeout(() => {
          if (icon) icon.className = original || 'fas fa-copy'
          btn.classList.remove('copied')
        }, 1200)
      }).catch((err) => {
        showToast({
          type: 'error',
          title: 'Copy failed',
          message: err instanceof Error ? err.message : 'Clipboard unavailable',
          duration: 3000,
        })
      })
    }
    document.addEventListener('click', onClick, true)
    return () => document.removeEventListener('click', onClick, true)
  }, [])
}
