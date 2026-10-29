import type { ConsoleInteractionTerminal } from './types'

export const CONSOLE_SCROLLBACK = 5000
// xterm and FitAddon read this same public option for scrollbar hit testing
// and column allocation. CSS must not narrow the hit target independently.
export const CONSOLE_OVERVIEW_RULER = { width: 8, showTopBorder: false, showBottomBorder: false }

/** Обычный drag принадлежит терминалу; Alt+drag — приложению. */
export function configureTerminalInteraction(terminal: ConsoleInteractionTerminal, container: HTMLElement): () => void {
  container.classList.add('bridge-console')
  const forwarded = new WeakSet<Event>()
  const document = container.ownerDocument
  const mac = /Mac/.test(document.defaultView?.navigator.platform ?? '')
  let gesture: { down: MouseEvent; target: Element; dragged: boolean; click: boolean } | null = null
  let applicationTarget: Element | null = null
  const dispatch = (target: Element, type: string, source: MouseEvent, forceSelection: boolean): void => {
    const next = new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      view: source.view,
      detail: source.detail,
      screenX: source.screenX,
      screenY: source.screenY,
      clientX: source.clientX,
      clientY: source.clientY,
      button: source.button,
      buttons: source.buttons,
      ctrlKey: source.ctrlKey,
      metaKey: source.metaKey,
      shiftKey: forceSelection && !mac,
      altKey: forceSelection && mac,
    })
    forwarded.add(next)
    target.dispatchEvent(next)
  }
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
    const reporting = terminal.modes.mouseTrackingMode !== 'none'
    if (!reporting) return
    event.preventDefault()
    event.stopImmediatePropagation()
    terminal.focus()
    if (event.altKey) {
      // Alt — модификатор маршрутизации, а не часть протокола CLI.
      applicationTarget = target
      dispatch(target, 'mousedown', event, false)
      return
    }
    gesture = { down: event, target, dragged: false, click: !event.shiftKey && event.detail === 1 }
    // Принудительное выделение — публичный механизм xterm; режим CLI не меняем.
    dispatch(target, 'mousedown', event, true)
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
  const documentMove = (event: MouseEvent): void => {
    if (forwarded.has(event)) return
    if (applicationTarget) {
      event.preventDefault()
      event.stopImmediatePropagation()
      dispatch(applicationTarget, 'mousemove', event, false)
      return
    }
    if (gesture && (event.clientX !== gesture.down.clientX || event.clientY !== gesture.down.clientY)) {
      gesture.dragged = true
    }
  }
  const documentUp = (event: MouseEvent): void => {
    if (forwarded.has(event) || event.button !== 0) return
    if (applicationTarget) {
      const target = applicationTarget
      applicationTarget = null
      event.preventDefault()
      event.stopImmediatePropagation()
      dispatch(target, 'mouseup', event, false)
      return
    }
    if (!gesture) return
    const finished = gesture
    gesture = null
    // Отложенный короткий клик сохраняет кнопки CLI без начала его drag-выделения.
    if (finished.click && !finished.dragged && !terminal.hasSelection() && finished.target.isConnected) {
      dispatch(finished.target, 'mousedown', finished.down, false)
      dispatch(finished.target, 'mouseup', event, false)
    }
  }
  container.addEventListener('mousedown', mouseDown, true)
  container.addEventListener('mouseup', mouseUp, true)
  container.addEventListener('mousemove', mouseMove, true)
  document.addEventListener('mousemove', documentMove, true)
  document.addEventListener('mouseup', documentUp, true)

  const syncScreen = (): void => {
    container.dataset.terminalScreen = terminal.buffer.active.type
    // При mouse reporting колесом владеет CLI, а не сохранённой историей xterm.
    const mode = terminal.modes.mouseTrackingMode
    container.dataset.terminalHistory = String(terminal.buffer.active.baseY > 0 && (mode === 'none' || mode === 'x10'))
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
    document.removeEventListener('mousemove', documentMove, true)
    document.removeEventListener('mouseup', documentUp, true)
    gesture = null
    applicationTarget = null
    subscriptions.forEach((subscription) => subscription.dispose())
    container.classList.remove('bridge-console')
    delete container.dataset.terminalScreen
    delete container.dataset.terminalHistory
  }
}
