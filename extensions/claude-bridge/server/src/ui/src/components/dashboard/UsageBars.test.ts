import { expect, it, vi } from 'vitest'
vi.mock('preact/jsx-runtime', () => ({
  jsx: (type: unknown, props: unknown) => ({ type, props }),
  jsxs: (type: unknown, props: unknown) => ({ type, props }),
  jsxDEV: (type: unknown, props: unknown) => ({ type, props }),
}))
vi.mock('preact/jsx-dev-runtime', () => ({
  jsx: (type: unknown, props: unknown) => ({ type, props }),
  jsxs: (type: unknown, props: unknown) => ({ type, props }),
  jsxDEV: (type: unknown, props: unknown) => ({ type, props }),
}))
import { UsageBars } from './UsageBars'
import { combinePlanUsage, emptyPlanUsage, parseUsageScreen } from '../../../../core/server/utils/plan-usage'

function text(node: any): string {
  if (node == null || typeof node === 'boolean') return ''
  if (Array.isArray(node)) return node.map(text).join('')
  if (typeof node !== 'object') return String(node)
  if (typeof node.type === 'function') return text(node.type(node.props))
  return text(node.props?.children)
}
it('renders arbitrary supported windows and labels partial/missing reset data in the actual Account component', () => {
  const usage = combinePlanUsage(
    parseUsageScreen('Current week (Fable)\n17% used\nResets Monday', new Date().toISOString()),
    null,
    null,
  )
  const output = text(UsageBars({ usage }))
  expect(output).toContain('Week (Fable)')
  expect(output).toContain('17%')
  expect(output).toContain('Partial usage')
  expect(output).toContain('Exact reset timestamp unavailable')
})
it('keeps last-known rows during refresh and unavailable captures, with explicit stale state', () => {
  const previous = combinePlanUsage(
    parseUsageScreen('Current week (Fable)\n17% used\nResets Monday', new Date().toISOString()),
    null,
    null,
  )
  const usage = combinePlanUsage(emptyPlanUsage('capture-error'), null, previous)
  const output = text(UsageBars({ usage, loading: true }))
  expect(output).toContain('Refreshing usage')
  expect(output).toContain('Usage unavailable')
  expect(output).toContain('Week (Fable)')
  expect(output).toContain('Stale')
  expect(output).toContain('Refresh failed')
})
it('shows unavailability rather than silently omitting the Account usage area', () => {
  expect(text(UsageBars({ usage: emptyPlanUsage() }))).toContain('Usage unavailable')
})
