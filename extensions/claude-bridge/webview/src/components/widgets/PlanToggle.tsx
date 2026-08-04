import { useSignal } from '@preact/signals'

interface PlanToggleProps {
  plan: string
}

export function PlanToggle({ plan }: PlanToggleProps) {
  const expanded = useSignal(false)

  function toggle() {
    expanded.value = !expanded.value
  }

  return (
    <>
      <div
        style="cursor:pointer;color:var(--accent-primary);font-size:12px;margin-top:6px;"
        onClick={toggle}
      >
        {expanded.value ? '\u25BC Hide plan' : '\u25B6 Show plan'}
      </div>
      {expanded.value && (
        <pre style="font-size:11px;color:var(--text-secondary);background:var(--bg-mantle);padding:8px;border-radius:var(--radius-sm);margin-top:6px;max-height:200px;overflow:auto;white-space:pre-wrap;word-break:break-word;">
          {plan}
        </pre>
      )}
    </>
  )
}
