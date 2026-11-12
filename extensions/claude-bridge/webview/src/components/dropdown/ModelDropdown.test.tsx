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
    getConfig: vi.fn(),
    openExternal: vi.fn(),
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
import { codexCatalog, codexCatalogLoading, defaultEngine, engineAuth, receiveAgentPreferences } from '../../signals/agent'
import type { TabInfo } from '../../../shared/types'

function render(): VNode<any> {
  f.cursor = 0
  return ModelDropdown() as VNode<any>
}

function options(node: VNode<any>): Array<{ name: string; description: string; selected: boolean; disabled?: boolean; onClick: () => void }> {
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
    engineAuth.value = null
  })

  it('asks the server for the GPT list when it opens', () => {
    tab('claude', 'claude-opus-5-5')
    render().props.onToggle()
    expect(f.bridge.requestCodexModels).toHaveBeenCalledWith('t1')
    expect(codexCatalogLoading.value).toBe(true)
  })

  it('in a Claude chat a GPT model continues the chat on ChatGPT, a Claude model changes in place', () => {
    tab('claude', 'claude-opus-5-5')
    const opts = options(render())
    expect(opts.find(o => o.name === 'Opus 5.5')?.selected).toBe(true)
    const gptOption = opts.find(o => o.name === 'GPT-5.5')!
    expect(gptOption.description).toBe('Continues this chat on ChatGPT')
    gptOption.onClick()
    expect(f.bridge.changeModel).toHaveBeenCalledWith('t1', 'gpt-5.5')
    expect(f.bridge.newAgentSession).not.toHaveBeenCalled()
    options(render()).find(o => o.name === 'Sonnet 5.5')!.onClick()
    expect(f.bridge.changeModel).toHaveBeenCalledWith('t1', 'claude-sonnet-5-5')
  })

  it('in a GPT chat the trigger shows the GPT model and a Claude pick continues the chat on Claude', () => {
    tab('codex', 'gpt-5.5')
    const node = render()
    expect(node.props.trigger.props.label).toBe('GPT-5.5')
    const opts = options(node)
    expect(opts.find(o => o.name === 'GPT-5.5')?.selected).toBe(true)
    expect(opts.find(o => o.name === 'Opus 5.5')?.selected).toBe(false)
    expect(opts.find(o => o.name === 'Opus 5.5')?.description).toBe('Continues this chat on Claude')
    opts.find(o => o.name === 'Opus 5.5')!.onClick()
    expect(f.bridge.changeModel).toHaveBeenCalledWith('t1', 'claude-opus-5-5')
    expect(f.bridge.newAgentSession).not.toHaveBeenCalled()
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

  it('shows ChatGPT as unavailable without letting it be chosen when the server has no codex', () => {
    tab('claude', 'claude-opus-5-5')
    codexCatalog.value = { models: [], unavailable: 'codex is not installed on this Bridge server' }
    const node = render()
    expect(text(node)).toContain('ChatGPT is unavailable: codex is not installed on this Bridge server')
    const main = options(node).find(o => o.name === 'ChatGPT')!
    expect(main).toMatchObject({ disabled: true, description: 'ChatGPT is unavailable: codex is not installed on this Bridge server' })
    expect(options(node).some(o => o.name === 'GPT-5.5')).toBe(false)
  })
})
// What the server pushes in agent:preferences (live on every sign-in change).
function signedIn(claude: boolean, codex: boolean): void {
  receiveAgentPreferences({
    auth: {
      claude: { loggedIn: claude, reason: claude ? null : 'Claude is not signed in on this Bridge server.' },
      codex: { loggedIn: codex, reason: codex ? null : 'ChatGPT is not signed in on this Bridge server.' },
    },
  })
}

describe('model selector follows which engines the server is signed in to', () => {
  beforeEach(() => {
    f.state = []
    for (const fn of Object.values(f.bridge)) fn.mockReset()
    codexCatalog.value = { models: [gpt], account: { loggedIn: true } }
    codexCatalogLoading.value = false
    defaultEngine.value = 'claude'
    engineAuth.value = null
    tab('claude', 'claude-opus-5-5')
  })

  const claudeModels = (node: VNode<any>) => options(node).filter(o => ['Opus 5.5', 'Sonnet 5.5', 'Haiku 5.5', 'Fable 5.2'].includes(o.name))
  const gptModel = (node: VNode<any>) => options(node).find(o => o.name === 'GPT-5.5')!
  const mainAgent = (node: VNode<any>, name: 'Claude' | 'ChatGPT') => options(node).filter(o => o.name === name).at(-1)!

  it('both signed in: every model and both main agents can be chosen', () => {
    signedIn(true, true)
    const node = render()
    expect(claudeModels(node).every(o => !o.disabled)).toBe(true)
    expect(gptModel(node).disabled).toBe(false)
    expect(mainAgent(node, 'ChatGPT').disabled).toBe(false)
    expect(text(node)).not.toMatch(/not signed in/)
  })

  it('only Claude signed in: GPT models and the ChatGPT main agent are inert, with the reason', () => {
    signedIn(true, false)
    const node = render()
    expect(claudeModels(node).every(o => !o.disabled)).toBe(true)
    expect(gptModel(node).disabled).toBe(true)
    expect(text(node)).toContain('ChatGPT is not signed in on this Bridge server.')
    expect(mainAgent(node, 'ChatGPT')).toMatchObject({ disabled: true, description: 'ChatGPT is not signed in on this Bridge server.' })
    gptModel(node).onClick()
    mainAgent(render(), 'ChatGPT').onClick()
    expect(f.bridge.newAgentSession).not.toHaveBeenCalled()
    expect(f.bridge.setDefaultEngine).not.toHaveBeenCalled()
  })

  it('only ChatGPT signed in: Claude models are inert and a Claude main agent explains where new chats go', () => {
    signedIn(false, true)
    const node = render()
    expect(claudeModels(node).every(o => o.disabled)).toBe(true)
    expect(text(node)).toContain('Claude is not signed in on this Bridge server.')
    expect(gptModel(node).disabled).toBe(false)
    // Claude is already the main agent: it stays selected and says what happens.
    expect(mainAgent(node, 'Claude')).toMatchObject({
      selected: true,
      disabled: false,
      description: 'Claude is not signed in on this Bridge server. New chats open on ChatGPT.',
    })
    claudeModels(node)[1]!.onClick()
    expect(f.bridge.changeModel).not.toHaveBeenCalled()
    mainAgent(render(), 'ChatGPT').onClick()
    expect(f.bridge.setDefaultEngine).toHaveBeenCalledWith('t1', 'codex')
  })

  it('neither signed in: one line with a link to the Bridge dashboard', async () => {
    signedIn(false, false)
    f.bridge.getConfig.mockResolvedValue({ serverUrl: 'wss://bridge.example.test/ws/session' })
    const node = render()
    expect(text(node)).toContain('Neither Claude nor ChatGPT is signed in on this Bridge server.')
    expect(text(node)).not.toContain('Claude is not signed in')
    expect(claudeModels(node).every(o => o.disabled)).toBe(true)
    expect(gptModel(node).disabled).toBe(true)
    const link = findButton(node, 'Open the Bridge dashboard')
    link.props.onClick()
    await vi.waitFor(() => expect(f.bridge.openExternal).toHaveBeenCalledWith('https://bridge.example.test/ui'))
  })

  it('a sign-in pushed by the server enables the engine without reopening anything', () => {
    signedIn(true, false)
    expect(gptModel(render()).disabled).toBe(true)
    signedIn(true, true)
    expect(gptModel(render()).disabled).toBe(false)
  })
})

function findButton(node: VNode<any>, label: string): VNode<any> {
  let hit: VNode<any> | undefined
  const walk = (n: unknown): void => {
    if (hit || !n || typeof n !== 'object') return
    if (Array.isArray(n)) return n.forEach(walk)
    const v = n as VNode<any>
    if (v.type === 'button' && [v.props.children].flat().includes(label)) hit = v
    walk(v.props?.children)
  }
  walk(node.props.children)
  if (!hit) throw new Error(`no button ${label}`)
  return hit
}
