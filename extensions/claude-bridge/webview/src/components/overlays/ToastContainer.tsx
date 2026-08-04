import type { JSX } from 'preact'
import { toasts, dismissToast } from '../../signals/toasts'

const ICONS: Record<string, string> = {
  success: 'fas fa-circle-check',
  error: 'fas fa-circle-xmark',
  info: 'fas fa-circle-info',
}

const COLORS: Record<string, string> = {
  success: 'var(--accent-green)',
  error: 'var(--accent-red)',
  info: 'var(--accent-primary)',
}

export function ToastContainer(): JSX.Element {
  const items = toasts.value

  if (items.length === 0) return <div />

  return (
    <div style="position:fixed;bottom:60px;right:16px;z-index:99998;display:flex;flex-direction:column-reverse;gap:8px;max-width:360px">
      {items.map(t => (
        <div
          key={t.id}
          style={`
            background:var(--bg-primary);border:1px solid ${COLORS[t.type]}40;border-left:3px solid ${COLORS[t.type]};
            border-radius:var(--radius-sm);padding:10px 14px;box-shadow:var(--shadow-dropdown);
            display:flex;align-items:flex-start;gap:10px;animation:slideInRight 0.2s ease-out;
          `}
        >
          <i class={ICONS[t.type]} style={`color:${COLORS[t.type]};font-size: var(--fs-md);margin-top:2px;flex-shrink:0`} />
          <div style="flex:1;min-width:0">
            <div style="font-size:12px;font-weight:600;color:var(--text-primary)">{t.title}</div>
            {t.message && <div style="font-size:11px;color:var(--text-muted);margin-top:2px;word-break:break-word">{t.message}</div>}
          </div>
          <button
            onClick={() => dismissToast(t.id)}
            style="background:none;border:none;color:var(--text-disabled);cursor:pointer;font-size:11px;padding:0;flex-shrink:0"
          >
            <i class="fas fa-xmark" />
          </button>
        </div>
      ))}
    </div>
  )
}
