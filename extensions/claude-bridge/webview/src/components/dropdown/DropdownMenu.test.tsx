// @vitest-environment jsdom
import { createRef, render, type RefObject } from 'preact'
import { act } from 'preact/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { DropdownMenu } from './DropdownMenu'

// jsdom has no layout: the trigger sits 300px from the top, the menu reports
// whatever height the test gives it (capped by its max-height like a browser).
let menuHeight = 0
let observed: Array<() => void> = []
const box = (top: number, height: number, left = 200, width = 280) =>
  ({ top, bottom: top + height, left, right: left + width, width, height, x: left, y: top, toJSON: () => ({}) }) as DOMRect

let container: HTMLDivElement
const realRect = HTMLElement.prototype.getBoundingClientRect
beforeEach(() => {
  container = document.createElement('div')
  document.body.append(container)
  observed = []
  vi.stubGlobal('ResizeObserver', class {
    constructor(private cb: () => void) {}
    observe(): void { observed.push(this.cb) }
    disconnect(): void { observed = observed.filter(cb => cb !== this.cb) }
  })
  HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
    if (this.dataset.anchor) return box(300, 30)
    if (this.classList.contains('perm-menu')) return box(0, Math.min(menuHeight, parseFloat(this.style.maxHeight || 'Infinity')))
    return realRect.call(this)
  }
})
afterEach(() => {
  act(() => { render(null, container) })
  container.remove()
  vi.unstubAllGlobals()
  HTMLElement.prototype.getBoundingClientRect = realRect
})

const anchor = createRef<HTMLElement>()
function mount(open: boolean): void {
  act(() => {
    render(
      <div ref={anchor as never} data-anchor="1">
        <DropdownMenu anchorRef={anchor as RefObject<HTMLElement>} open={open} align="left"><div>items</div></DropdownMenu>
      </div>,
      container,
    )
  })
}

function menu(): HTMLElement { return container.querySelector('.perm-menu') as HTMLElement }

it('re-places the open menu when its content grows and caps it to the room above the trigger', () => {
  menuHeight = 120
  mount(true)
  expect(menu().style.maxHeight).toBe('292px')
  expect(menu().style.top).toBe(`${300 - 120 - 4}px`)

  // The GPT catalog arrives: the list grows past the room above the trigger.
  menuHeight = 640
  act(() => { for (const cb of observed) cb() })
  expect(menu().style.top).toBe('4px')
  expect(menu().style.maxHeight).toBe('292px')

  // A window resize re-places it too.
  menuHeight = 200
  act(() => { window.dispatchEvent(new Event('resize')) })
  expect(menu().style.top).toBe(`${300 - 200 - 4}px`)
})

it('stops observing once the menu closes', () => {
  menuHeight = 100
  mount(true)
  expect(observed.length).toBe(1)
  mount(false)
  expect(observed.length).toBe(0)
})