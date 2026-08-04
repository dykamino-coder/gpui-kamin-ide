// Pointer-based tab drag state + handlers (Sprint 5 / Stage E1).
// Extracted from `TabsBar.tsx` so the visual component shrinks below the
// 300-LOC threshold and the drag logic becomes testable / reusable.
//
// Why pointer events instead of HTML5 DnD? `useDragDrop.ts` installs
// document-level drop listeners for image attachments — they swallow drop
// events whose payload type doesn't match, so HTML5 DnD on tabs never
// resolved. Pointer events skip the platform's DnD pipeline entirely.

import { useRef, useState } from 'preact/hooks'

const DRAG_THRESHOLD_PX = 5

interface DragState {
  draggingId: string | null
  dndOverId: string | null
  dndOverSide: 'left' | 'right' | null
}

interface DragHandlers {
  onPointerDown: (e: PointerEvent, tabId: string) => void
  onPointerMove: (e: PointerEvent) => void
  onPointerUp: (e: PointerEvent) => void
  onPointerCancel: () => void
}

/** Pointer-driven tab reorder. `stripRef` must point at the scrollable
 *  strip element; `onDrop(srcId, dstId, after)` is called once the user
 *  releases over a target tab past the drag threshold. */
export function useTabDrag(
  stripRef: { current: HTMLElement | null },
  onDrop: (srcId: string, dstId: string, after: boolean) => void,
): DragState & { handlers: DragHandlers } {
  const [draggingId, setDraggingId] = useState<string | null>(null)
  const [dndOverId, setDndOverId] = useState<string | null>(null)
  const [dndOverSide, setDndOverSide] = useState<'left' | 'right' | null>(null)
  // Mirror state in a ref — state updates are async so the first few
  // pointermove frames after a fresh drag would see stale closure values.
  const draggingRef = useRef<string | null>(null)
  const pointerStartRef = useRef<{ id: string; x: number; y: number; captured: boolean } | null>(null)

  const cleanDrag = (): void => {
    pointerStartRef.current = null
    draggingRef.current = null
    setDraggingId(null)
    setDndOverId(null)
    setDndOverSide(null)
  }

  function hitTest(clientX: number): { id: string | null; side: 'left' | 'right' | null } {
    const stripEl = stripRef.current
    if (!stripEl) return { id: null, side: null }
    const children = Array.from(stripEl.querySelectorAll<HTMLElement>('[data-tab-id]'))
    for (const el of children) {
      const r = el.getBoundingClientRect()
      if (clientX >= r.left && clientX <= r.right) {
        const side: 'left' | 'right' = (clientX - r.left) < r.width / 2 ? 'left' : 'right'
        return { id: el.getAttribute('data-tab-id'), side }
      }
    }
    // Out of range — snap to the nearest edge tab so drop-in-gap still works.
    if (children.length === 0) return { id: null, side: null }
    const first = children[0]!.getBoundingClientRect()
    if (clientX < first.left) return { id: children[0]!.getAttribute('data-tab-id'), side: 'left' }
    const last = children[children.length - 1]!.getBoundingClientRect()
    if (clientX > last.right) return { id: children[children.length - 1]!.getAttribute('data-tab-id'), side: 'right' }
    return { id: null, side: null }
  }

  function onPointerDown(e: PointerEvent, tabId: string): void {
    // Only left button, and ignore if the click originated on a button child
    // (close / pin) — those handle their own click-through.
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('button')) return
    pointerStartRef.current = { id: tabId, x: e.clientX, y: e.clientY, captured: false }
  }

  function onPointerMove(e: PointerEvent): void {
    const start = pointerStartRef.current
    if (!start) return
    if (!start.captured) {
      const dx = Math.abs(e.clientX - start.x)
      const dy = Math.abs(e.clientY - start.y)
      if (dx < DRAG_THRESHOLD_PX && dy < DRAG_THRESHOLD_PX) return
      // Passed threshold — officially dragging. Capture pointer on the strip
      // so move/up events stay routed even when the cursor leaves it.
      const stripEl = stripRef.current
      stripEl?.setPointerCapture?.(e.pointerId)
      start.captured = true
      draggingRef.current = start.id
      setDraggingId(start.id)
    }
    const hit = hitTest(e.clientX)
    setDndOverId(hit.id && hit.id !== start.id ? hit.id : null)
    setDndOverSide(hit.side)
  }

  function onPointerUp(e: PointerEvent): void {
    const start = pointerStartRef.current
    if (!start) return
    const wasDragging = start.captured
    const srcId = start.id
    if (wasDragging) {
      const hit = hitTest(e.clientX)
      if (hit.id && hit.id !== srcId) onDrop(srcId, hit.id, hit.side === 'right')
      const stripEl = stripRef.current
      try { stripEl?.releasePointerCapture?.(e.pointerId) } catch { /* ignore */ }
    }
    cleanDrag()
  }

  function onPointerCancel(): void {
    cleanDrag()
  }

  return {
    draggingId,
    dndOverId,
    dndOverSide,
    handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel },
  }
}
