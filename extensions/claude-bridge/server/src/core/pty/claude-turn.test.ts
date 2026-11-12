import { beforeEach, describe, expect, it, vi } from 'vitest'

const cliInterrupt = vi.fn<() => boolean>()
const interruptToolCalls = vi.fn()
const armSwitchConfirm = vi.fn()
const rememberModelSelection = vi.fn()

vi.mock('./session-cli-control', () => ({
  sessionCliControl: (s: { cli?: boolean }) => (s.cli ? { interrupt: cliInterrupt } : undefined),
}))
vi.mock('./mcp-foreground-calls', () => ({ interruptSessionToolCalls: interruptToolCalls }))
vi.mock('./live-switch', () => ({ armSwitchConfirm, modelCommand: (m: string) => `\x15/model ${m}\r` }))
vi.mock('./model-selection', () => ({ rememberModelSelection }))

const { interruptClaudeTurn, switchClaudeModelInPlace } = await import('./claude-turn')

beforeEach(() => vi.clearAllMocks())

describe('interruptClaudeTurn', () => {
  it('uses the CLI control channel when it takes the interrupt', () => {
    cliInterrupt.mockReturnValue(true)
    const session = { cli: true, streamingProxy: { interrupt: vi.fn() } } as any
    const write = vi.fn(() => true)
    interruptClaudeTurn('s', () => session, write)
    expect(write).not.toHaveBeenCalled()
    expect(session.streamingProxy.interrupt).not.toHaveBeenCalled()
    expect(interruptToolCalls).toHaveBeenCalledWith(session)
  })

  it('otherwise aborts the upstream stream and sends Ctrl+C', () => {
    const session = { streamingProxy: { interrupt: vi.fn() } } as any
    const write = vi.fn(() => true)
    interruptClaudeTurn('s', () => session, write)
    expect(session.streamingProxy.interrupt).toHaveBeenCalled()
    expect(write).toHaveBeenCalledWith('s', '\x03')
    expect(interruptToolCalls).toHaveBeenCalledWith(session)
  })

  it('still sends Ctrl+C for an unknown session', () => {
    const write = vi.fn(() => false)
    interruptClaudeTurn('gone', () => undefined, write)
    expect(write).toHaveBeenCalledWith('gone', '\x03')
    expect(interruptToolCalls).not.toHaveBeenCalled()
  })
})

describe('switchClaudeModelInPlace', () => {
  it('writes /model into a live conversation and records the selection', () => {
    const session = { cliConversationId: 'c1', isRestarting: false, model: 'old' } as any
    const write = vi.fn(() => true)
    expect(switchClaudeModelInPlace(session, 's', 'opus', write)).toBe(true)
    expect(write).toHaveBeenCalledWith('s', '\x15/model opus\r')
    expect(armSwitchConfirm).toHaveBeenCalledWith(session, 'model', undefined)
    expect(session.model).toBe('opus')
    expect(rememberModelSelection).toHaveBeenCalledWith('c1', 'opus')
  })

  it('asks for a restart without a conversation, while restarting, or when the write fails', () => {
    const write = vi.fn(() => true)
    expect(switchClaudeModelInPlace({ cliConversationId: null } as any, 's', 'm', write)).toBe(false)
    expect(switchClaudeModelInPlace({ cliConversationId: 'c', isRestarting: true } as any, 's', 'm', write)).toBe(false)
    expect(write).not.toHaveBeenCalled()
    const session = { cliConversationId: 'c', model: 'old' } as any
    expect(switchClaudeModelInPlace(session, 's', 'm', () => false)).toBe(false)
    expect(session.model).toBe('old')
    expect(rememberModelSelection).not.toHaveBeenCalled()
  })
})
