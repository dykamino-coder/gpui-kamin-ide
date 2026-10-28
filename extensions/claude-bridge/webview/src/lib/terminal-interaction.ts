import type { Terminal } from '@xterm/xterm'

export const CONSOLE_SCROLLBACK = 5000

/** Keep application mouse protocols intact. Shift+drag selects text; in a
 * non-reporting screen we gate selection explicitly as well. Scrollbar gestures
 * bypass this policy and continue to use xterm's own hit testing. */
export function configureTerminalInteraction(terminal: Terminal, container: HTMLElement): () => void {
  const forwarded = new WeakSet<Event>()
  const select = (event: MouseEvent): void => {
    if (forwarded.has(event) || event.button !== 0) return
    const target = event.target as Element | null
    if (!target?.closest('.xterm-screen')) return
    // xterm already implements modifier selection in mouse-reporting modes.
    if (terminal.modes.mouseTrackingMode !== 'none') return

    event.preventDefault()
    event.stopImmediatePropagation()
    terminal.focus()
    if (!event.shiftKey) return

    // In normal mode xterm treats Shift as "extend an existing selection".
    // Forward an ordinary initial click only after the explicit modifier gate,
    // so Shift+drag also starts a fresh selection when there is no prior anchor.
    const initial = new MouseEvent('mousedown', {
      bubbles: true, cancelable: true, view: event.view, detail: event.detail,
      screenX: event.screenX, screenY: event.screenY,
      clientX: event.clientX, clientY: event.clientY,
      button: event.button, buttons: event.buttons,
      ctrlKey: event.ctrlKey, altKey: event.altKey, metaKey: event.metaKey,
    })
    forwarded.add(initial)
    target.dispatchEvent(initial)
  }
  container.addEventListener('mousedown', select, true)

  terminal.attachCustomWheelEventHandler(event => {
    const mode = terminal.modes.mouseTrackingMode
    if (terminal.buffer.active.type === 'alternate' && (mode === 'none' || mode === 'x10')) {
      // No declared wheel protocol: do not let xterm synthesize cursor keys.
      event.preventDefault()
      event.stopPropagation()
      return false
    }
    // Normal history uses xterm's viewport; reporting apps use its encoder.
    return true
  })

  return () => container.removeEventListener('mousedown', select, true)
}
