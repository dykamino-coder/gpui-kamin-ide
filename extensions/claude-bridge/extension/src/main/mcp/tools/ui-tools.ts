// UI tools — AskUserQuestion, PlanMode, Skill
// These show interactive widgets in the KaminIDE webview.

import type { BrowserWindow, IpcMainEvent } from '@kaminide/host-compat'
import { ipcMain } from '@kaminide/host-compat'
import type { McpResult } from '../tool-registry'
import { randomUUID } from 'crypto'

// ─── Pending question bookkeeping ───────────────────────
// We run everything in "deferred" mode: the MCP call returns immediately with
// a placeholder so the CLI's built-in elicitation/tool-timeout clocks never
// fire. The user's real answer is later injected as a normal user turn via
// PTY (that's what the CLI would see if the user just typed it).
type PendingQuestion = {
  tabId: string | null
  question: string
  isPlanApproval: boolean
}
const pendingQuestions = new Map<string, PendingQuestion>()

let mainWindow: BrowserWindow | null = null
let activeTabId: string | null = null
let sendToPty: ((tabId: string, text: string) => void) | null = null

export function setupUiTools(
  win: BrowserWindow,
  _getActiveTabId: () => string | null,
  ptySender?: (tabId: string, text: string) => void,
): void {
  mainWindow = win
  activeTabId = null
  if (ptySender) sendToPty = ptySender

  ipcMain.on('ask-user-response', (_event: IpcMainEvent, requestId: string, answer: string) => {
    const pending = pendingQuestions.get(requestId)
    if (!pending) return
    pendingQuestions.delete(requestId)

    // Deliver the user's answer as if they just typed it in the terminal.
    // If we don't have a tabId or PTY sender, we simply drop — the CLI already
    // got our placeholder response and moved on; it'll ask again or continue.
    if (pending.tabId && sendToPty) {
      const clean = (answer ?? '').trim()
      const text = clean.length > 0 ? clean : '[User dismissed the question]'
      // Short answers: plain type + CR. Long/multiline: bracketed paste.
      if (text.length < 400 && !text.includes('\n')) {
        sendToPty(pending.tabId, text)
        sendToPty(pending.tabId, '\r')
      } else {
        sendToPty(pending.tabId, `\x1b[200~${text}\x1b[201~`)
        sendToPty(pending.tabId, '\r')
      }
    }
  })
}

export function updateUiToolsWindow(win: BrowserWindow): void {
  mainWindow = win
}

export function updateUiToolsActiveTab(tabId: string | null): void {
  activeTabId = tabId
}

export function updateUiToolsPtySender(fn: (tabId: string, text: string) => void): void {
  sendToPty = fn
}

const DEFERRED_RESULT_TEXT =
  'The user is still answering this question in the UI. Their reply will arrive as the next user message — do not act yet, wait for it.'

export async function askUserQuestion(input: Record<string, unknown>, context?: { tabId?: string | null }): Promise<McpResult> {
  // Normalize both formats into a `questions[]` array of {header, question, options[]}.
  type RawQuestion = {
    question: string
    header?: string
    options?: Array<{ label: string; description?: string; preview?: string }>
    multiSelect?: boolean
  }
  // CLI шлёт questions то массивом, то JSON-СТРОКОЙ (наблюдалось живьём:
  // `"questions": "[{\"question\"..."`). Без парса строка проваливалась в
  // legacy-ветку с ПУСТЫМ вопросом — виджет рисовал голый textarea без опций.
  let rawQuestions = input.questions as RawQuestion[] | string | undefined
  if (typeof rawQuestions === 'string') {
    try { rawQuestions = JSON.parse(rawQuestions) as RawQuestion[] } catch { rawQuestions = undefined }
  }

  type SimpleQuestion = { header?: string; question: string; options?: string[]; multiSelect?: boolean }
  let questionsList: SimpleQuestion[] = []

  if (rawQuestions && Array.isArray(rawQuestions) && rawQuestions.length > 0) {
    questionsList = rawQuestions.map(q => ({
      header: q.header,
      question: q.question,
      options: q.options?.map(opt => opt.description ? `${opt.label} — ${opt.description}` : opt.label),
      multiSelect: q.multiSelect === true,
    }))
  } else {
    const legacyQuestion = (input.question as string) || (input.message as string) || ''
    const legacyOptions = input.options as string[] | undefined
    questionsList = [{ question: legacyQuestion, options: legacyOptions }]
  }

  if (!mainWindow || mainWindow.isDestroyed()) {
    const flattened = questionsList.map(q =>
      `${q.header ? q.header + '\n' : ''}${q.question}${q.options ? '\n' + q.options.join(', ') : ''}`
    ).join('\n\n')
    return {
      content: [{ type: 'text', text: `${flattened}\n\n(No UI window available — answer via terminal)` }],
    }
  }

  const requestId = randomUUID()
  const targetTabId = context?.tabId ?? activeTabId
  const firstQuestion = questionsList[0]?.question ?? ''

  pendingQuestions.set(requestId, { tabId: targetTabId, question: firstQuestion, isPlanApproval: false })

  // Back-compat payload:
  //   `question`/`options` hold the first question (old widgets fall back to this)
  //   `questions` carries the full list so the new widget can render tabs.
  mainWindow.webContents.send('ask-user-question', targetTabId, {
    requestId,
    question: questionsList[0]?.header ? `${questionsList[0]!.header}\n\n${questionsList[0]!.question}` : firstQuestion,
    options: questionsList[0]?.options,
    questions: questionsList,
  })

  return { content: [{ type: 'text', text: DEFERRED_RESULT_TEXT }] }
}

export async function enterPlanMode(_input: Record<string, unknown>): Promise<McpResult> {
  return {
    content: [{
      type: 'text',
      text: 'Entered plan mode. Changes will be planned but not executed until approved.',
    }],
  }
}

export async function exitPlanMode(input: Record<string, unknown>, context?: { tabId?: string | null }): Promise<McpResult> {
  const plan = (input.plan as string) || ''

  if (!mainWindow || mainWindow.isDestroyed()) {
    return {
      content: [{
        type: 'text',
        text: `Plan submitted:\n${plan.slice(0, 500)}\n\n(No UI window available — approve via terminal)`,
      }],
    }
  }

  const requestId = randomUUID()
  const targetTabId = context?.tabId ?? activeTabId

  pendingQuestions.set(requestId, { tabId: targetTabId, question: 'Approve plan', isPlanApproval: true })

  mainWindow.webContents.send('ask-user-question', targetTabId, {
    requestId,
    question: 'Approve this plan?',
    options: ['Approve', 'Reject', 'Edit'],
    plan,
    isPlanApproval: true,
  })

  return { content: [{ type: 'text', text: DEFERRED_RESULT_TEXT }] }
}

export async function skillLoad(input: Record<string, unknown>): Promise<McpResult> {
  const skillName = (input.name as string) || (input.skill as string) || ''
  return {
    content: [{
      type: 'text',
      text: `Skill "${skillName}" loaded.`,
    }],
  }
}
