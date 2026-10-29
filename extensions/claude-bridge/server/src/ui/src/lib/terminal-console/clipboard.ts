import type { ConsoleClipboardTerminal } from './types'

export interface ClipboardOptions {
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
  try {
    textarea.select()
    document.execCommand('copy')
  } finally {
    textarea.remove()
    previous?.focus()
  }
}

function writeClipboard(document: Document, options: ClipboardOptions, text: string): void | Promise<void> {
  if (options.writeText) return Promise.resolve(options.writeText(text)).then(() => {})
  const clipboard = document.defaultView?.navigator.clipboard
  if (clipboard?.writeText) return clipboard.writeText(text).catch(() => fallbackCopy(document, text))
  fallbackCopy(document, text)
}

/** OSC 52 записывает через тот же clipboard, что и меню. Чтение из PTY не разрешено. */
export function createOsc52ClipboardProvider(document: Document, options: ClipboardOptions = {}) {
  return {
    readText: () => '',
    writeText: (selection: string, text: string): void | Promise<void> => {
      if (selection === 'c' || selection === '') return writeClipboard(document, options, text)
    },
  }
}

/** One clipboard/menu policy for the console. Native paste is the sole keyboard
 * paste path, avoiding a second send from an asynchronous clipboard read. */
export function configureTerminalClipboard(
  terminal: ConsoleClipboardTerminal,
  container: HTMLElement,
  options: ClipboardOptions = {},
): () => void {
  const document = container.ownerDocument
  const clipboard = document.defaultView?.navigator.clipboard
  const copy = (text: string): void => {
    void writeClipboard(document, options, text)
  }
  const copySelection = (): boolean => {
    const text = terminal.getSelection()
    if (!text) return false
    copy(text)
    terminal.clearSelection()
    return true
  }
  const clipboardText = async (): Promise<string> => {
    try {
      if (options.readText) return await options.readText()
      // Inspecting a menu must not trigger a browser permission prompt. The
      // native console supplies the host reader; browser clients can still
      // paste with the trusted clipboard event when permission is unavailable.
      const permission = await document.defaultView?.navigator.permissions?.query({
        name: 'clipboard-read' as PermissionName,
      })
      return permission?.state === 'granted' ? ((await clipboard?.readText()) ?? '') : ''
    } catch {
      return ''
    }
  }
  const readClipboard = async (): Promise<void> => {
    const text = await clipboardText()
    if (text && !disposed) terminal.paste(text)
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
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== 'keydown') return true
    if (event.ctrlKey && !event.altKey && !event.metaKey) {
      // CEF can omit code when native input has no scan code. Its Windows
      // virtual key remains layout independent even when key is Cyrillic.
      const code = event.code || ({ 67: 'KeyC', 86: 'KeyV', 88: 'KeyX' } as Record<number, string>)[event.keyCode]
      if (code === 'KeyC' || code === 'KeyX') {
        if (copySelection() || (code === 'KeyC' && event.shiftKey)) {
          event.preventDefault()
          event.stopPropagation()
          return false
        }
      }
      if (code === 'KeyV') return false
    }
    return options.handleKey?.(event) ?? true
  })

  let dismissMenu: (() => void) | undefined
  const contextMenu = (event: MouseEvent): void => {
    event.preventDefault()
    event.stopImmediatePropagation()
    openMenus.get(document)?.()
    const menu = document.createElement('div')
    const selection = terminal.hasSelection() ? terminal.getSelection() : ''
    let closed = false
    menu.className = 'terminal-context-menu'
    menu.setAttribute('role', 'menu')
    const close = (): void => {
      closed = true
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
      if ((event.code || event.key) !== 'Escape') return
      event.preventDefault()
      event.stopImmediatePropagation()
      close()
      terminal.focus()
    }
    const item = (label: string, icon: string, action: () => void, disabled = false): HTMLButtonElement => {
      const button = document.createElement('button')
      button.type = 'button'
      button.setAttribute('role', 'menuitem')
      button.disabled = disabled
      const glyph = document.createElement('i')
      glyph.className = `fas fa-${icon}`
      glyph.setAttribute('aria-hidden', 'true')
      button.append(glyph, document.createTextNode(label))
      button.onclick = () => {
        action()
        close()
        terminal.focus()
      }
      menu.appendChild(button)
      return button
    }
    item(
      'Copy',
      'copy',
      () => {
        if (selection) {
          copy(selection)
          terminal.clearSelection()
        }
      },
      !selection,
    )
    let pasteText = ''
    const pasteItem = item(
      'Paste',
      'paste',
      () => {
        if (pasteText) terminal.paste(pasteText)
      },
      true,
    )
    void clipboardText().then((text) => {
      if (closed || disposed) return
      pasteText = text
      pasteItem.disabled = !text
    })
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
