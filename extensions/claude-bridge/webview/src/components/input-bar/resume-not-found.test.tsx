import { afterEach, expect, it, vi } from 'vitest'
import { activeTabId, tabs } from '../../signals/tabs'
import { PromptTextarea } from './PromptTextarea'
import { ResumeNotFoundNotice } from './ResumeNotFoundNotice'
import { SEND } from '../../lib/bridge-channels'
import { WidgetActions } from '../widgets/WidgetActions'
import { WidgetButton } from '../widgets/WidgetButton'

vi.mock('preact/hooks', () => ({ useRef: () => ({ current: null }), useEffect: vi.fn() }))
afterEach(() => { tabs.value = []; activeTabId.value = null })

it('disables only the missing-conversation composer with an actionable hint', () => {
  activeTabId.value = 'a'
  tabs.value = [{ id: 'a', status: 'error', resumeNotFound: true } as any]
  const props = { onInput: vi.fn(), onKeyDown: vi.fn(), onPaste: vi.fn() }
  const held = PromptTextarea(props)
  expect(held.props.disabled).toBe(true)
  expect(held.props.placeholder).toBe('Choose Retry or Start new conversation to continue.')
  expect(held.props.title).toBe(held.props.placeholder)
  tabs.value = [{ id: 'a', status: 'connecting' } as any]
  expect(PromptTextarea(props).props.disabled).toBe(false)
})

it('renders the persistent choice and wires both explicit actions', () => {
  const onNew = vi.fn(), onRetry = vi.fn()
  const notice = ResumeNotFoundNotice({ onNew, onRetry })
  expect(notice.props.role).toBe('alert')
  const actions = (notice.props.children as any[]).find(child => child?.type === WidgetActions)
  const buttons = ([] as any[]).concat(actions.props.children).filter(child => child?.type === WidgetButton)
  expect(buttons.map(button => button.props.label)).toEqual(['Start new conversation', 'Retry'])
  expect(buttons.map(button => button.props.variant)).toEqual(['accept', 'dismiss'])
  buttons[0].props.onClick()
  buttons[1].props.onClick()
  expect(onNew).toHaveBeenCalledTimes(1)
  expect(onRetry).toHaveBeenCalledTimes(1)
  expect(SEND.chooseNewConversation).toBe('session:choose-new')
  expect(SEND.retryStoredConversation).toBe('session:retry-stored')
})
