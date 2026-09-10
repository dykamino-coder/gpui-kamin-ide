import { describe, expect, it } from 'vitest'
import {
  approvalSelectionKey,
  formatHookDeclaration,
  formatReviewText,
  handlerTypeLabel,
  initiallySelectedHookHashes,
  isReviewableHandler,
  looksDangerous,
} from './hook-approval-display'

describe('hook approval display', () => {
  it('shows command args and every supported handler shape before relay rewrite', () => {
    expect(formatHookDeclaration({
      type: 'command',
      command: 'node',
      args: ['hooks/guard.mjs', '--mode', 'strict'],
      shell: 'bash',
      host: 'local',
    })).toContain('hooks/guard.mjs')

    expect(formatHookDeclaration({ type: 'prompt', prompt: 'Review this', model: 'sonnet' })).toContain('Review this')
    expect(formatHookDeclaration({ type: 'agent', prompt: 'Investigate this', model: 'haiku' })).toContain('Investigate this')
    expect(formatHookDeclaration({ type: 'http', url: 'https://example.test/hook', allowedEnvVars: ['SAFE_NAME'] })).toContain('SAFE_NAME')
    expect(formatHookDeclaration({ type: 'mcp_tool', server: 'docs', tool: 'audit', input: { mode: 'strict' } })).toContain('"tool": "audit"')
    expect(formatHookDeclaration({ type: 'command', command: 'node', futureField: 'still visible' })).toContain('futureField')
  })

  it('detects dangerous content hidden in exec-form args and Windows commands', () => {
    expect(looksDangerous({ type: 'command', command: 'node', args: ['-e', "execSync('rm -rf /tmp/data')"] })).toBe(true)
    expect(looksDangerous({ type: 'command', command: 'powershell', args: ['-Command', 'Remove-Item C:\\data -Recurse -Force'] })).toBe(true)
    expect(looksDangerous({ type: 'command', command: 'node', args: ['hooks/guard.mjs'] })).toBe(false)
  })

  it('redacts credentials while preserving the declaration structure', () => {
    const declaration = formatHookDeclaration({
      type: 'http',
      url: 'https://user:pass@example.test/hook?token=url-secret&mode=strict',
      headers: { Authorization: 'Bearer header-secret', 'X-API-Key': 'alternate-header-secret', 'X-Mode': 'strict' },
      env: { DATABASE_URL: 'postgres://secret', SAFE_FLAG: 'yes' },
      input: { apiKey: 'input-secret', nested: { password: 'nested-secret', mode: 'strict' } },
    })

    expect(declaration).not.toContain('url-secret')
    expect(declaration).not.toContain('header-secret')
    expect(declaration).not.toContain('alternate-header-secret')
    expect(declaration).not.toContain('postgres://secret')
    expect(declaration).not.toContain('input-secret')
    expect(declaration).not.toContain('nested-secret')
    expect(declaration).toContain('"Authorization": "***"')
    expect(declaration).toContain('"X-API-Key": "***"')
    expect(declaration).toContain('"X-Mode": "strict"')
    expect(declaration).toContain('"DATABASE_URL": "***"')
    expect(declaration).toContain('"mode": "strict"')
  })

  it('does not preselect an unknown hook just because the heuristic is quiet', () => {
    const hooks = [
      { hash: 'approved', handler: { type: 'command', command: 'node' } },
      { hash: 'new-safe-looking', handler: { type: 'command', command: 'node' } },
    ]
    expect([...initiallySelectedHookHashes(hooks, ['approved', 'stale'])]).toEqual(['approved'])
    expect([...initiallySelectedHookHashes(hooks, [])]).toEqual([])
  })

  it('changes selection identity across plugins, declarations, and approval sets', () => {
    const base = approvalSelectionKey('plugin-a', [{ hash: 'hook-a' }], [])
    expect(approvalSelectionKey('plugin-b', [{ hash: 'hook-a' }], [])).not.toBe(base)
    expect(approvalSelectionKey('plugin-a', [{ hash: 'hook-b' }], [])).not.toBe(base)
    expect(approvalSelectionKey('plugin-a', [{ hash: 'hook-a' }], ['hook-a'])).not.toBe(base)
  })

  it('keeps long declarations intact and rejects malformed handlers', () => {
    const tail = `tail-${'x'.repeat(800)}`
    expect(formatHookDeclaration({ type: 'prompt', prompt: tail })).toContain(tail)
    expect(isReviewableHandler(null)).toBe(false)
    expect(isReviewableHandler({ type: 'command', command: 'node', args: [42] })).toBe(false)
    expect(handlerTypeLabel(null)).toBe('invalid')
    expect([...initiallySelectedHookHashes([{ hash: 'bad', handler: null }], ['bad'])]).toEqual([])
  })

  it('makes bidi controls visible without hiding executable command text', () => {
    const declaration = formatHookDeclaration({ type: 'command', command: 'node\u202Eevil.mjs', args: ['--token', 'literal-value'] })
    expect(declaration).toContain('node[U+202E]evil.mjs')
    expect(declaration).toContain('literal-value')
    expect(formatReviewText('Bash\u202E|Read')).toBe('Bash[U+202E]|Read')
  })
})
