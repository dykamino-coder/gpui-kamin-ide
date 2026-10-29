import { beforeEach, expect, it, vi } from 'vitest'

const fixture = vi.hoisted(() => ({
  root: { innerHTML: '' },
  effects: [] as Array<() => void>,
  attach: vi.fn(),
}))
vi.mock('preact/hooks', () => ({
  useRef: () => ({ current: { shadowRoot: fixture.root, attachShadow: fixture.attach } }),
  useEffect: (effect: () => void) => fixture.effects.push(effect),
}))
import { WidgetBlock } from './WidgetBlock'

beforeEach(() => {
  fixture.root.innerHTML = ''
  fixture.effects = []
  fixture.attach.mockClear()
})

it('renders text and links through inherited theme variables inside the shadow root', () => {
  const html = '<p>Theme preview <a href="https://example.com">link</a></p>'
  WidgetBlock({ input: { html } })
  fixture.effects[0]()
  expect(fixture.root.innerHTML).toContain(':host{display:block;color:var(--text-primary)')
  expect(fixture.root.innerHTML).toContain('a{color:var(--accent-action)}')
  expect(fixture.root.innerHTML).toContain(html)
  expect(fixture.root.innerHTML).not.toMatch(/#[\da-f]{3,8}\b/i)
  expect(fixture.attach).not.toHaveBeenCalled()
})
