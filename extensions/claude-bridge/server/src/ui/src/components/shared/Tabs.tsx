import styles from './Tabs.module.css'

export interface Tab {
  id: string
  label: string
  icon?: string
  badge?: number
  disabled?: boolean
}

interface TabsProps {
  tabs: Tab[]
  activeTab: string
  onChange: (tabId: string) => void
  variant?: 'pills' | 'underline' | 'bordered'
  size?: 'sm' | 'md'
  fullWidth?: boolean
  className?: string
}

export function Tabs({
  tabs,
  activeTab,
  onChange,
  variant = 'pills',
  size = 'md',
  fullWidth = false,
  className = '',
}: TabsProps) {
  const containerClass = [
    styles.tabs, styles[variant], styles[size],
    fullWidth && styles.fullWidth, className,
  ].filter(Boolean).join(' ')

  return (
    <div class={containerClass}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          class={[styles.tab, activeTab === tab.id && styles.active, tab.disabled && styles.disabled].filter(Boolean).join(' ')}
          onClick={() => !tab.disabled && onChange(tab.id)}
          disabled={tab.disabled}
        >
          {tab.icon && <i class={tab.icon} />}
          <span>{tab.label}</span>
          {tab.badge !== undefined && tab.badge > 0 && (
            <span class={styles.badge}>{tab.badge}</span>
          )}
        </button>
      ))}
    </div>
  )
}
