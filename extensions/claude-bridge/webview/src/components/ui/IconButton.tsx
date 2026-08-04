import type { JSX } from 'preact'

interface Props {
  icon: string                // FontAwesome class without 'fa-' prefix
  onClick: (e: MouseEvent) => void
  tooltip?: string            // shown via the project-wide CSS data-tooltip system
  size?: 'sm' | 'md'          // sm = 18px, md = 24px (default sm)
  variant?: 'ghost' | 'subtle' // ghost = no bg until hover, subtle = always-on dim bg
  ariaLabel?: string
  disabled?: boolean
  style?: string              // optional extra inline overrides (kept escape hatch)
}

/** Atomic icon-only button — close `×`, expand `▾`, etc. Centralizes the
 *  hover / focus / disabled visual rhythm so each call-site stops inventing
 *  its own padding/colors. Uses var() tokens throughout. */
export function IconButton({
  icon, onClick, tooltip, size = 'sm', variant = 'ghost', ariaLabel, disabled, style,
}: Props): JSX.Element {
  const dim = size === 'sm' ? '18px' : '24px'
  const fs = size === 'sm' ? 'var(--fs-10)' : 'var(--fs-base)'
  const baseBg = variant === 'subtle' ? 'var(--tint-muted-soft)' : 'transparent'
  return (
    <button
      type="button"
      onClick={(e: MouseEvent) => { if (!disabled) onClick(e) }}
      data-tooltip={tooltip}
      aria-label={ariaLabel || tooltip}
      disabled={disabled}
      style={`
        width:${dim};height:${dim};
        display:inline-flex;align-items:center;justify-content:center;
        background:${baseBg};
        border:none;border-radius:var(--radius-sm);
        color:var(--text-muted);font-size:${fs};
        cursor:${disabled ? 'not-allowed' : 'pointer'};
        opacity:${disabled ? 0.4 : 1};
        padding:0;flex-shrink:0;
        transition:background var(--transition-fast),color var(--transition-fast);
        ${style || ''}
      `}
      onMouseEnter={(e: MouseEvent) => {
        if (disabled) return
        const t = e.currentTarget as HTMLElement
        t.style.background = 'var(--bg-surface)'
        t.style.color = 'var(--text-primary)'
      }}
      onMouseLeave={(e: MouseEvent) => {
        if (disabled) return
        const t = e.currentTarget as HTMLElement
        t.style.background = baseBg
        t.style.color = 'var(--text-muted)'
      }}
    >
      <i class={`fas ${icon}`} />
    </button>
  )
}
