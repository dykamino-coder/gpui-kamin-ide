import type { Terminal } from '@xterm/xterm'

interface ClipboardOptions {
  writeText?: (text: string) => unknown
  readText?: () => Promise<string>
  handleKey?: (event: KeyboardEvent) => boolean
}

const openMenus = new WeakMap<Document, () => void>()

function fallbackCopy(document: Document, text: string): void {
  const previous = document.activeElement as HTMLElement | null
  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.style.cssText = 'position:fixed;left:-9999px;top:-9999px'
  document.body.appendChild(textarea)
  try { textarea.select(); document.execCommand('copy') }
  finally { textarea.remove(); previous?.focus() }
}

/** One clipboard/menu policy for the console. Native paste is the sole keyboard
 * paste path, avoiding a second send from an asynchronous clipboard read. */
export function configureTerminalClipboard(terminal: Terminal, container: HTMLElement, options: ClipboardOptions = {}): () => void {
  const document = container.ownerDocument
  const clipboard = document.defaultView?.navigator.clipboard
  const copy = (text: string): void => {
    if (options.writeText) { void options.writeText(text); return }
    if (clipboard?.writeText) {
      void clipboard.writeText(text).catch(() => fallbackCopy(document, text))
    } else fallbackCopy(document, text)
  }
  const copySelection = (): boolean => {
    const text = terminal.getSelection()
    if (!text) return false
    copy(text)
    terminal.clearSelection()
    return true
  }
  const readClipboard = async (): Promise<void> => {
    try {
      const text = options.readText ? await options.readText() : await clipboard?.readText()
      if (text && !disposed) terminal.paste(text)
    } catch { /* Clipboard permission may be denied by the browser. */ }
  }
  let disposed = false
  const paste = (event: ClipboardEvent): void => {
    event.preventDefault()
    event.stopImmediatePropagation()
    const text = event.clipboardData?.getData('text/plain')
    if (text) terminal.paste(text)
    else void readClipboard()
  }
  container.addEventListener('paste', paste, true)
  terminal.attachCustomKeyEventHandler(event => {
    if (event.type !== 'keydown') return true
    if (event.ctrlKey && !event.altKey && !event.metaKey) {
      if (event.code === 'KeyC' || event.code === 'KeyX') {
        if (copySelection() || (event.code === 'KeyC' && event.shiftKey)) {
          event.preventDefault()
          event.stopPropagation()
          return false
        }
      }
      if (event.code === 'KeyV') return false
    }
    return options.handleKey?.(event) ?? true
  })

  let dismissMenu: (() => void) | undefined
  const contextMenu = (event: MouseEvent): void => {
    event.preventDefault()
    event.stopImmediatePropagation()
    openMenus.get(document)?.()
    const menu = document.createElement('div')
    menu.className = 'terminal-context-menu'
    menu.setAttribute('role', 'menu')
    const close = (): void => {
      menu.remove()
      document.removeEventListener('mousedown', outside, true)
      document.removeEventListener('keydown', keydown, true)
      if (openMenus.get(document) === close) openMenus.delete(document)
      dismissMenu = undefined
    }
    const outside = (event: MouseEvent): void => {
      if (!menu.contains(event.target as Node)) close()
    }
    const keydown = (event: KeyboardEvent): void => {
      if (event.code !== 'Escape') return
      event.preventDefault()
      event.stopImmediatePropagation()
      close()
      terminal.focus()
    }
    const item = (label: string, icon: string, action: () => void, disabled = false): void => {
      const button = document.createElement('button')
      button.type = 'button'
      button.setAttribute('role', 'menuitem')
      button.disabled = disabled
      const glyph = document.createElement('i')
      glyph.className = `fas fa-${icon}`
      glyph.setAttribute('aria-hidden', 'true')
      button.append(glyph, document.createTextNode(label))
      button.onclick = () => { action(); close(); terminal.focus() }
      menu.appendChild(button)
    }
    item('Copy', 'copy', copySelection, !terminal.hasSelection())
    item('Paste', 'paste', () => { void readClipboard() })
    item('Select All', 'check-double', () => terminal.selectAll())
    document.body.appendChild(menu)
    const bounds = menu.getBoundingClientRect()
    menu.style.left = `${Math.max(0, Math.min(event.clientX, (document.defaultView?.innerWidth ?? event.clientX + bounds.width) - bounds.width))}px`
    menu.style.top = `${Math.max(0, Math.min(event.clientY, (document.defaultView?.innerHeight ?? event.clientY + bounds.height) - bounds.height))}px`
    document.addEventListener('mousedown', outside, true)
    document.addEventListener('keydown', keydown, true)
    openMenus.set(document, close)
    dismissMenu = close
  }
  container.addEventListener('contextmenu', contextMenu, true)

  return () => {
    disposed = true
    dismissMenu?.()
    container.removeEventListener('paste', paste, true)
    container.removeEventListener('contextmenu', contextMenu, true)
  }
}
