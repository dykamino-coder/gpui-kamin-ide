import type { ConsoleInteractionTerminal } from './types'

export const CONSOLE_SCROLLBACK = 5000
// xterm and FitAddon read this same public option for scrollbar hit testing
// and column allocation. CSS must not narrow the hit target independently.
export const CONSOLE_OVERVIEW_RULER = { width: 8, showTopBorder: false, showBottomBorder: false }

/** Normal-screen drag selects text. Alternate-screen applications retain mouse
 * ownership; their explicit selection modifier is handled by xterm. */
export function configureTerminalInteraction(terminal: ConsoleInteractionTerminal, container: HTMLElement): () => void {
  container.classList.add('bridge-console')
  const forwarded = new WeakSet<Event>()
  const mouseDown = (event: MouseEvent): void => {
    if (event.button === 2) {
      // The console menu owns secondary clicks, including in reporting apps.
      event.preventDefault()
      event.stopImmediatePropagation()
      terminal.focus()
      return
    }
    if (forwarded.has(event) || event.button !== 0) return
    const target = event.target as Element | null
    if (!target?.closest('.xterm-screen')) return
    const normal = terminal.buffer.active.type === 'normal'
    const reporting = terminal.modes.mouseTrackingMode !== 'none'
    if (normal && !reporting) return
    if (terminal.buffer.active.type === 'alternate' && (reporting || event.shiftKey)) {
      // Without reporting, Shift means "extend" to xterm; remove it so the
      // first modifier-drag can start a selection without an existing anchor.
      if (reporting) return
    } else if (!normal) {
      event.preventDefault()
      event.stopImmediatePropagation()
      terminal.focus()
      return
    }

    event.preventDefault()
    event.stopImmediatePropagation()
    terminal.focus()
    const mac = /Mac/.test(container.ownerDocument.defaultView?.navigator.platform ?? '')
    const initial = new MouseEvent('mousedown', {
      bubbles: true,
      cancelable: true,
      view: event.view,
      detail: event.detail,
      screenX: event.screenX,
      screenY: event.screenY,
      clientX: event.clientX,
      clientY: event.clientY,
      button: event.button,
      buttons: event.buttons,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      shiftKey: reporting && !mac,
      altKey: reporting && mac ? true : event.altKey,
    })
    forwarded.add(initial)
    target.dispatchEvent(initial)
  }
  const mouseUp = (event: MouseEvent): void => {
    if (event.button !== 2) return
    event.preventDefault()
    event.stopImmediatePropagation()
  }
  const mouseMove = (event: MouseEvent): void => {
    // xterm treats a mouse report as input and clears selection. While text
    // is selected, hovering must not send reports; clicks/drags and wheel
    // retain the application's protocol and its declared mouse modes.
    if (event.buttons === 0 && terminal.hasSelection()) event.stopImmediatePropagation()
  }
  container.addEventListener('mousedown', mouseDown, true)
  container.addEventListener('mouseup', mouseUp, true)
  container.addEventListener('mousemove', mouseMove, true)

  const syncScreen = (): void => {
    container.dataset.terminalScreen = terminal.buffer.active.type
    container.dataset.terminalHistory = String(terminal.buffer.active.baseY > 0)
  }
  syncScreen()
  const subscriptions = [
    terminal.buffer.onBufferChange(syncScreen),
    terminal.onWriteParsed(syncScreen),
    terminal.onScroll(syncScreen),
    terminal.onResize(syncScreen),
  ]
  terminal.attachCustomWheelEventHandler((event) => {
    const mode = terminal.modes.mouseTrackingMode
    if (terminal.buffer.active.type === 'alternate' && (mode === 'none' || mode === 'x10')) {
      // No declared wheel protocol: do not let xterm synthesize cursor keys.
      event.preventDefault()
      event.stopPropagation()
      return false
    }
    return true
  })

  return () => {
    container.removeEventListener('mousedown', mouseDown, true)
    container.removeEventListener('mouseup', mouseUp, true)
    container.removeEventListener('mousemove', mouseMove, true)
    subscriptions.forEach((subscription) => subscription.dispose())
    container.classList.remove('bridge-console')
    delete container.dataset.terminalScreen
    delete container.dataset.terminalHistory
  }
}
