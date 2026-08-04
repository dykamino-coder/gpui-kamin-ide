import type { ComponentChildren } from 'preact'

interface WidgetActionsProps {
  children: ComponentChildren
}

export function WidgetActions({ children }: WidgetActionsProps) {
  return (
    <div class="widget-actions">
      {children}
    </div>
  )
}
