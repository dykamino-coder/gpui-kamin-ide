import { readFile } from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { check, CheckFailure } from './report.mjs'
import { poll } from './resources.mjs'
import { raw, ready, newTab, turn, transcript, complete, used, heap, requirePanel } from './browser.mjs'

const nonce = () => `acceptance-${randomUUID()}`
const finish = n => `When finished reply with exactly ${n}:ok.`
export function liveScenarios(state) {
  const { owned, context, metrics } = state
  const file = '/workspace/bridgeweb-acc-file.txt'
  return {
    async chat() {
      await newTab(state.page)
      const n = nonce()
      await turn(state.page, `Reply with exactly ${n}:ok. Do not use tools.`)
      complete(await transcript(state.page, n))
    },
    async 'file-tools'() {
      const n = nonce()
      await turn(state.page, `Use the Write tool to write alpha followed by one newline into ${file}. Use Read to read it, then Edit to replace alpha with beta, then Read again. Do not use Bash for these operations. ${finish(n)}`)
      const t = await transcript(state.page, n)
      complete(t)
      for (const tool of ['Read', 'Write', 'Edit']) used(t, tool)
      check((await owned.read(file)).trim() === 'beta', 'FILE_BYTES')
      const result = await raw(state.page, 'readFileAsBase64', file)
      check(result.ok && Buffer.from(result.base64, 'base64').toString().trim() === 'beta', 'BRIDGE_READ_BYTES')
    },
    async bash() {
      const n = nonce()
      await turn(state.page, `Use Bash to run: printf 'bash-ok\\n' > /workspace/bridgeweb-acc-bash. ${finish(n)}`)
      const t = await transcript(state.page, n)
      complete(t); used(t, 'Bash')
      check((await owned.read('/workspace/bridgeweb-acc-bash')).trim() === 'bash-ok', 'BASH_BYTES')
    },
    async mcp() {
      await owned.write('/workspace/bridgeweb-acc-mcp.mjs', await readFile(new URL('./mcp-fixture.mjs', import.meta.url), 'utf8'))
      const server = await raw(state.page, 'addMcpServer', { name: 'acceptance', type: 'stdio',
        command: 'node', args: ['/workspace/bridgeweb-acc-mcp.mjs'], workingDirectory: '/workspace', enabled: true })
      check(typeof server.id === 'string', 'MCP_CONFIG')
      check((await raw(state.page, 'testMcpServer', server.id)).ok, 'MCP_DISCOVERY')
      await newTab(state.page)
      const n = nonce()
      await turn(state.page, `Find and call the acceptance MCP server's acceptance_record tool with nonce ${n}. Do not simulate it with Bash or Write. ${finish(n)}`)
      const t = await transcript(state.page, n)
      complete(t)
      check(t.tools.some(tool => tool.includes('acceptance_record')), 'MCP_TOOL_TRACE')
      check((await owned.read('/workspace/bridgeweb-acc-mcp-result')).trim() === n, 'MCP_BYTES')
    },
    async hook() {
      const n = nonce()
      const saved = await raw(state.page, 'hooksSave', { event: 'PostToolUse', matcher: '.*Bash$',
        handler: { type: 'command', host: 'local', command: `printf '${n}\\n' >> /workspace/bridgeweb-acc-hook` } })
      check(saved.ok, 'HOOK_CONFIG')
      // Hook changes apply on the next server session, rather than to an old CLI.
      await newTab(state.page)
      await turn(state.page, `Use Bash to run printf 'hook-trigger\\n'. ${finish(n)}`)
      complete(await transcript(state.page, n))
      await poll(async () => (await owned.read('/workspace/bridgeweb-acc-hook')).includes(n), { code: 'HOOK_BYTES' })
      const log = await raw(state.page, 'hooksGetLog', 100)
      check(log.entries.some(e => e.event === 'PostToolUse' && e.exitCode === 0), 'HOOK_LOG')
    },
    async 'file-panel'() {
      // Produce a fresh changed-file event in the active session before opening its viewer.
      const n = nonce()
      await turn(state.page, `Use Read on ${file}, then Edit to replace beta with gamma. ${finish(n)}`)
      complete(await transcript(state.page, n))
      check((await owned.read(file)).trim() === 'gamma', 'FILE_BYTES')
      await requirePanel(state.page, file, 'gamma')
    },
    async 'close-mid-turn'() {
      const page = state.page
      await newTab(page)
      const before = nonce()
      await turn(page, `Reply with exactly ${before}:ok.`)
      const baseline = (await transcript(page, before)).ids
      const tab = await page.evaluate(() => window.bridgeCmd.state().activeTabId)
      const n = nonce()
      await page.evaluate(prompt => window.bridgeCmd.send(prompt),
        `Use Bash to run this exact foreground command (do not background): printf 'started\\n' > /workspace/bridgeweb-acc-long-start; sleep 15; printf 'finished\\n' > /workspace/bridgeweb-acc-long-finish. ${finish(n)}`)
      await poll(async () => (await owned.read('/workspace/bridgeweb-acc-long-start')).trim() === 'started', { timeout: 120_000, code: 'BASH_START' })
      check(!(await owned.read('/workspace/bridgeweb-acc-long-finish')), 'BASH_ALREADY_FINISHED')
      check(context.pages().length === 1, 'SOLE_BROWSER_PAGE')
      await page.close()
      let continued = false
      try {
        await poll(async () => (await owned.read('/workspace/bridgeweb-acc-long-finish')).trim() === 'finished', { timeout: 45_000, code: 'CONTINUATION' })
        continued = true
      } finally {
        state.page = await context.newPage()
        await ready(state.page, owned.origin)
      }
      check(continued, 'CONTINUATION')
      const tabs = await raw(state.page, 'listTabs')
      check(tabs.some(t => t.id === tab), 'SNAPSHOT')
      await state.page.evaluate(tab => window.bridgeCmd.switchTab(tab), tab)
      try { await state.page.evaluate(() => window.bridgeCmd.waitForIdle({ timeoutMs: 120_000 })) }
      catch { throw new CheckFailure('SNAPSHOT') }
      complete(await transcript(state.page, n), { baseline, code: 'SNAPSHOT' })
      complete(await transcript(state.page, before), { baseline, code: 'SNAPSHOT' })
    },
    async 'reopen-permission'() {
      const page = state.page
      const tab = await newTab(page, 'default')
      await page.evaluate(() => {
        window.__acceptancePermission = undefined
        window.__acceptanceUnsubscribe = window.bridgeCmd.raw('onPermissionRequest', (id, req) => {
          window.__acceptancePermission = { tab: id, id: req.requestId, shell: req.toolName === 'Bash',
            expectedInput: JSON.stringify(req.input).includes('/workspace/bridgeweb-acc-permission') }
        })
      })
      const n = nonce()
      await page.evaluate(prompt => window.bridgeCmd.send(prompt),
        `Use Bash to run: printf 'approved\\n' > /workspace/bridgeweb-acc-permission. ${finish(n)}`)
      await page.waitForFunction(() => !!window.__acceptancePermission, null, { timeout: 120_000 })
      const pending = await page.evaluate(() => window.__acceptancePermission)
      check(pending.tab === tab && pending.shell && pending.expectedInput &&
        !(await owned.read('/workspace/bridgeweb-acc-permission')), 'PENDING_PERMISSION')
      try { await page.getByRole('button', { name: 'Allow', exact: true }).waitFor({ timeout: 10_000 }) }
      catch { throw new CheckFailure('PERMISSION_INITIAL_WIDGET') }
      check(context.pages().length === 1, 'SOLE_BROWSER_PAGE')
      await page.close()
      state.page = await context.newPage()
      await ready(state.page, owned.origin)
      check((await raw(state.page, 'listTabs')).some(t => t.id === tab), 'PERMISSION_TAB')
      try {
        // switchTab normally awaits idle, which cannot happen before this prompt is answered.
        await state.page.evaluate(tab => { void window.bridgeCmd.switchTab(tab).catch(() => {}) }, tab)
        await state.page.waitForFunction(tab => window.bridgeCmd.state().activeTabId === tab, tab)
      } catch { throw new CheckFailure('PERMISSION_BINDING') }
      try { await state.page.getByRole('button', { name: 'Allow', exact: true }).waitFor({ timeout: 15_000 }) }
      catch { throw new CheckFailure('PERMISSION_REOPEN_WIDGET') }
      try { await state.page.getByText('/workspace/bridgeweb-acc-permission', { exact: false }).first().waitFor({ timeout: 5_000 }) }
      catch { throw new CheckFailure('PERMISSION_REOPEN_INPUT') }
      try { await state.page.getByRole('button', { name: 'Allow', exact: true }).click({ timeout: 10_000 }) }
      catch { throw new CheckFailure('PERMISSION_ALLOW_CLICK') }
      await state.page.evaluate(() => window.bridgeCmd.waitForIdle({ timeoutMs: 120_000 }))
      check((await owned.read('/workspace/bridgeweb-acc-permission')).trim() === 'approved', 'PERMISSION_BYTES')
      complete(await transcript(state.page, n), { code: 'PERMISSION_REHYDRATION' })
    },
    async 'two-browser-tabs'() {
      const tab = await newTab(state.page)
      const other = await context.newPage()
      try {
        await ready(other, owned.origin)
        check((await raw(other, 'listTabs')).some(t => t.id === tab), 'SNAPSHOT')
        await other.evaluate(tab => window.bridgeCmd.switchTab(tab), tab)
        const n = nonce()
        await turn(state.page, `Reply with exactly ${n}:ok.`)
        await other.evaluate(() => window.bridgeCmd.waitForIdle({ timeoutMs: 120_000 }))
        const a = await transcript(state.page, n), b = await transcript(other, n)
        complete(a); complete(b, { baseline: a.ids, code: 'SNAPSHOT' })
        check(a.ids.length === b.ids.length, 'SNAPSHOT')
      } finally { await other.close() }
    },
    async 'browser-metrics'() {
      metrics.heapAfter = await heap(context, state.page)
      metrics.browser = context.browser().version()
      check(metrics.heapAfter.usedBytes > 0, 'HEAP')
    },
  }
}
