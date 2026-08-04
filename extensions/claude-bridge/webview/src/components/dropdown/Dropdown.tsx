import type { JSX, RefObject } from 'preact'
import type { ComponentChildren } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import styles from './Dropdown.module.css'
import { DropdownMenu } from './DropdownMenu'

interface DropdownProps {
  trigger: ComponentChildren
  open: boolean
  onToggle: () => void
  children: ComponentChildren
  align?: 'left' | 'right'
}

// Module-level coordinator: only one dropdown stays open at a time.
// Tracks the *instance* (stable id) so closing doesn't rely on function
// identity — onToggle is recreated every render, which otherwise leaves a
// stale closer pointing at a closed dropdown and accidentally reopens it on
// the next switch.
interface OpenEntry { id: number; close: () => void }
let currentEntry: OpenEntry | null = null
let nextDropdownId = 0

export function Dropdown({ trigger, open, onToggle, children, align = 'left' }: DropdownProps): JSX.Element {
  const triggerRef = useRef<HTMLDivElement>(null)
  const onToggleRef = useRef(onToggle)
  onToggleRef.current = onToggle
  const idRef = useRef<number>(-1)
  if (idRef.current === -1) idRef.current = ++nextDropdownId

  useEffect(() => {
    if (!open) {
      if (currentEntry?.id === idRef.current) currentEntry = null
      return
    }
    // Close any other dropdown that was open before us.
    if (currentEntry && currentEntry.id !== idRef.current) {
      const prev = currentEntry
      currentEntry = null  // clear before calling so the closer's cleanup is a no-op
      try { prev.close() } catch {}
    }
    currentEntry = { id: idRef.current, close: () => onToggleRef.current() }

    function handleOutside(e: MouseEvent): void {
      const target = e.target as Node | null
      if (!target) return
      if (triggerRef.current && !triggerRef.current.contains(target)) {
        onToggleRef.current()
      }
    }
    function handleKeydown(e: KeyboardEvent): void {
      if (e.key === 'Escape') onToggleRef.current()
    }
    // We live in a sandboxed iframe: a `mousedown` in the HOST document (clicking
    // outside this iframe) never reaches us, so the dropdown would stay open. The
    // iframe window losing focus IS observable — close on it so an outside click
    // in the main app dismisses the menu too.
    function handleBlur(): void { onToggleRef.current() }
    // Capture phase so stopPropagation() on another trigger doesn't block us.
    document.addEventListener('mousedown', handleOutside, true)
    document.addEventListener('keydown', handleKeydown)
    window.addEventListener('blur', handleBlur)
    return () => {
      document.removeEventListener('mousedown', handleOutside, true)
      document.removeEventListener('keydown', handleKeydown)
      window.removeEventListener('blur', handleBlur)
      if (currentEntry?.id === idRef.current) currentEntry = null
    }
  }, [open])

  return (
    <div class={`perm-dropdown ${styles.dropdown}`} ref={triggerRef}>
      <div onClick={(e) => { e.stopPropagation(); onToggle() }}>
        {trigger}
      </div>
      <DropdownMenu anchorRef={triggerRef as RefObject<HTMLElement>} open={open} align={align}>
        {children}
      </DropdownMenu>
    </div>
  )
}
