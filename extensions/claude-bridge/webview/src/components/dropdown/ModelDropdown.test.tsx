import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'

const f = vi.hoisted(() => ({
  state: [] as unknown[],
  cursor: 0,
  bridge: {
    changeModel: vi.fn(),
    requestCodexModels: vi.fn(),
    setDefaultEngine: vi.fn(),
    newAgentSession: vi.fn(),
  },
}))
vi.mock('preact/hooks', () => ({
  useState: (initial: unknown) => {
    const i = f.cursor++
    if (!(i in f.state)) f.state[i] = initial
    return [f.state[i], (value: unknown) => { f.state[i] = value }]
  },
}))
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => f.bridge }))

import { ModelDropdown } from './ModelDropdown'
import { DropdownOption } from './DropdownOption'
import { activeTabId, tabs } from '../../signals/tabs'
import { codexCatalog, codexCatalogLoading, defaultEngine } from '../../signals/agent'
import type { TabInfo } from '../../../shared/types'

function render(): VNode<any> {
  f.cursor = 0
  return ModelDropdown() as VNode<any>
}

function options(node: VNode<any>): Array<{ name: string; description: string; selected: boolean; onClick: () => void }> {
  const found: Array<any> = []
  const walk = (n: unknown): void => {
    if (Array.isArray(n)) return n.forEach(walk)
    if (!n || typeof n !== 'object') return
    const v = n as VNode<any>
    if (v.type === DropdownOption) found.push(v.props)
    walk(v.props?.children)
  }
  walk(node.props.children)
  return found
}

function text(node: VNode<any>): string {
  const out: string[] = []
  const walk = (n: unknown): void => {
    if (Array.isArray(n)) return n.forEach(walk)
    if (typeof n === 'string') return void out.push(n)
    if (!n || typeof n !== 'object') return
    walk((n as VNode<any>).props?.children)
  }
  walk(node.props.children)
  return out.join(' | ')
}

const gpt = { id: 'gpt-5.5', model: 'gpt-5.5', displayName: 'GPT-5.5', description: 'Frontier', isDefault: true, defaultReasoningEffort: null, reasoningEfforts: [] }

function tab(engine: 'claude' | 'codex', model: string): void {
  tabs.value = [{ id: 't1', engine, model } as unknown as TabInfo]
  activeTabId.value = 't1'
}

describe('model selector with Claude and GPT groups', () => {
  beforeEach(() => {
    f.state = []
    for (const fn of Object.values(f.bridge)) fn.mockReset()
    codexCatalog.value = { models: [gpt], account: { loggedIn: true } }
    codexCatalogLoading.value = false
    defaultEngine.value = 'claude'
  })

  it('asks the server for the GPT list when it opens', () => {
    tab('claude', 'claude-opus-5-5')
    render().props.onToggle()
    expect(f.bridge.requestCodexModels).toHaveBeenCalledWith('t1')
    expect(codexCatalogLoading.value).toBe(true)
  })

  it('in a Claude chat a GPT model opens a new ChatGPT chat, a Claude model changes in place', () => {
    tab('claude', 'claude-opus-5-5')
    const opts = options(render())
    expect(opts.find(o => o.name === 'Opus 5.5')?.selected).toBe(true)
    opts.find(o => o.name === 'GPT-5.5')!.onClick()
    expect(f.bridge.newAgentSession).toHaveBeenCalledWith('codex', 'gpt-5.5')
    expect(f.bridge.changeModel).not.toHaveBeenCalled()
    options(render()).find(o => o.name === 'Sonnet 5.5')!.onClick()
    expect(f.bridge.changeModel).toHaveBeenCalledWith('t1', 'claude-sonnet-5-5')
  })

  it('in a GPT chat the trigger shows the GPT model and a Claude pick opens a Claude chat', () => {
    tab('codex', 'gpt-5.5')
    const node = render()
    expect(node.props.trigger.props.label).toBe('GPT-5.5')
    const opts = options(node)
    expect(opts.find(o => o.name === 'GPT-5.5')?.selected).toBe(true)
    expect(opts.find(o => o.name === 'Opus 5.5')?.selected).toBe(false)
    opts.find(o => o.name === 'Opus 5.5')!.onClick()
    expect(f.bridge.newAgentSession).toHaveBeenCalledWith('claude', 'claude-opus-5-5')
  })

  it('sets the main agent for new chats', () => {
    tab('claude', 'claude-opus-5-5')
    options(render()).find(o => o.name === 'ChatGPT')!.onClick()
    expect(f.bridge.setDefaultEngine).toHaveBeenCalledWith('t1', 'codex')
    expect(defaultEngine.value).toBe('codex')
    expect(options(render()).find(o => o.name === 'ChatGPT')?.selected).toBe(true)
  })

  it('says why GPT chats cannot answer when ChatGPT is not signed in', () => {
    tab('claude', 'claude-opus-5-5')
    codexCatalog.value = { models: [gpt], account: { loggedIn: false } }
    expect(text(render())).toMatch(/not signed in/)
  })
})
