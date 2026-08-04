import type { ComponentChildren } from 'preact'

interface WidgetBodyProps {
  children: ComponentChildren
}

export function WidgetBody({ children }: WidgetBodyProps) {
  return (
    <div class="widget-body">
      {children}
    </div>
  )
}
