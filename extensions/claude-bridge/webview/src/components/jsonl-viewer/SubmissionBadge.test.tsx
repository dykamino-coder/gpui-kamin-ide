// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { SubmissionBadge } from './SubmissionBadge'
import { activeTabId } from '../../signals/tabs'
import { receiveSubmission, submissions } from '../../signals/submissions'
import type { JsonlEntryData } from '../../types/jsonl'

const action = vi.hoisted(() => vi.fn())
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => ({ submissionAction: action }) }))
const root = document.createElement('div')
it('shows a rebind failure reason and addresses Retry and Cancel to the same ID', () => {
  activeTabId.value = 'tab'
  receiveSubmission('tab', { id: 'unknown', text: 'message', state: 'uncertain', retryable: true, cancelable: true,
    warning: 'The previous CLI session ended before delivery was confirmed.' })
  act(() => render(<SubmissionBadge entry={{ submissionId: 'unknown' } as JsonlEntryData} />, root))
  expect(root.textContent).toContain('The previous CLI session ended')
  const buttons = [...root.querySelectorAll('button')]
  buttons.find(button => button.textContent === 'Retry')!.click()
  buttons.find(button => button.textContent === 'Cancel')!.click()
  expect(action.mock.calls).toEqual([['tab', 'unknown', 'push'], ['tab', 'unknown', 'cancel']])
})
afterEach(() => { act(() => render(null, root)); submissions.value = new Map(); action.mockClear() })

it('shows why delivery is waiting without hovering and preserves the addressed actions', () => {
  activeTabId.value = 'tab'
  receiveSubmission('tab', { id: 'waiting', text: 'message', state: 'queued', cancelable: true,
    warning: 'Waiting for the current terminal input transaction to finish.' })
  act(() => render(<SubmissionBadge entry={{ submissionId: 'waiting' } as JsonlEntryData} />, root))
  expect(root.textContent).toContain('Waiting for the current terminal input transaction to finish.')
  const buttons = [...root.querySelectorAll('button')]
  buttons.find(button => button.textContent === 'Send now')!.click()
  buttons.find(button => button.textContent === 'Interrupt and send')!.click()
  expect(action.mock.calls).toEqual([['tab', 'waiting', 'push'], ['tab', 'waiting', 'interrupt']])
  act(() => receiveSubmission('tab', { id: 'waiting', state: 'native', cancelable: false }))
  expect(root.textContent).not.toContain('Waiting for the current terminal')
  expect([...root.querySelectorAll('button')].map(button => button.textContent)).toEqual(['Interrupt and send'])
})
