import type { JSX } from 'preact'
import { usePanelResize } from '../../hooks/usePanelResize'
import styles from './TerminalPanel.module.css'

interface TerminalResizeHandleProps {
  onResize: (delta: number) => void
}

export function TerminalResizeHandle({ onResize }: TerminalResizeHandleProps): JSX.Element {
  const { handleRef, isDragging } = usePanelResize({ direction: 'vertical', onResize })

  return (
    <div
      ref={handleRef as any}
      class={`${styles.resizeHandle} ${isDragging ? styles.resizeHandleDragging : ''}`}
    />
  )
}
