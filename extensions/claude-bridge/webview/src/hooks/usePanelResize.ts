import { useRef, useState, useEffect } from 'preact/hooks'

interface UsePanelResizeArgs {
  direction: 'horizontal' | 'vertical'
  onResize: (delta: number) => void
}

interface UsePanelResizeReturn {
  handleRef: { current: HTMLDivElement | null }
  isDragging: boolean
}

export function usePanelResize({ direction, onResize }: UsePanelResizeArgs): UsePanelResizeReturn {
  const handleRef = useRef<HTMLDivElement>(null) as { current: HTMLDivElement | null }
  const [isDragging, setIsDragging] = useState(false)
  const resizingRef = useRef(false)
  const lastPosRef = useRef(0)
  // Keep the latest onResize in a ref so we don't re-bind listeners on every
  // render. Previously every state change inside onResize re-ran the effect,
  // which removed the global mousemove listener mid-drag — the cursor stopped
  // after the first millimetre.
  const onResizeRef = useRef(onResize)
  onResizeRef.current = onResize

  useEffect(() => {
    const handle = handleRef.current
    if (!handle) return

    const onMouseMove = (e: MouseEvent): void => {
      if (!resizingRef.current) return
      const current = direction === 'horizontal' ? e.clientY : e.clientX
      const delta = current - lastPosRef.current
      lastPosRef.current = current
      onResizeRef.current(delta)
    }

    const onMouseUp = (): void => {
      resizingRef.current = false
      setIsDragging(false)
      document.removeEventListener('mousemove', onMouseMove)
      document.removeEventListener('mouseup', onMouseUp)
    }

    const onMouseDown = (e: MouseEvent): void => {
      e.preventDefault()
      resizingRef.current = true
      setIsDragging(true)
      lastPosRef.current = direction === 'horizontal' ? e.clientY : e.clientX
      document.addEventListener('mousemove', onMouseMove)
      document.addEventListener('mouseup', onMouseUp)
    }

    handle.addEventListener('mousedown', onMouseDown)
    return () => {
      handle.removeEventListener('mousedown', onMouseDown)
      document.removeEventListener('mousemove', onMouseMove)
      document.removeEventListener('mouseup', onMouseUp)
    }
    // Intentionally depend only on direction — re-rendering must NOT tear
    // down the drag listeners (which would stop the drag mid-gesture).
  }, [direction])

  return { handleRef, isDragging }
}
