import { describe, expect, it, vi } from 'vitest'
import { codexAccountAction, quickCommands, terminalEngines } from './quick-commands'
import { codexTerminalArgs } from '../../../../core/server/terminal-codex'

describe('terminal engine actions', () => {
  it('preserves Claude commands and offers Codex only when available', () => {
    expect(terminalEngines(false)).toEqual(['claude'])
    expect(terminalEngines(true)).toEqual(['claude', 'codex'])
    expect(quickCommands('claude').map((q) => 'cmd' in q && q.cmd)).toEqual([
      'claude --dangerously-skip-permissions',
      'claude --dangerously-skip-permissions --resume',
      'claude --dangerously-skip-permissions auth status',
      'claude --dangerously-skip-permissions /usage',
      'claude --dangerously-skip-permissions /login',
      'claude --dangerously-skip-permissions /logout',
    ])
  })

  it('starts Codex, resumes the last thread and reads status; never launches CLI login/logout writers', () => {
    const commands = quickCommands('codex')
    expect(commands.map((q) => q.label)).toEqual(['Codex', 'Resume', 'Auth Status', 'Usage', 'Login', 'Logout'])
    expect(commands.filter((q) => 'codex' in q).map((q) => codexTerminalArgs(q.codex))).toEqual([
      [],
      ['resume', '--last'],
      ['login', 'status'],
      [],
    ])
    expect(commands.slice(4).map((q) => 'account' in q && q.account)).toEqual(['login', 'logout'])
    expect(() => codexTerminalArgs('login')).toThrow('Unknown')
    expect(() => codexTerminalArgs('logout')).toThrow('Unknown')
  })

  it.each(['login', 'logout'] as const)(
    're-reads the shared app-server account after %s, including a failed action',
    async (action) => {
      const events: string[] = []
      const api = {
        startCodexLogin: vi.fn(async () => {
          events.push('login')
        }),
        logoutCodex: vi.fn(async () => {
          events.push('logout')
        }),
        getCodexAccount: vi.fn(async () => {
          events.push('read')
        }),
      }
      await codexAccountAction(action, api)
      expect(events).toEqual([action, 'read'])
      expect(api.getCodexAccount).toHaveBeenCalledWith(true)
      api.startCodexLogin.mockRejectedValueOnce(new Error('busy'))
      await expect(codexAccountAction('login', api)).rejects.toThrow('busy')
      expect(api.getCodexAccount).toHaveBeenCalledTimes(2)
    },
  )
})
