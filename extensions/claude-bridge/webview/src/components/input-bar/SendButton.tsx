import type { JSX } from 'preact'

export type SendMode = 'send' | 'queue' | 'stop'

interface SendButtonProps {
  mode: SendMode
  queueCount: number
  onClick: () => void
}

const CONFIG: Record<SendMode, { icon: string; tooltip: string; className: string }> = {
  send:  { icon: 'fas fa-arrow-up',     tooltip: 'Send (Enter)',           className: 'send-btn' },
  queue: { icon: 'fas fa-layer-group',   tooltip: 'Queue — will send when ready', className: 'send-btn send-btn-queue' },
  stop:  { icon: 'fas fa-stop',          tooltip: 'Stop (interrupt)',       className: 'send-btn send-btn-stop' },
}

export function SendButton({ mode, queueCount, onClick }: SendButtonProps): JSX.Element {
  const cfg = CONFIG[mode]
  return (
    <button
      class={cfg.className}
      data-tooltip={cfg.tooltip}
      type="button"
      onClick={onClick}
      style={mode === 'send' ? undefined : undefined}
    >
      <i class={cfg.icon} />
      {mode === 'queue' && queueCount > 0 && (
        <span style="position:absolute;top:-4px;right:-4px;background:var(--accent-purple);color:var(--bg-primary);font-size:9px;font-weight:700;border-radius:50%;width:16px;height:16px;display:flex;align-items:center;justify-content:center">
          {queueCount}
        </span>
      )}
    </button>
  )
}
