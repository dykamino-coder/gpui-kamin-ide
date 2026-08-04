// Imperative window-level bridge: child renderers (BashBlock "Use as input"
// action) need to push text into the prompt textarea without an explicit
// signal chain. A window-level helper avoids prop-drilling through 6+
// layers of JsonlViewer/JsonlEntry/JsonlToolUse and keeps InputBar as the
// sole owner of the textarea ref. Mounted/unmounted with the InputBar
// instance so a renderer-only component never sees stale closures.

import { useEffect } from 'preact/hooks'

export function useAppendToInputBridge(
  textareaRef: { current: HTMLTextAreaElement | null },
  setHasText: (b: boolean) => void,
): void {
  useEffect(() => {
    const w = window as unknown as { __appendToInput?: (text: string) => void }
    const fn = (text: string) => {
      const el = textareaRef.current
      if (!el) return
      const sep = el.value && !el.value.endsWith('\n') ? '\n' : ''
      el.value = el.value + sep + text
      el.dispatchEvent(new Event('input', { bubbles: true }))
      el.focus()
      el.selectionStart = el.selectionEnd = el.value.length
      setHasText(true)
    }
    w.__appendToInput = fn
    return () => {
      const w2 = window as unknown as { __appendToInput?: (text: string) => void }
      if (w2.__appendToInput === fn) delete w2.__appendToInput
    }
  }, [])
}
