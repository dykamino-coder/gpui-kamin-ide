import type { JSX } from 'preact'
import { useState } from 'preact/hooks'

/** Исторический рендер Agent-тула (спавн субагента). Раньше — сырой JSON;
 *  теперь имя + тип + описание карточкой, промпт раскрывается по клику
 *  (он длинный и нужен редко — живой ход агента смотрят в панели Agents). */
export function AgentToolRender({ input }: { input: any }): JSX.Element {
  const [promptOpen, setPromptOpen] = useState(false)
  const name = typeof input?.name === 'string' ? input.name : ''
  const subagentType = typeof input?.subagent_type === 'string' ? input.subagent_type : ''
  const description = typeof input?.description === 'string' ? input.description : ''
  const prompt = typeof input?.prompt === 'string' ? input.prompt : ''
  if (!name && !subagentType && !description && !prompt) {
    return <>{JSON.stringify(input, null, 2)}</>
  }
  return (
    <div style="white-space:normal;display:flex;flex-direction:column;gap:6px">
      <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap">
        <i class="fas fa-robot" style="color:var(--accent-purple);font-size:11px" />
        {name && <span style="font-weight:600;font-size:12px">{name}</span>}
        {subagentType && (
          <span style="padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-purple-medium);color:var(--accent-purple);font-size:9px;font-weight:600;text-transform:uppercase;letter-spacing:0.03em">
            {subagentType}
          </span>
        )}
      </div>
      {description && <div style="color:var(--text-secondary);font-size:12px">{description}</div>}
      {prompt && (
        <div>
          <span
            onClick={(e) => { e.stopPropagation(); setPromptOpen(!promptOpen) }}
            style="cursor:pointer;color:var(--text-muted);font-size:11px;user-select:none"
          >
            <i class={`fas fa-chevron-${promptOpen ? 'down' : 'right'}`} style="font-size:9px;margin-right:4px" />
            prompt · {prompt.length} chars
          </span>
          {promptOpen && (
            <div style="white-space:pre-wrap;font-size:11px;color:var(--text-secondary);margin-top:4px;max-height:300px;overflow-y:auto;border-left:2px solid var(--border-subtle);padding-left:8px">
              {prompt}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
