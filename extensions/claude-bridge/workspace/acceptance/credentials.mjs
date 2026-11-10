// Real first-run registration and cookie persistence against the supplied server.
// No credential, transcript, browser profile, trace or screenshot is retained.
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { parseArgs } from 'node:util'
import { randomUUID } from 'node:crypto'
import { chromium } from 'playwright'
import { check, outcome, exitStatus, markdown, CheckFailure } from './report.mjs'
import { command, OwnedResources, poll, upstreamHealth } from './resources.mjs'
import { ready, raw, newTab, turn, transcript, complete } from './browser.mjs'

async function main() {
  check(process.platform === 'linux' && process.env.BRIDGEWEB_ACC_CONTAINER === '1', 'USE_PODMAN_CONTAINER_RUNNER')
  check(process.env.PLAYWRIGHT_BROWSERS_PATH && process.env.PLAYWRIGHT_SKIP_BROWSER_GC === '1', 'PRIVATE_BROWSER_CACHE_REQUIRED')
  const { values } = parseArgs({ options: {
    image: { type: 'string', default: 'localhost/bridgeweb-acc-workspace:local' },
    'server-origin': { type: 'string', default: 'http://host.containers.internal:3462' },
    'report-dir': { type: 'string' }, strict: { type: 'boolean', default: false },
  } })
  check(values['report-dir'] && /^[a-f0-9]{40}$/.test(process.env.BRIDGEWEB_ACC_SOURCE ?? ''), 'REPORT_AND_SOURCE_REQUIRED')
  const server = new URL(values['server-origin'])
  check(server.protocol === 'http:' && ['127.0.0.1', 'host.containers.internal'].includes(server.hostname) &&
    !server.username && !server.password && server.pathname === '/' && !server.search && !server.hash, 'EXPECTED_LOOPBACK_SERVER_ORIGIN')
  const owned = new OwnedResources(values.image)
  const directory = path.join(values['report-dir'], owned.name)
  await mkdir(directory, { recursive: true, mode: 0o700 })
  const report = { started: new Date().toISOString(), source: process.env.BRIDGEWEB_ACC_SOURCE,
    sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY === '1', results: [], metrics: {}, cleanup: [] }
  const record = async (name, fn) => {
    let error
    try { await fn() } catch (e) { error = e }
    report.results.push(outcome(name, error, values.strict))
    process.stdout.write(`${name}: ${report.results.at(-1).status}\n`)
    return !error
  }
  let browser, tokenId, context, page, before, conversationId, nonce
  const fingerprint = () => owned.exec(`const fs=require('fs'),c=require('crypto'),p=process.env.HOME+'/.local/state/bridge/';
    const b=fs.readFileSync(p+'workspace-credentials.json');
    process.stdout.write(JSON.stringify({digest:c.createHash('sha256').update(b).digest('hex'),
      mode:fs.statSync(p+'workspace-credentials.json').mode&511,
      sessionMode:fs.statSync(p+'workspace-browser-sessions.json').mode&511,
      dirMode:fs.statSync(p).mode&511}));`).then(JSON.parse)
  const sameCookie = async () => {
    const response = await context.request.get(`${owned.origin}/auth/session`)
    check(response.ok() && (await response.json()).authenticated && !response.headers()['set-cookie'], 'OLD_COOKIE_SURVIVED')
    check(JSON.stringify(await context.cookies()) === before.cookies, 'COOKIE_UNCHANGED')
    check(JSON.stringify(await fingerprint()) === before.fingerprint, 'PRIVATE_CREDENTIALS_UNCHANGED')
    await ready(page, owned.origin)
    check(!await page.locator('#workspace-login').isVisible(), 'NO_SIGN_IN_PAGE')
  }
  try {
    const started = await record('fresh-volume-auto-registration', async () => {
      check(await upstreamHealth(server), 'UPSTREAM_UNREACHABLE')
      const [image] = JSON.parse(await command('podman', ['image', 'inspect', values.image]))
      report.imageId = image.Id
      report.imageSource = image.Config?.Labels?.['org.opencontainers.image.revision'] || 'unspecified'
      await owned.start(undefined, `ws://host.containers.internal:${server.port || 80}`)
      // Resolve only this run's own automatically registered identity in-container.
      // The secret never crosses this pipe or enters the report.
      tokenId = await owned.exec(`(async()=>{const c=JSON.parse(require('fs').readFileSync(process.env.HOME+'/.local/state/bridge/workspace-credentials.json'));
        const r=await fetch(c.server+'/api/dashboard/tokens/resolve',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({token:c.token})});
        if(!r.ok)process.exit(1);const b=await r.json();if(typeof b.id!=='string')process.exit(1);process.stdout.write(b.id)})().catch(()=>process.exit(1))`)
      check(tokenId.length > 0 && tokenId.length < 256, 'OWN_IDENTITY')
    })
    if (started) {
      const loggedIn = await record('fresh-browser-no-sign-in', async () => {
        browser = await chromium.launch({ headless: true })
        context = await browser.newContext()
        page = await context.newPage()
        await ready(page, owned.origin)
        check(!await page.locator('#workspace-login').isVisible(), 'NO_SIGN_IN_PAGE')
        const cookies = await context.cookies()
        check(cookies.length === 1 && cookies[0].httpOnly && cookies[0].sameSite === 'Strict', 'PRIVATE_STRICT_COOKIE')
        const privateFiles = await fingerprint()
        check(privateFiles.mode === 0o600 && privateFiles.sessionMode === 0o600 && privateFiles.dirMode === 0o700, 'PRIVATE_VOLUME_MODES')
        before = { cookies: JSON.stringify(cookies), fingerprint: JSON.stringify(privateFiles) }
      })
      if (loggedIn) {
        const chatted = await record('conversation-before-restart', async () => {
          const id = await newTab(page)
          nonce = `acceptance-${randomUUID()}`
          await turn(page, `Remember this exact marker for our conversation: ${nonce}. Reply with exactly ${nonce}:ok and use no tools.`)
          complete(await transcript(page, nonce))
          conversationId = (await raw(page, 'listTabs')).find(t => t.id === id)?.conversationId
          check(typeof conversationId === 'string' && conversationId.length > 0, 'CONVERSATION_ID')
        })
        const restarted = await record('restart-same-cookie-and-credentials', async () => {
          await owned.restart()
          await sameCookie()
        })
        if (chatted && restarted) await record('same-agent-conversation-after-restart', async () => {
          // The existing Resume action reattaches the server conversation. This
          // asserts conversation continuity, not survival of local child PIDs.
          const config = await raw(page, 'getConfig')
          const id = await raw(page, 'resumeSession', { ...config, cwd: '/workspace', conversationId })
          await page.evaluate(id => window.bridgeCmd.switchTab(id), id)
          await poll(async () => (await transcript(page, nonce)).done, { timeout: 30_000, code: 'HISTORY_AFTER_RESTART' })
          const followup = `acceptance-${randomUUID()}`
          await turn(page, `What exact marker did I ask you to remember before the workspace restarted? Reply with that marker followed by :ok, then ${followup}:ok. Use no tools.`)
          check(await page.evaluate(({ nonce, followup }) => {
            const id = window.bridgeCmd.state().activeTabId
            return (window.__jsonlEntries?.[id] ?? []).some(e => {
              if (e.type !== 'assistant') return false
              const content = e.message?.content
              const text = typeof content === 'string' ? content : (content ?? []).filter(b => b.type === 'text').map(b => b.text ?? '').join('\n')
              return text.includes(`${nonce}:ok`) && text.includes(`${followup}:ok`)
            })
          }, { nonce, followup }), 'AGENT_MEMORY_AFTER_RESTART')
          const snapshot = await transcript(page, followup)
          complete(snapshot)
          check((await raw(page, 'listTabs')).find(t => t.id === id)?.conversationId === conversationId, 'SAME_CONVERSATION')
        })
        await record('recreate-same-volume-without-bootstrap', async () => {
          await owned.recreate()
          await sameCookie()
        })
        await record('foreign-origin-and-rebinding-rejected', async () => {
          for (const headers of [{ Origin: 'http://foreign.invalid' }, { Host: 'foreign.invalid' }, { 'Sec-Fetch-Site': 'cross-site' }]) {
            const response = await context.request.get(`${owned.origin}/auth/session`, { headers })
            check(response.status() === 403 && !response.headers()['set-cookie'], 'FOREIGN_REQUEST_REJECTED')
          }
        })
      }
    }
  } finally {
    try { await browser?.close() } catch { report.cleanup.push('BROWSER_CLOSE') }
    report.cleanup.push(...await owned.cleanup())
    if (tokenId) {
      try {
        const response = await fetch(new URL(`/api/dashboard/tokens/${encodeURIComponent(tokenId)}`, server), { method: 'DELETE', signal: AbortSignal.timeout(10_000), redirect: 'error' })
        if (!response.ok) report.cleanup.push('OWN_TOKEN_REVOKE')
      } catch { report.cleanup.push('OWN_TOKEN_REVOKE') }
    }
    const required = ['fresh-volume-auto-registration', 'fresh-browser-no-sign-in', 'conversation-before-restart',
      'restart-same-cookie-and-credentials', 'same-agent-conversation-after-restart', 'recreate-same-volume-without-bootstrap', 'foreign-origin-and-rebinding-rejected']
    for (const name of required) if (!report.results.some(r => r.name === name)) report.results.push({ name, status: 'BLOCKED', reason: 'Required prerequisite failed; scenario not run.' })
    if (report.cleanup.length) report.results.push({ name: 'cleanup', status: 'FAIL', reason: report.cleanup.join(', ') })
    report.metrics.conversationRecovery = 'Existing Resume action after host restart; completed server conversation and model context. Does not assert in-flight container-local process survival.'
    report.exitCode = exitStatus(report.results)
    await writeFile(path.join(directory, 'summary.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 })
    await writeFile(path.join(directory, 'summary.md'), markdown(report), { flag: 'wx', mode: 0o600 })
    process.stdout.write(`Sanitized summary: ${path.join(directory, 'summary.md')}\n`)
    process.exitCode = report.exitCode
  }
}
main().catch(error => {
  process.stderr.write(`Credentials acceptance stopped: ${error instanceof CheckFailure ? error.code : 'UNEXPECTED_ERROR'} (raw error omitted).\n`)
  process.exitCode = 1
})
