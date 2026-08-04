interface WidgetButtonProps {
  variant: 'accept' | 'deny' | 'dismiss'
  onClick: () => void
  label: string
  disabled?: boolean
}

export function WidgetButton({ variant, onClick, label, disabled }: WidgetButtonProps) {
  return (
    <button
      class={`widget-btn ${variant}`}
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </button>
  )
}
