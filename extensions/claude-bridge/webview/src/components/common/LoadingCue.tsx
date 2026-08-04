import type { JSX, ComponentChildren } from 'preact'
import kaminoidRaw from '../../assets/kaminoid.svg?raw'

// The ONE loading visual — the same brand loader the KaminIDE shell draws for
// its panel skeletons and the chat switch cover: the K mark with a breathing
// radial glow over a running progress bar. Every panel-scale wait (boot
// restore, conversation load, console connect) renders THIS, so loading reads
// as one system state instead of a zoo of different placeholders.

const STYLE = `
@keyframes kamin-breathe { 0%,100% { opacity:.5; transform:scale(.94) } 50% { opacity:1; transform:scale(1.06) } }
@keyframes kamin-float { 0%,100% { transform:translateY(0) } 50% { transform:translateY(-4px) } }
@keyframes kamin-sweep { 0% { transform:translateX(-100%) } 100% { transform:translateX(100%) } }
.kamin-loader-logo svg { width:100%; height:100%; display:block }
`

export function LoadingCue({ title, sub, pct, compact, children }: {
  title: string
  sub?: string
  /** Determinate progress 0-100; omitted = indeterminate running bar. */
  pct?: number
  /** Smaller brand mark for tight surfaces (console overlay). */
  compact?: boolean
  children?: ComponentChildren
}): JSX.Element {
  const brand = compact ? 56 : 96
  const logo = compact ? 38 : 64
  return (
    <>
      <style>{STYLE}</style>
      <div style={`position:relative;width:${String(brand)}px;height:${String(brand)}px`}>
        <div style="position:absolute;inset:-28%;border-radius:50%;background:radial-gradient(circle, color-mix(in srgb, var(--accent-primary) 26%, transparent) 0%, transparent 66%);animation:kamin-breathe 2.4s ease-in-out infinite" />
        <div
          class="kamin-loader-logo"
          style={`position:absolute;left:50%;top:50%;width:${String(logo)}px;height:${String(logo)}px;margin-left:-${String(logo / 2)}px;margin-top:-${String(logo / 2)}px;animation:kamin-float 2.4s ease-in-out infinite`}
          dangerouslySetInnerHTML={{ __html: kaminoidRaw }}
        />
      </div>
      <span style="color:var(--text-muted);font-size:var(--fs-md);font-weight:500">
        {title}{pct !== undefined ? ` ${String(pct)}%` : ''}
      </span>
      <div style={`width:${compact ? '140' : '180'}px;height:3px;border-radius:999px;overflow:hidden;background:color-mix(in srgb, var(--text-primary) 8%, transparent)`}>
        {pct !== undefined ? (
          <div style={`width:${String(pct)}%;height:100%;background:var(--accent-primary);transition:width .2s`} />
        ) : (
          <div style="width:100%;height:100%;background:linear-gradient(90deg, transparent, var(--accent-primary), transparent);animation:kamin-sweep 1.15s ease-in-out infinite" />
        )}
      </div>
      {sub && (
        <span style="color:var(--text-disabled);font-size:12px;text-align:center;max-width:240px;line-height:1.5">{sub}</span>
      )}
      {children}
    </>
  )
}
