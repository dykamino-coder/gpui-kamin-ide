interface WidgetHeaderProps {
  icon: string
  title: string
  typeBadge: string
}

export function WidgetHeader({ icon, title, typeBadge }: WidgetHeaderProps) {
  const isFA = icon.startsWith('fa')
  return (
    <div class="widget-header">
      <span class="widget-icon">
        {isFA ? <i class={icon} /> : icon}
      </span>
      <span class="widget-title">{title}</span>
      <span class="widget-type-badge">{typeBadge}</span>
    </div>
  )
}
