import { useSignal } from '@preact/signals'
import { activeWidgets } from '../../signals/widgets'
import { useBridge } from '../../hooks/useBridge'
import { WidgetHeader } from './WidgetHeader'
import { WidgetBody } from './WidgetBody'
import { WidgetActions } from './WidgetActions'
import { AskUserOptions } from './AskUserOptions'
import { AskUserTextInput } from './AskUserTextInput'
import { PlanToggle } from './PlanToggle'
import styles from './AskUserWidget.module.css'

interface MultiQuestion {
  header?: string
  question: string
  options?: string[]
  /** true → выбор нескольких опций (чекбоксы вместо радио). */
  multiSelect?: boolean
}

interface AskUserWidgetProps {
  tabId: string
  data: {
    requestId: string
    question: string
    options?: string[]
    plan?: string
    isPlanApproval?: boolean
    questions?: MultiQuestion[]
  }
}

export function AskUserWidget({ data }: AskUserWidgetProps) {
  const bridge = useBridge()
  // Multi-question: array of {selectedIndex, text} per question; otherwise
  // single-question path uses items[0].
  const items: MultiQuestion[] = (data.questions && data.questions.length > 0)
    ? data.questions
    : [{ question: data.question, options: data.options }]
  const activeTab = useSignal(0)
  // `selected` — одиночный выбор (радио), `picked` — множественный (multiSelect).
  const answers = useSignal<Array<{ selected: number | null; picked: number[]; text: string }>>(
    items.map(() => ({ selected: null, picked: [], text: '' }))
  )

  const isPlan = !!data.isPlanApproval
  const isMulti = items.length > 1

  const icon = isPlan ? 'fas fa-clipboard-list' : 'fas fa-circle-question'
  const title = isPlan ? 'Plan Review' : 'Claude is asking'
  const typeBadge = isPlan ? 'Plan' : isMulti ? `${items.length} questions` : 'Question'

  function dismiss() {
    activeWidgets.value = activeWidgets.value.filter(w => w.requestId !== data.requestId)
  }

  function updateAnswer(idx: number, patch: { selected?: number | null; picked?: number[]; text?: string }) {
    const next = answers.value.slice()
    next[idx] = { ...next[idx]!, ...patch }
    answers.value = next
  }

  function togglePicked(idx: number, opt: number) {
    const cur = answers.value[idx]!.picked
    updateAnswer(idx, { picked: cur.includes(opt) ? cur.filter(o => o !== opt) : [...cur, opt] })
  }

  function pickedAnswer(q: MultiQuestion, a: { picked: number[] }): string {
    return a.picked.slice().sort((x, y) => x - y).map(o => q.options![o]!).join('; ')
  }

  function collectAnswer(): string | null {
    const parts: string[] = []
    for (let i = 0; i < items.length; i++) {
      const q = items[i]!
      const a = answers.value[i]!
      let ans = a.text.trim()
      if (!ans && q.options) {
        if (q.multiSelect && a.picked.length > 0) ans = pickedAnswer(q, a)
        else if (!q.multiSelect && a.selected !== null) ans = q.options[a.selected]!
      }
      if (!ans) return null  // a question is unanswered
      const label = q.header || q.question.split('\n')[0]!.slice(0, 60)
      parts.push(isMulti ? `${label}: ${ans}` : ans)
    }
    return parts.join('\n')
  }

  function handleApprove() {
    bridge.respondAskUser(data.requestId, 'Approved')
    dismiss()
  }

  function handleReject() {
    bridge.respondAskUser(data.requestId, 'Rejected')
    dismiss()
  }

  function handleSubmit() {
    const answer = collectAnswer()
    if (!answer) return
    bridge.respondAskUser(data.requestId, answer)
    dismiss()
  }

  function handleSkip() {
    bridge.respondAskUser(data.requestId, '[Skipped by user]')
    dismiss()
  }

  const current = items[activeTab.value]!
  const currentAnswer = answers.value[activeTab.value]!
  const optionDone = (q: MultiQuestion, a: { selected: number | null; picked: number[] }) =>
    !!q.options && (q.multiSelect ? a.picked.length > 0 : a.selected !== null)
  const allAnswered = items.every((q, i) => {
    const a = answers.value[i]!
    return !!a.text.trim() || optionDone(q, a)
  })

  return (
    <div class={styles.card}>
      <WidgetHeader icon={icon} title={title} typeBadge={typeBadge} />
      <WidgetBody>
        {isMulti && (
          <div style="display:flex;gap:4px;border-bottom:1px solid var(--bg-surface);margin-bottom:10px;overflow-x:auto">
            {items.map((q, i) => {
              const a = answers.value[i]!
              const done = !!a.text.trim() || optionDone(q, a)
              const active = i === activeTab.value
              const label = q.header || `Q${i + 1}`
              return (
                <button
                  key={i}
                  onClick={() => { activeTab.value = i }}
                  style={`padding:6px 10px;background:${active ? 'var(--bg-surface)' : 'transparent'};border:none;border-bottom:2px solid ${active ? 'var(--accent-purple)' : 'transparent'};color:${active ? 'var(--text-primary)' : done ? 'var(--accent-green)' : 'var(--text-muted-2)'};font-size:12px;cursor:pointer;white-space:nowrap;display:inline-flex;align-items:center;gap:4px`}
                >
                  {done && <i class="fas fa-check" style="font-size:10px;color:var(--accent-green)" />}
                  {label}
                </button>
              )
            })}
          </div>
        )}

        <div class="widget-message" style="white-space:pre-wrap">
          {current.header && isMulti ? current.question : (current.header ? `${current.header}\n\n${current.question}` : current.question)}
        </div>

        {data.plan && <PlanToggle plan={data.plan} />}

        {current.options && current.options.length > 0 && (
          <AskUserOptions
            key={`opts-${activeTab.value}`}
            options={current.options}
            requestId={`${data.requestId}-${activeTab.value}`}
            selectedIndex={currentAnswer.selected}
            multiSelect={current.multiSelect === true}
            pickedIndexes={currentAnswer.picked}
            onChange={idx => {
              if (current.multiSelect) togglePicked(activeTab.value, idx)
              else updateAnswer(activeTab.value, { selected: idx })
            }}
          />
        )}

        <AskUserTextInput
          key={`text-${activeTab.value}`}
          placeholder={current.options && current.options.length > 0 ? 'Or type a custom answer...' : 'Type your answer...'}
          value={currentAnswer.text}
          onChange={val => updateAnswer(activeTab.value, { text: val })}
        />
      </WidgetBody>
      <WidgetActions>
        {isPlan ? (
          <>
            <button class="widget-btn accept" onClick={handleApprove}>Approve</button>
            <button class="widget-btn deny" onClick={handleReject}>Reject</button>
          </>
        ) : (
          <>
            {isMulti && activeTab.value < items.length - 1 && (
              <button class="widget-btn" onClick={() => { activeTab.value = Math.min(items.length - 1, activeTab.value + 1) }}>
                Next →
              </button>
            )}
            <button class="widget-btn accept" onClick={handleSubmit} disabled={!allAnswered} style={!allAnswered ? 'opacity:0.5;cursor:not-allowed' : ''}>
              Submit{isMulti ? ` (${items.filter((q, i) => { const a = answers.value[i]!; return !!a.text.trim() || optionDone(q, a) }).length}/${items.length})` : ''}
            </button>
            <button class="widget-btn dismiss" onClick={handleSkip}>Skip</button>
          </>
        )}
      </WidgetActions>
    </div>
  )
}
