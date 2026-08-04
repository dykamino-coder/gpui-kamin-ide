import type { JSX } from 'preact'
import { useEffect } from 'preact/hooks'

// Bridge tooltips are drawn by the HOST, not here. A sandboxed webview iframe
// clips its own overflow, so a tooltip in the narrow Plan/Todos/Console tools
// (or near a panel edge) can't spill past the iframe. Instead we post the
// target's rect (in this frame's viewport coords) + text to the parent
// (WebviewPanelView), which translates to host-window coords and renders the
// single host <Tooltip/> — unclipped, using the full window to measure + clamp.
// This component renders nothing; it only wires the hover → postMessage bridge.
function postTooltip(text: string, rect?: DOMRect): void {
  try {
    parent.postMessage(
      { __kaminTooltip: true, text, rect: rect ? { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height } : undefined },
      '*',
    )
  } catch { /* not embedded in a host that handles it — no tooltip */ }
}

export function Tooltip(): JSX.Element | null {
  useEffect(() => {
    function handlePointerEnter(e: PointerEvent): void {
      const el = e.target as HTMLElement | null
      if (!el || typeof el.closest !== 'function') return
      const target = el.closest('[data-tooltip]') as HTMLElement | null
      if (!target) return
      const text = target.getAttribute('data-tooltip')
      if (!text) return
      postTooltip(text, target.getBoundingClientRect())
    }

    function handlePointerLeave(e: PointerEvent): void {
      const el = e.target as HTMLElement | null
      if (el && typeof el.closest === 'function' && el.closest('[data-tooltip]')) postTooltip('')
    }

    function hide(): void { postTooltip('') }

    document.addEventListener('pointerenter', handlePointerEnter, true)
    document.addEventListener('pointerleave', handlePointerLeave, true)
    window.addEventListener('blur', hide)
    window.addEventListener('scroll', hide, true)
    document.addEventListener('visibilitychange', hide)
    document.addEventListener('mousedown', hide)

    return () => {
      postTooltip('')
      document.removeEventListener('pointerenter', handlePointerEnter, true)
      document.removeEventListener('pointerleave', handlePointerLeave, true)
      window.removeEventListener('blur', hide)
      window.removeEventListener('scroll', hide, true)
      document.removeEventListener('visibilitychange', hide)
      document.removeEventListener('mousedown', hide)
    }
  }, [])

  return null
}
