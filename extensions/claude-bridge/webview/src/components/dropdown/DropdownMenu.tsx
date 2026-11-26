import type { JSX, RefObject } from 'preact'
import type { ComponentChildren } from 'preact'
import { useLayoutEffect, useRef } from 'preact/hooks'

interface DropdownMenuProps {
  children: ComponentChildren
  anchorRef: RefObject<HTMLElement>
  open: boolean
  align?: 'left' | 'right'
}

/** Gap between the menu and the viewport edge / its trigger, px. */
const EDGE = 4

/** Fixed-position menu that opens above its trigger. The content can change
 *  while it is open (the GPT catalog arrives after the first open), so the
 *  placement is recomputed whenever the menu or the window resizes, and the
 *  height is capped to the room above the trigger: a list taller than that
 *  scrolls inside the menu instead of running off the viewport. */
export function DropdownMenu({ children, anchorRef, open, align = 'left' }: DropdownMenuProps): JSX.Element {
  const menuRef = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    if (!open || !anchorRef.current || !menuRef.current) return
    const trigger = anchorRef.current
    const menu = menuRef.current

    function place(): void {
      const rect = trigger.getBoundingClientRect()
      // Room above the trigger; the cap only bounds the height, so setting it
      // again with the same value does not resize the menu (no observer loop).
      const room = Math.max(0, rect.top - 2 * EDGE)
      menu.style.maxHeight = `${room}px`
      const menuRect = menu.getBoundingClientRect()
      const menuH = Math.min(menuRect.height, room)
      const menuW = menuRect.width || 270

      menu.style.top = `${Math.max(EDGE, rect.top - menuH - EDGE)}px`
      menu.style.bottom = 'auto'

      let left = align === 'right' ? rect.right - menuW : rect.left
      if (left + menuW > window.innerWidth - EDGE) left = window.innerWidth - menuW - EDGE
      if (left < EDGE) left = EDGE
      menu.style.left = `${left}px`
    }

    place()
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(place)
    ro?.observe(menu)
    window.addEventListener('resize', place)
    return () => {
      ro?.disconnect()
      window.removeEventListener('resize', place)
    }
  }, [open, anchorRef, align])

  return (
    <div ref={menuRef} class={`perm-menu${open ? ' visible' : ''}`}>
      {children}
    </div>
  )
}
