import { afterEach, expect, it, vi } from 'vitest'
import { activeTabId, tabs } from '../../signals/tabs'
import { PromptTextarea } from './PromptTextarea'
import { ResumeNotFoundNotice } from './ResumeNotFoundNotice'
import { SEND } from '../../lib/bridge-channels'

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
  const buttons = (notice.props.children as any[]).filter(child => child?.type === 'button')
  expect(buttons.map(button => button.props.children)).toEqual(['Start new conversation', 'Retry'])
  buttons[0].props.onClick()
  buttons[1].props.onClick()
  expect(onNew).toHaveBeenCalledTimes(1)
  expect(onRetry).toHaveBeenCalledTimes(1)
  expect(SEND.chooseNewConversation).toBe('session:choose-new')
  expect(SEND.retryStoredConversation).toBe('session:retry-stored')
})
