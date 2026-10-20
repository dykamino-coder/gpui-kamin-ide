// INC-2026-0072: read-only project session tools over a real temp layout.
// The caller context is injected exactly as session-glue builds it from an
// authenticated session; files, directories and symlinks are real.
import fs from 'node:fs'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  addConversation,
  assistant,
  at,
  ctxFor,
  makeLayout,
  snapshotTree,
  text,
  thinking,
  toolResult,
  toolUse,
  trySymlink,
  user,
  type Layout,
} from './fixtures.test-support'
import { readerStats } from './line-reader'
import { NOT_FOUND, type CallerContext } from './scope'
import { compileSearch } from './search'
import { runProjectSessionTool, type AccessRecord, type ProjectSessionToolName } from './tools'

const TOKEN = '6f1c2d7e-0000-4000-8000-000000000001'
const OTHER_TOKEN = '6f1c2d7e-0000-4000-8000-000000000002'
const CWD = 'C:\\Users\\alice\\Projects\\app'
const OTHER_CWD = 'C:\\Users\\alice\\Projects\\other'
const A = 'aaaaaaaa-0000-4000-8000-00000000000a'
const B = 'bbbbbbbb-0000-4000-8000-00000000000b'
const FOREIGN = 'ffffffff-0000-4000-8000-00000000000f'
const OTHER_PROJECT = 'cccccccc-0000-4000-8000-00000000000c'

let l: Layout
let logs: AccessRecord[]

beforeEach(() => {
  l = makeLayout()
  logs = []
})
afterEach(() => {
  fs.rmSync(l.root, { recursive: true, force: true })
})

async function call(ctx: CallerContext, name: ProjectSessionToolName, args: Record<string, unknown> = {}) {
  const res = await runProjectSessionTool(name, args, ctx, (r) => logs.push(r))
  const body = res.content[0]?.text ?? ''
  const pageText = res.content[1]?.text
  return { res, body, page: pageText ? (JSON.parse(pageText) as Record<string, any>) : {}, isError: !!res.isError }
}

function seedProject(): void {
  addConversation(l, TOKEN, CWD, A, [
    { type: 'custom-title', customTitle: 'Fix the parser' },
    user('Please fix the parser bug', at(0), { gitBranch: 'main' }),
    assistant(
      at(1),
      [
        thinking('Need to read parser.ts'),
        text('Reading the parser.'),
        toolUse('t1', 'Read', { file_path: '/w/app/parser.ts' }),
      ],
      {
        id: 'm1',
        usage: { input_tokens: 100, output_tokens: 20 },
      },
    ),
    toolResult(at(2), 't1', 'export function parse() { return 42 }'),
    assistant(at(3), [toolUse('t2', 'Edit', { file_path: '/w/app/parser.ts', old_string: '42', new_string: '43' })], {
      id: 'm1',
    }),
    toolResult(at(4), 't2', 'ok'),
    {
      type: 'system',
      subtype: 'compact_boundary',
      timestamp: at(5),
      compactMetadata: { trigger: 'auto', preTokens: 150000 },
    },
    user('Summary: parser fixed, tests pending', at(5), { isCompactSummary: true }),
    user('Now run the tests', at(6)),
    assistant(at(7), [text('All 12 tests pass.')], { id: 'm2', model: 'claude-other' }),
  ])
  addConversation(l, TOKEN, CWD, B, [
    user('Neighbour chat about deployment', at(10)),
    assistant(at(11), [text('Deploy notes.')]),
  ])
  // Same cwd, another token: must stay invisible.
  addConversation(l, OTHER_TOKEN, CWD, FOREIGN, [user('secret plan of another user parser', at(0))])
  // Same token, another project: must stay invisible.
  addConversation(l, TOKEN, OTHER_CWD, OTHER_PROJECT, [user('other project parser work', at(0))])
}

describe('project_sessions_list', () => {
  it('lists only the caller token and project, newest first, with flags', async () => {
    seedProject()
    const fileB = path.join(l.projectsDir)
    expect(fs.existsSync(fileB)).toBe(true)
    const ctx = ctxFor(l, TOKEN, CWD, { selfIds: [B], live: [{ ids: [A], attached: false }] })
    const { body, isError } = await call(ctx, 'project_sessions_list')
    expect(isError).toBe(false)
    const out = JSON.parse(body)
    const ids = out.conversations.map((c: any) => c.conversation_id)
    expect(ids.sort()).toEqual([A, B].sort())
    expect(body).not.toContain(FOREIGN)
    expect(body).not.toContain(OTHER_PROJECT)
    const a = out.conversations.find((c: any) => c.conversation_id === A)
    expect(a).toMatchObject({
      title: 'Fix the parser',
      live: true,
      attached: false,
      self: false,
      source: 'live',
      lines: 10,
    })
    expect(a.started_at).toBe(at(0))
    const b = out.conversations.find((c: any) => c.conversation_id === B)
    expect(b).toMatchObject({ title: 'Neighbour chat about deployment', self: true, live: false })
  })

  it('pages with limit/offset', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    const first = JSON.parse((await call(ctx, 'project_sessions_list', { limit: 1 })).body)
    expect(first.returned).toBe(1)
    expect(first.next_offset).toBe(1)
    const second = JSON.parse((await call(ctx, 'project_sessions_list', { limit: 1, offset: 1 })).body)
    expect(second.next_offset).toBeNull()
    expect(second.conversations[0].conversation_id).not.toBe(first.conversations[0].conversation_id)
  })

  it('does not match token directories that merely share a prefix', async () => {
    // tok / tok2 / tok-<other project> / tok with no cwd are all distinct namespaces.
    addConversation(l, 'tok', '/w/app', A, [user('mine', at(0))])
    addConversation(l, 'tok2', '/w/app', FOREIGN, [user('prefix token', at(0))])
    addConversation(l, 'tok', '/w/app2', OTHER_PROJECT, [user('prefix project', at(0))])
    addConversation(l, 'tok-w', '/app', B, [user('separator spoof', at(0))])
    addConversation(l, 'tok', '', 'dddddddd-0000-4000-8000-00000000000d', [user('no project', at(0))])
    const out = JSON.parse((await call(ctxFor(l, 'tok', '/w/app'), 'project_sessions_list')).body)
    expect(out.conversations.map((c: any) => c.conversation_id)).toEqual([A])
    const none = JSON.parse((await call(ctxFor(l, 'tok', ''), 'project_sessions_list')).body)
    expect(none.conversations.map((c: any) => c.conversation_id)).toEqual(['dddddddd-0000-4000-8000-00000000000d'])
  })

  it('includes archive-only transcripts of the project and never foreign archive entries', async () => {
    const { slugDir, file } = addConversation(l, TOKEN, CWD, A, [user('archived chat', at(0))])
    const foreign = addConversation(l, OTHER_TOKEN, CWD, FOREIGN, [user('foreign archived', at(0))])
    for (const [id, src, slug] of [
      [A, file, path.basename(slugDir)],
      [FOREIGN, foreign.file, path.basename(foreign.slugDir)],
    ] as const) {
      fs.mkdirSync(path.join(l.archiveRoot, id), { recursive: true })
      fs.copyFileSync(src, path.join(l.archiveRoot, id, 'transcript.jsonl'))
      fs.writeFileSync(path.join(l.archiveRoot, id, 'meta.json'), JSON.stringify({ slug, bytes: 1, archivedAt: at(0) }))
      fs.rmSync(src) // expired from projects/
    }
    const ctx = ctxFor(l, TOKEN, CWD)
    const out = JSON.parse((await call(ctx, 'project_sessions_list')).body)
    expect(out.conversations).toHaveLength(1)
    expect(out.conversations[0]).toMatchObject({ conversation_id: A, source: 'archive', title: 'archived chat' })
    expect((await call(ctx, 'project_session_read', { conversation_id: A })).body).toContain('archived chat')
    const foreignRead = await call(ctx, 'project_session_read', { conversation_id: FOREIGN })
    expect(foreignRead.isError).toBe(true)
    const excluded = JSON.parse((await call(ctx, 'project_sessions_list', { include_archived: false })).body)
    expect(excluded.conversations).toHaveLength(0)
  })
})

describe('project_session_read', () => {
  it('renders text mode with summarised tools and hides thinking', async () => {
    seedProject()
    const { body } = await call(ctxFor(l, TOKEN, CWD), 'project_session_read', { conversation_id: B })
    expect(body).toContain('Neighbour chat about deployment')
    const a = await call(ctxFor(l, TOKEN, CWD), 'project_session_read', { conversation_id: A })
    expect(a.body).toMatch(/#1 2026-10-20T10:00:00.000Z user\nPlease fix the parser bug/)
    expect(a.body).toContain('→ Read {"file_path":"/w/app/parser.ts"}')
    expect(a.body).toContain('--- compact boundary trigger=auto preTokens=150000')
    expect(a.body).toContain('Summary: parser fixed')
    expect(a.body).not.toContain('Need to read parser.ts')
    expect(a.body).not.toContain('return 42')
    expect(a.page).toMatchObject({ conversation_id: A, done: true, next_cursor: null, first_index: 1, last_index: 9 })
  })

  it('full mode shows thinking, tool arguments, results, usage and other rows', async () => {
    seedProject()
    const { body } = await call(ctxFor(l, TOKEN, CWD), 'project_session_read', { conversation_id: A, detail: 'full' })
    expect(body).toContain('[thinking]\nNeed to read parser.ts')
    expect(body).toContain('"new_string": "43"')
    expect(body).toContain('← result [t1]\nexport function parse() { return 42 }')
    expect(body).toContain('model=claude-test in=100 out=20')
    expect(body).toContain('#0 - other type=custom-title')
  })

  it('filters by kinds, time range, index range and tail', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    const users = await call(ctx, 'project_session_read', { conversation_id: A, kinds: ['user'] })
    expect(users.page.entries).toBe(2)
    const since = await call(ctx, 'project_session_read', { conversation_id: A, since: at(6) })
    expect(since.page.first_index).toBe(8)
    const range = await call(ctx, 'project_session_read', {
      conversation_id: A,
      detail: 'full',
      from_index: 3,
      to_index: 5,
    })
    expect([range.page.first_index, range.page.last_index]).toEqual([3, 4])
    const tail = await call(ctx, 'project_session_read', { conversation_id: A, tail: 2 })
    expect([tail.page.first_index, tail.page.last_index, tail.page.entries]).toEqual([8, 9, 2])
    const bad = await call(ctx, 'project_session_read', { conversation_id: A, kinds: ['nope'] })
    expect(bad.isError).toBe(true)
  })

  it('pages under the cap and the pages reassemble the whole rendering, including a split entry', async () => {
    const lines = [] as Array<Record<string, unknown>>
    for (let i = 0; i < 60; i++) lines.push(user(`prompt number ${i} ` + 'x'.repeat(200), at(i)))
    lines.push(user('BIG ' + 'y'.repeat(5000) + ' END', at(61)))
    addConversation(l, TOKEN, CWD, A, lines)
    const ctx = ctxFor(l, TOKEN, CWD)
    const whole = (await call(ctx, 'project_session_read', { conversation_id: A, max_chars: 200_000 })).body
    let cursor: string | null = null
    let joined = ''
    let pages = 0
    let sawPartial = false
    do {
      const r = await call(ctx, 'project_session_read', { conversation_id: A, max_chars: 1000, cursor })
      expect(r.body.length).toBeLessThanOrEqual(1000)
      joined += r.body
      sawPartial ||= r.page.partial_entry === true
      cursor = r.page.next_cursor
      pages++
    } while (cursor && pages < 500)
    expect(pages).toBeGreaterThan(10)
    expect(sawPartial).toBe(true)
    expect(joined).toBe(whole)
  })

  it('reads a subagent transcript by agent_id and refuses traversal agent ids', async () => {
    const { slugDir } = addConversation(l, TOKEN, CWD, A, [user('lead', at(0))])
    const sub = path.join(slugDir, A, 'subagents')
    fs.mkdirSync(sub, { recursive: true })
    fs.writeFileSync(path.join(sub, 'agent-a1b2.jsonl'), JSON.stringify(user('teammate task', at(1))) + '\n')
    fs.writeFileSync(
      path.join(sub, 'agent-a1b2.meta.json'),
      JSON.stringify({ agentType: 'general-purpose', description: 'Digest' }),
    )
    const ctx = ctxFor(l, TOKEN, CWD)
    expect((await call(ctx, 'project_session_read', { conversation_id: A, agent_id: 'agent-a1b2' })).body).toContain(
      'teammate task',
    )
    for (const agent_id of ['../a1b2', 'a1b2/../../x', '..', 'a b']) {
      const r = await call(ctx, 'project_session_read', { conversation_id: A, agent_id })
      expect(r.isError).toBe(true)
    }
    const list = JSON.parse((await call(ctx, 'project_session_subagents', { conversation_id: A })).body)
    expect(list.agents).toEqual([
      expect.objectContaining({
        agent_id: 'a1b2',
        agent_type: 'general-purpose',
        description: 'Digest',
        first_prompt: 'teammate task',
        lines: 1,
      }),
    ])
  })
})

describe('project_session_raw', () => {
  it('returns exact lines and pages reassemble the file, splitting a long line', async () => {
    const lines = [user('short', at(0)), user('long ' + 'z'.repeat(250_000), at(1)), user('после — юникод', at(2))]
    const { file } = addConversation(l, TOKEN, CWD, A, lines)
    const ctx = ctxFor(l, TOKEN, CWD)
    let cursor: string | null = null
    let joined = ''
    let n = 0
    do {
      const r = await call(ctx, 'project_session_raw', { conversation_id: A, max_chars: 200_000, cursor })
      expect(r.body.length).toBeLessThanOrEqual(200_000)
      joined += r.body
      cursor = r.page.next_cursor
      n++
    } while (cursor && n < 20)
    // short line; the long line split in two; the rest of it plus the last line
    expect(n).toBe(3)
    expect(joined).toBe(fs.readFileSync(file, 'utf8'))
    const tail = await call(ctx, 'project_session_raw', { conversation_id: A, tail: 1 })
    expect(tail.body).toBe(JSON.stringify(lines[2]) + '\n')
    expect(tail.page.first_index).toBe(2)
  })

  it('streams an oversized line by bytes without splitting characters', async () => {
    const big = JSON.stringify(user('Ж'.repeat(9 * 1024 * 1024), at(0))) // ~18 MiB > MAX_LINE_BYTES
    const { file } = addConversation(l, TOKEN, CWD, A, [big, user('after', at(1))])
    const ctx = ctxFor(l, TOKEN, CWD)
    readerStats.peakBufferedBytes = 0
    let cursor: string | null = null
    const parts: string[] = []
    let n = 0
    do {
      const r = await call(ctx, 'project_session_raw', { conversation_id: A, max_chars: 200_000, cursor })
      expect(r.body).not.toContain('\uFFFD')
      parts.push(r.body)
      cursor = r.page.next_cursor
      n++
    } while (cursor && n < 400)
    expect(parts.join('')).toBe(fs.readFileSync(file, 'utf8'))
    expect(readerStats.peakBufferedBytes).toBeLessThanOrEqual(16 * 1024 * 1024)
    const read = await call(ctx, 'project_session_read', { conversation_id: A })
    expect(read.body).toContain('#0 oversized entry')
    expect(read.body).toContain('after')
  }, 60_000)
})

describe('project_session_search', () => {
  it('finds literal and regex matches across the project only, with snippets and indices', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    const r = await call(ctx, 'project_session_search', { query: 'PARSER' })
    expect(r.body).toContain(`${A} #1 ${at(0)} user`)
    expect(r.body).not.toContain(FOREIGN)
    expect(r.body).not.toContain(OTHER_PROJECT)
    expect(r.body).not.toContain('another user')
    const cs = await call(ctx, 'project_session_search', { query: 'PARSER', case_sensitive: true })
    expect(cs.page.results).toBe(0)
    const re = await call(ctx, 'project_session_search', { query: 'return \\d+', regex: true })
    expect(re.body).toContain(`${A} #3`)
    const kinds = await call(ctx, 'project_session_search', { query: 'parser', kinds: ['tool_result'] })
    expect(kinds.page.results).toBe(0)
    const time = await call(ctx, 'project_session_search', { query: 'deploy', since: at(9) })
    expect(time.body).toContain(B)
  })

  it('pages results with a cursor and covers every hit exactly once', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    const all = await call(ctx, 'project_session_search', { query: 'a', max_results: 200 })
    let cursor: string | null = null
    let seen = 0
    let pages = 0
    do {
      const r = await call(ctx, 'project_session_search', { query: 'a', max_results: 1, cursor })
      seen += r.page.results
      cursor = r.page.next_cursor
      pages++
    } while (cursor && pages < 100)
    expect(seen).toBe(all.page.results)
    expect(seen).toBeGreaterThan(3)
  })

  it('searches subagents on request and refuses risky regexes', async () => {
    const { slugDir } = addConversation(l, TOKEN, CWD, A, [user('lead', at(0))])
    fs.mkdirSync(path.join(slugDir, A, 'subagents'), { recursive: true })
    fs.writeFileSync(
      path.join(slugDir, A, 'subagents', 'agent-x1.jsonl'),
      JSON.stringify(user('needle in agent', at(1))) + '\n',
    )
    const ctx = ctxFor(l, TOKEN, CWD)
    expect((await call(ctx, 'project_session_search', { query: 'needle' })).page.results).toBe(0)
    expect((await call(ctx, 'project_session_search', { query: 'needle', include_subagents: true })).body).toContain(
      `${A} agent=x1 #0`,
    )
    for (const q of ['(a+)+$', '(a|aa)*b', '(\\w+\\s?)*$', '(a)\\1', 'x'.repeat(300)]) {
      expect(() => compileSearch(q, true, false)).toThrow()
    }
    expect(() => compileSearch('foo(bar)?baz[+*]+', true, false)).not.toThrow()
    expect((await call(ctx, 'project_session_search', { query: '(a+)+', regex: true })).isError).toBe(true)
  })
})

describe('project_session_outline and project_session_brief', () => {
  it('outlines counts, compact segments, tools, files, models and usage', async () => {
    seedProject()
    const { body } = await call(ctxFor(l, TOKEN, CWD), 'project_session_outline', { conversation_id: A })
    const o = JSON.parse(body)
    expect(o.title).toBe('Fix the parser')
    expect(o.kinds).toMatchObject({
      user: 2,
      assistant: 3,
      tool_result: 2,
      compact_boundary: 1,
      compact_summary: 1,
      other: 1,
    })
    expect(o.compact_segments[0]).toMatchObject({ index: 6, trigger: 'auto', pre_tokens: 150000 })
    expect(o.tools).toEqual({ Read: 1, Edit: 1 })
    expect(o.files_touched).toEqual({ '/w/app/parser.ts': 2 })
    expect(o.usage.assistant_messages).toBe(2) // two lines of message m1 count once
    expect(o.models).toEqual({ 'claude-test': 1, 'claude-other': 1 })
    expect(o.first_prompts[0]).toEqual({ index: 1, text: 'Please fix the parser bug' })
    expect(o.git_branches).toEqual(['main'])
  })

  it('builds a continuation brief with the compact summary and last turns', async () => {
    seedProject()
    const { body } = await call(ctxFor(l, TOKEN, CWD), 'project_session_brief', { conversation_id: A, turns: 1 })
    expect(body).toContain('# Continuation brief: Fix the parser')
    expect(body).toContain('## Latest compact summary (entry #7)\nSummary: parser fixed, tests pending')
    expect(body).toContain('**User:** Now run the tests')
    expect(body).toContain('**Assistant:** All 12 tests pass.')
    expect(body).not.toContain('**User:** Please fix the parser bug')
    expect(body).toContain('- /w/app/parser.ts (2)')
  })
})

describe('isolation and hardening', () => {
  it('answers foreign, other-project, missing and malformed ids identically', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    const tools: ProjectSessionToolName[] = [
      'project_session_read',
      'project_session_raw',
      'project_session_outline',
      'project_session_subagents',
      'project_session_brief',
    ]
    for (const tool of tools) {
      for (const id of [FOREIGN, OTHER_PROJECT, '99999999-0000-4000-8000-000000000000']) {
        const r = await call(ctx, tool, { conversation_id: id })
        expect(r.isError).toBe(true)
        expect(r.body).toBe(`Error: ${NOT_FOUND}`)
      }
      for (const id of ['../' + FOREIGN, '..\\x', 'a/b', '', '.', `${A}.jsonl`, 'x'.repeat(200), 42]) {
        const r = await call(ctx, tool, { conversation_id: id })
        expect(r.isError).toBe(true)
        expect(r.body).toBe(`Error: ${NOT_FOUND}`)
      }
    }
    // The other token sees its own conversation and nothing of ours.
    const theirs = JSON.parse((await call(ctxFor(l, OTHER_TOKEN, CWD), 'project_sessions_list')).body)
    expect(theirs.conversations.map((c: any) => c.conversation_id)).toEqual([FOREIGN])
    expect((await call(ctxFor(l, OTHER_TOKEN, CWD), 'project_session_read', { conversation_id: A })).isError).toBe(true)
    // A traversal-shaped token id never resolves.
    expect((await call(ctxFor(l, '../' + OTHER_TOKEN, CWD), 'project_sessions_list')).isError).toBe(true)
  })

  it('refuses symlinked slug directories, transcripts and session directories', async (t) => {
    const foreign = addConversation(l, OTHER_TOKEN, CWD, FOREIGN, [user('foreign secret', at(0))])
    const mine = addConversation(l, TOKEN, CWD, A, [user('mine', at(0))])
    const ctx = ctxFor(l, TOKEN, CWD)
    // 1) my slug dir replaced by a link to the foreign slug dir
    fs.rmSync(mine.slugDir, { recursive: true })
    if (!trySymlink(foreign.slugDir, mine.slugDir, 'dir')) return t.skip()
    expect(JSON.parse((await call(ctx, 'project_sessions_list')).body).conversations).toHaveLength(0)
    expect((await call(ctx, 'project_session_read', { conversation_id: FOREIGN })).isError).toBe(true)
    fs.rmSync(mine.slugDir, { recursive: true, force: true })
    try {
      fs.unlinkSync(mine.slugDir)
    } catch {
      /* already gone */
    }
    // 2) a session dir in my project dir that links to the foreign settings dir
    const linkedSession = path.join(path.dirname(mine.settingsDir), 'linked-session')
    trySymlink(foreign.settingsDir, linkedSession, 'dir')
    expect(JSON.parse((await call(ctx, 'project_sessions_list')).body).conversations).toHaveLength(0)
    // 3) a transcript file in my slug dir that links to a foreign transcript
    fs.mkdirSync(mine.slugDir, { recursive: true })
    if (trySymlink(foreign.file, path.join(mine.slugDir, `${B}.jsonl`), 'file')) {
      expect(JSON.parse((await call(ctx, 'project_sessions_list')).body).conversations).toHaveLength(0)
      expect((await call(ctx, 'project_session_raw', { conversation_id: B })).isError).toBe(true)
      expect((await call(ctx, 'project_session_search', { query: 'foreign' })).page.results).toBe(0)
    }
  })

  it('refuses a symlinked subagents directory', async (t) => {
    const foreign = addConversation(l, OTHER_TOKEN, CWD, FOREIGN, [user('x', at(0))])
    fs.mkdirSync(path.join(foreign.slugDir, FOREIGN, 'subagents'), { recursive: true })
    fs.writeFileSync(
      path.join(foreign.slugDir, FOREIGN, 'subagents', 'agent-s1.jsonl'),
      JSON.stringify(user('foreign agent', at(0))) + '\n',
    )
    const mine = addConversation(l, TOKEN, CWD, A, [user('mine', at(0))])
    fs.mkdirSync(path.join(mine.slugDir, A), { recursive: true })
    if (!trySymlink(path.join(foreign.slugDir, FOREIGN, 'subagents'), path.join(mine.slugDir, A, 'subagents'), 'dir'))
      return t.skip()
    const ctx = ctxFor(l, TOKEN, CWD)
    expect((await call(ctx, 'project_session_read', { conversation_id: A, agent_id: 's1' })).isError).toBe(true)
    expect(JSON.parse((await call(ctx, 'project_session_subagents', { conversation_id: A })).body).agents).toEqual([])
  })

  it('never writes: the whole tree is unchanged after every tool runs', async () => {
    seedProject()
    const before = snapshotTree(l.root)
    const ctx = ctxFor(l, TOKEN, CWD)
    await call(ctx, 'project_sessions_list')
    await call(ctx, 'project_session_read', { conversation_id: A, detail: 'full' })
    await call(ctx, 'project_session_raw', { conversation_id: A })
    await call(ctx, 'project_session_search', { query: 'parser', include_subagents: true })
    await call(ctx, 'project_session_outline', { conversation_id: A })
    await call(ctx, 'project_session_subagents', { conversation_id: A })
    await call(ctx, 'project_session_brief', { conversation_id: A })
    expect(snapshotTree(l.root)).toBe(before)
  })

  it('redacts credentials in every content tool', async () => {
    const secret = 'sk-ant-api03-' + 'Q'.repeat(40)
    addConversation(l, TOKEN, CWD, A, [
      user(`my key is ${secret} ok`, at(0)),
      assistant(at(1), [text(`Authorization: Bearer ${'t'.repeat(30)}`)]),
    ])
    const ctx = ctxFor(l, TOKEN, CWD)
    for (const [tool, args] of [
      ['project_session_read', { conversation_id: A }],
      ['project_session_raw', { conversation_id: A }],
      ['project_session_search', { query: 'my key' }],
      ['project_session_brief', { conversation_id: A }],
      ['project_session_outline', { conversation_id: A }],
      ['project_sessions_list', {}],
    ] as const) {
      const r = await call(ctx, tool, args)
      const all = r.res.content.map((c) => c.text).join('\n')
      expect(all, tool).not.toContain(secret)
      expect(all, tool).not.toContain('t'.repeat(30))
    }
    const raw = await call(ctx, 'project_session_raw', { conversation_id: A })
    expect(raw.body).toContain('[REDACTED:anthropic-key]')
    for (const line of raw.body.trim().split('\n')) expect(() => JSON.parse(line)).not.toThrow()
    expect(raw.page.redactions).toBe(2)
  })

  it('logs metadata only: tool, outcome, target and size, never content', async () => {
    seedProject()
    const ctx = ctxFor(l, TOKEN, CWD)
    await call(ctx, 'project_session_read', { conversation_id: A })
    await call(ctx, 'project_session_read', { conversation_id: FOREIGN })
    await call(ctx, 'project_session_search', { query: 'parser' })
    expect(logs[0]).toMatchObject({ tool: 'project_session_read', outcome: 'ok', conversationId: A, self: false })
    expect(logs[0]!.bytes).toBeGreaterThan(100)
    expect(logs[1]).toEqual({ tool: 'project_session_read', outcome: 'not_found', conversationId: FOREIGN, bytes: 0 })
    expect(JSON.stringify(logs)).not.toMatch(/parser|Please fix/)
  })
})

describe('bounded memory', () => {
  it('streams a large transcript: peak buffered bytes stay at one line', async () => {
    const line = JSON.stringify(user('filler ' + 'w'.repeat(900), at(0)))
    const { file } = addConversation(l, TOKEN, CWD, A, [])
    const fd = fs.openSync(file, 'w')
    const chunk = (line + '\n').repeat(1000)
    for (let i = 0; i < 40; i++) fs.writeSync(fd, chunk) // ~37 MB, 40k lines
    fs.writeSync(fd, JSON.stringify(user('the very end', at(99))) + '\n')
    fs.closeSync(fd)
    const ctx = ctxFor(l, TOKEN, CWD)
    readerStats.peakBufferedBytes = 0
    const tail = await call(ctx, 'project_session_read', { conversation_id: A, tail: 1 })
    expect(tail.body).toContain('the very end')
    expect(tail.page.first_index).toBe(40_000)
    const search = await call(ctx, 'project_session_search', { query: 'very end' })
    expect(search.page.results).toBe(1)
    const outline = JSON.parse((await call(ctx, 'project_session_outline', { conversation_id: A })).body)
    expect(outline.lines).toBe(40_001)
    const brief = await call(ctx, 'project_session_brief', { conversation_id: A, turns: 2 })
    expect(brief.body).toContain('the very end')
    expect(readerStats.peakBufferedBytes).toBeLessThan(4 * 1024)
  }, 60_000)
})
