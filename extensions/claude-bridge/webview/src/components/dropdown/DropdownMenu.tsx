import type { JSX, RefObject } from 'preact'
import type { ComponentChildren } from 'preact'
import { useEffect, useRef } from 'preact/hooks'

interface DropdownMenuProps {
  children: ComponentChildren
  anchorRef: RefObject<HTMLElement>
  open: boolean
  align?: 'left' | 'right'
}

export function DropdownMenu({ children, anchorRef, open, align = 'left' }: DropdownMenuProps): JSX.Element {
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open || !anchorRef.current || !menuRef.current) return

    const trigger = anchorRef.current
    const menu = menuRef.current
    const rect = trigger.getBoundingClientRect()
    const menuRect = menu.getBoundingClientRect()
    const menuH = menuRect.height
    const menuW = menuRect.width || 270

    const top = rect.top - menuH - 4
    menu.style.top = `${Math.max(4, top)}px`
    menu.style.bottom = 'auto'

    if (align === 'right') {
      let left = rect.right - menuW
      if (left < 4) left = 4
      menu.style.left = `${left}px`
    } else {
      let left = rect.left
      if (left + menuW > window.innerWidth - 4) left = window.innerWidth - menuW - 4
      if (left < 4) left = 4
      menu.style.left = `${left}px`
    }
  }, [open, anchorRef, align])

  return (
    <div ref={menuRef} class={`perm-menu${open ? ' visible' : ''}`}>
      {children}
    </div>
  )
}
