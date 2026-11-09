import { mkdir, writeFile, realpath } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'
import { chromium } from 'playwright'
import { check, scenarios, outcome, exitStatus, markdown, CheckFailure, assertImageSource, completionReceipt } from './report.mjs'
import { command, OwnedResources, imageProbe, issueToken, upstreamHealth } from './resources.mjs'
import { heap } from './browser.mjs'
import { liveScenarios } from './scenarios.mjs'

async function main() {
  check(process.platform === 'linux' && process.env.BRIDGEWEB_ACC_CONTAINER === '1', 'USE_PODMAN_CONTAINER_RUNNER')
  const { values } = parseArgs({ options: {
    image: { type: 'string', default: 'localhost/bridgeweb-acc-workspace:local' },
    'server-origin': { type: 'string', default: 'http://127.0.0.1:3462' },
    'report-dir': { type: 'string' }, strict: { type: 'boolean', default: false },
  } })
  check(values['report-dir'], 'REPORT_DIR_REQUIRED')
  check(process.env.PLAYWRIGHT_BROWSERS_PATH && process.env.PLAYWRIGHT_SKIP_BROWSER_GC === '1', 'PRIVATE_BROWSER_CACHE_REQUIRED')
  const repo = await realpath(fileURLToPath(new URL('../../../../', import.meta.url)))
  const destination = path.resolve(values['report-dir'])
  await mkdir(destination, { recursive: true, mode: 0o700 })
  const reportRoot = await realpath(destination)
  const relative = path.relative(repo, reportRoot)
  check(relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative), 'REPORT_MUST_BE_OUTSIDE_REPO')
  const server = new URL(values['server-origin'])
  check(server.protocol === 'http:' && ['127.0.0.1', 'host.containers.internal'].includes(server.hostname) && !server.username && !server.password &&
    server.pathname === '/' && !server.search && !server.hash, 'EXPECTED_LOOPBACK_SERVER_ORIGIN')
  const owned = new OwnedResources(values.image)
  const reportDir = path.join(reportRoot, owned.name)
  await mkdir(reportDir, { mode: 0o700 })
  check(/^[a-f0-9]{40}$/.test(process.env.BRIDGEWEB_ACC_SOURCE ?? ''), 'SOURCE_REVISION_REQUIRED')
  const report = { started: new Date().toISOString(), source: process.env.BRIDGEWEB_ACC_SOURCE,
    sourceDirty: process.env.BRIDGEWEB_ACC_SOURCE_DIRTY === '1',
    results: [], metrics: {}, cleanup: [] }
  const record = async (name, fn, blockedReason) => {
    const started = Date.now()
    let error
    try { await fn() } catch (e) { error = e }
    const result = { ...outcome(name, error, values.strict), durationMs: Date.now() - started }
    if (error && blockedReason) { result.status = 'BLOCKED'; result.reason = blockedReason }
    report.results.push(result)
    process.stdout.write(`${name}: ${result.status}\n`)
    return !error
  }
  let browser, token, imageReady = false, cliReady = false, credentialsReady = false
  try {
    imageReady = await record('image-contract', async () => {
      const [config] = JSON.parse(await command('podman', ['image', 'inspect', values.image]))
      report.imageId = config.Id
      report.imageSource = config.Config?.Labels?.['org.opencontainers.image.revision'] || 'unspecified'
      // Reject an old/concurrently retagged image before probing it or issuing
      // a token. A passing run must identify the same clean candidate twice.
      assertImageSource(report)
      // Tags can move while another build runs on the shared Podman engine.
      // Probe and start the immutable image that was actually inspected.
      owned.image = report.imageId
      check(config.Config?.User && !['root', '0', '0:0'].includes(config.Config.User), 'IMAGE_NONROOT')
      check((config.Healthcheck ?? config.Config?.Healthcheck)?.Test?.length, 'IMAGE_HEALTHCHECK_USE_DOCKER_FORMAT')
      const probe = JSON.parse(await command('podman', ['run', '--rm', '--name', `${owned.name}-probe`,
        '--tmpfs', '/workspace:rw,mode=1777',
        '--tmpfs', '/home/workspace:rw,mode=1777',
        '--entrypoint', 'node', report.imageId, '-e', imageProbe]))
      check(probe.uid !== 0 && probe.bundle.raw > 0, 'IMAGE_CONTRACT')
      cliReady = probe.cli
      credentialsReady = probe.credentialsMemoryOnly
      report.metrics.bundle = { ...probe.bundle, method: 'all shipped .js files; individual gzip/Brotli sums' }
    })
    if (imageReady && cliReady) await record('credential-storage', () => check(credentialsReady, 'TOKEN_PERSISTENCE'))
    const upstreamReady = await record('upstream-health', async () => {
      check(await upstreamHealth(server), 'UPSTREAM_UNREACHABLE')
    }, 'Supplied loopback Bridge server /health is unreachable; no token issued and no server resources changed.')
    let workspaceReady = false
    if (imageReady && !cliReady) await record('workspace-start', () => check(false, 'CORE_ENTRYPOINT'))
    if (imageReady && cliReady && credentialsReady && upstreamReady) {
      workspaceReady = await record('workspace-start', async () => {
        token = await issueToken(server, owned.name)
        await owned.start(token.token, `ws://host.containers.internal:${server.port || 80}`)
      })
    }
    if (workspaceReady) {
      const loggedIn = await record('browser-login', async () => {
        browser = await chromium.launch({ headless: true })
        const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, colorScheme: 'dark' })
        const page = await context.newPage()
        await page.goto(owned.origin, { waitUntil: 'domcontentloaded' })
        await page.waitForFunction(() => !!window.bridgeCmd, null, { timeout: 30_000 })
        check(!await page.locator('#workspace-login').isVisible(), 'LOOPBACK_AUTO_LOGIN')
        check(await page.evaluate(secret => ![...Object.values(localStorage), ...Object.values(sessionStorage)].some(v => v.includes(secret)), token.token), 'NO_TOKEN_IN_STORAGE')
        report.metrics.heapStartup = await heap(context, page)
        report.metrics.browser = browser.version()
        report.state = { owned, context, page, metrics: report.metrics, screenshot: path.join(reportDir, 'layout.png') }
      })
      if (loggedIn) {
        const tests = liveScenarios(report.state)
        for (const [name, test] of Object.entries(tests)) await record(name, test)
      }
    }
  } finally {
    try { await browser?.close() } catch { report.cleanup.push('BROWSER_CLOSE') }
    report.cleanup.push(...await owned.cleanup())
    if (token) {
      try {
        const response = await fetch(new URL(`/api/dashboard/tokens/${encodeURIComponent(token.id)}`, server), {
          method: 'DELETE', signal: AbortSignal.timeout(10_000), redirect: 'error',
        })
        if (!response.ok) report.cleanup.push('OWN_TOKEN_REVOKE')
      } catch { report.cleanup.push('OWN_TOKEN_REVOKE') }
      token = undefined
    }
    // Runtime objects and credential-bearing browser handles must never be serialized.
    delete report.state
    for (const name of scenarios) if (!report.results.some(r => r.name === name)) {
      report.results.push({ name, status: 'BLOCKED', reason: 'A required image/server/workspace/browser prerequisite did not pass; scenario not run.' })
    }
    if (report.cleanup.length) report.results.push({ name: 'cleanup', status: 'FAIL', reason: report.cleanup.join(', ') })
    report.exitCode = exitStatus(report.results)
    await writeFile(path.join(reportDir, 'summary.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 })
    await writeFile(path.join(reportDir, 'summary.md'), markdown(report), { flag: 'wx', mode: 0o600 })
    await writeFile(path.join(reportDir, 'completion.json'), `${JSON.stringify(completionReceipt(report), null, 2)}\n`, { flag: 'wx', mode: 0o600 })
    process.stdout.write(`Sanitized summary: ${path.join(reportDir, 'summary.md')}\n`)
    process.exitCode = report.exitCode
  }
}
main().catch(error => {
  process.stderr.write(`Acceptance runner stopped: ${error instanceof CheckFailure ? error.code : 'UNEXPECTED_ERROR'} (raw error omitted).\n`)
  process.exitCode = 1
})
