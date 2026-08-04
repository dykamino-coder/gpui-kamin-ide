import type { JSX } from 'preact'

interface QuestionOption { label?: string; description?: string }
interface Question {
  header?: string
  question?: string
  options?: QuestionOption[]
  multiSelect?: boolean
}

/** Исторический рендер AskUserQuestion (реплей транскрипта). Живой вопрос
 *  рисует ElicitationWidget; в истории тот же tool_use падал в generic
 *  JSON-блок — вопрос и опции были нечитаемы (аудит транскрипта 968683a8,
 *  18 вхождений). Ответ юзера приходит следующим user-сообщением, поэтому
 *  здесь только сам вопрос с опциями. */
export function AskUserQuestionRender({ input }: { input: any }): JSX.Element {
  const questions: Question[] = Array.isArray(input?.questions) ? input.questions : []
  if (questions.length === 0) return <>{JSON.stringify(input, null, 2)}</>
  return (
    <div style="display:flex;flex-direction:column;gap:8px;white-space:normal">
      {questions.map((q, qi) => (
        <div key={qi} style="border:1px solid var(--border-subtle);border-radius:var(--radius-sm);padding:8px 10px">
          <div style="display:flex;align-items:baseline;gap:6px;margin-bottom:4px">
            {q.header && (
              <span style="padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-purple-medium);color:var(--accent-purple);font-size:9px;font-weight:600;text-transform:uppercase;letter-spacing:0.03em;flex-shrink:0">
                {q.header}
              </span>
            )}
            {q.multiSelect && (
              <span style="color:var(--text-muted);font-size:10px;flex-shrink:0">multi-select</span>
            )}
          </div>
          {q.question && <div style="white-space:pre-wrap;margin-bottom:6px">{q.question}</div>}
          {Array.isArray(q.options) && q.options.length > 0 && (
            <div style="display:flex;flex-direction:column;gap:4px">
              {q.options.map((o, oi) => (
                <div key={oi} style="padding:4px 8px;border-radius:var(--radius-xs);background:var(--bg-tint-weak, rgba(255,255,255,0.04))">
                  <div style="font-weight:600;font-size:12px">{o.label ?? ''}</div>
                  {o.description && (
                    <div style="color:var(--text-muted);font-size:11px;white-space:pre-wrap">{o.description}</div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
