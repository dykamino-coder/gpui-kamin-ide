// engine-compare browser driver: ONE long-lived browser per lane, a fresh context (Chromium) or tab (Firefox) per page.
// Usage: node browser-driver.cjs <chrome|edge|firefox|servo> <driver-config.json>
// driver-config: {exe, cls: "gpu"|"sw", workDir, playwright, selenium, geckodriver}
// Protocol (JSON lines): stdout "READY {...}", "MARK <name>" (the controller snapshots CPU/memory of the tree and
// answers "ACK" on stdin before the page continues), "RESULT {...}". stdin: {"op":"page",...} | {"op":"quit"} | "ACK".
// Page op: {probe, cases:"R,C,...", steadyMs, cpuCases:[...], cpuFrames, trace, navTimeoutMs}
// Metric definitions match the probe page (window.__probeStart / window.__loop): see ec/metrics.py.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const readline = require('node:readline');
const { pathToFileURL } = require('node:url');
const { performance: clock } = require('node:perf_hooks');

const [engine, cfgFile] = process.argv.slice(2);
const cfg = JSON.parse(fs.readFileSync(cfgFile, 'utf8'));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const out = (kind, data) => process.stdout.write(kind + (data === undefined ? '' : ' ' + JSON.stringify(data)) + '\n');

const lines = []; let waiter = null;
readline.createInterface({ input: process.stdin }).on('line', l => { if (waiter) { const w = waiter; waiter = null; w(l); } else lines.push(l); })
  .on('close', () => { lines.push('{"op":"quit"}'); if (waiter) waiter('{"op":"quit"}'); });
const nextLine = () => lines.length ? Promise.resolve(lines.shift()) : new Promise(r => { waiter = r; });
async function mark(name) { out('MARK ' + name); const a = await nextLine(); if (a !== 'ACK') throw new Error('expected ACK, got ' + a); }

const SETTLE = `return new Promise(done => { requestAnimationFrame(() => requestAnimationFrame(() => {
  const n = performance.getEntriesByType('navigation')[0] || {};
  done({ elements: document.getElementsByTagName('*').length, loadEventEndMs: n.loadEventEnd, twoRafMs: performance.now(),
         viewport: [innerWidth, innerHeight, devicePixelRatio] }); })); });`;
const PROBE = cases => `window.__b4cases = ${JSON.stringify(cases)}; return new Promise(done => window.__probeStart(done));`;
const LOOP = (c, n) => `return new Promise(done => window.__loop(${JSON.stringify(c)}, ${n}, done));`;
const tmpDir = path.join(cfg.workDir, 'tmp'); fs.mkdirSync(tmpDir, { recursive: true });
const env = { ...process.env, TEMP: tmpDir, TMP: tmpDir };

const TRACE_CATEGORIES = 'devtools.timeline,blink,loading,cc,viz,gpu,disabled-by-default-devtools.timeline';
// ---- Chromium trace summary: complete events by name inside a window of console.timeStamp markers ----
const NAMES = ['ParseHTML', 'ParseAuthorStyleSheet', 'UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint', 'Layerize', 'Commit',
  'RasterTask', 'Display::DrawAndSwap'];
function summarize(evs) {
  const st = {};
  for (const e of evs) if (e.name === 'TimeStamp' && e.args && e.args.data && e.args.data.message) st[e.args.data.message] = e.ts;
  const win = (a, b) => {
    const o = {};
    for (const nm of NAMES) o[nm] = { sumMs: 0, maxMs: 0, count: 0 };
    for (const e of evs) {
      if (e.ph !== 'X' || e.ts < a || e.ts > b || !o[e.name]) continue;
      const d = (e.dur || 0) / 1000, w = o[e.name]; w.count++; w.sumMs += d; if (d > w.maxMs) w.maxMs = d;
    }
    return o;
  };
  const windows = {};
  if (st.nav && st.loaded) windows.cold = win(st.nav, st.loaded);
  for (const k of Object.keys(st)) {
    const m = /^(frame|req):(\w+):0$/.exec(k);
    if (m && st[`${m[1]}:${m[2]}:1`]) windows[`${m[1]}:${m[2]}`] = win(st[k], st[`${m[1]}:${m[2]}:1`]);
  }
  return { stamps: Object.keys(st).length, windows };
}

async function chromium() {
  const pw = require(cfg.playwright);
  const args = ['--no-first-run', '--no-default-browser-check'].concat(cfg.cls === 'sw' ? ['--disable-gpu', '--disable-gpu-compositing'] : []);
  const t0 = clock.now();
  const browser = await pw.chromium.launch({ executablePath: cfg.exe, headless: true, timeout: 60000, env, args });
  const info = { version: browser.version(), launchMs: clock.now() - t0, args };
  async function page(op) {
    const w0 = clock.now();
    const ctx = await browser.newContext({ viewport: { width: 800, height: 600 }, deviceScaleFactor: 1 });
    try {
      const pg = await ctx.newPage(); await pg.goto('about:blank');
      const cdp = op.trace ? await ctx.newCDPSession(pg) : null;
      let done = null;
      if (cdp) {
        await cdp.send('Tracing.start', { categories: op.traceCategories || TRACE_CATEGORIES, transferMode: 'ReturnAsStream' });
        await pg.evaluate(() => console.timeStamp('nav'));
      }
      await mark('nav');
      const navStart = clock.now();
      await pg.goto(pathToFileURL(op.probe).href, { waitUntil: 'load', timeout: op.navTimeoutMs || 180000 });
      const metrics = await pg.evaluate(new Function(SETTLE));
      if (cdp) await pg.evaluate(() => console.timeStamp('loaded'));
      const navWallMs = clock.now() - navStart;
      await mark('loaded');
      if (op.steadyMs) { await sleep(op.steadyMs); await mark('steady_end'); }
      const p0 = clock.now();
      const probe = op.cases ? await pg.evaluate(new Function(PROBE(op.cases))) : null;
      const probeWallMs = clock.now() - p0;
      let trace = null; const t0 = clock.now();
      if (cdp) {
        const completed = new Promise(r => cdp.once('Tracing.tracingComplete', r));
        await cdp.send('Tracing.end'); const ev = await completed; const chunks = [];
        while (true) { const c = await cdp.send('IO.read', { handle: ev.stream }); chunks.push(c.base64Encoded ? Buffer.from(c.data, 'base64') : Buffer.from(c.data)); if (c.eof) break; }
        await cdp.send('IO.close', { handle: ev.stream });
        const data = JSON.parse(Buffer.concat(chunks).toString('utf8')); trace = summarize(data.traceEvents || data);
      }
      const cpu = {}; const c0 = clock.now();
      for (const c of op.cpuCases || []) { await mark('cpu_' + c + '_0'); cpu[c] = await pg.evaluate(new Function(LOOP(c, op.cpuFrames))); await mark('cpu_' + c + '_1'); }
      return { navWallMs, probeWallMs, traceWallMs: c0 - t0, cpuWallMs: clock.now() - c0, pageWallMs: clock.now() - w0, ...metrics, probe, cpu, trace };
    } finally { await ctx.close().catch(() => {}); }
  }
  return { info, page, quit: () => Promise.race([browser.close().catch(() => {}), sleep(3000)]) };
}

async function firefox() {
  const selenium = require(cfg.selenium); const ff = require(path.join(cfg.selenium, 'firefox'));
  const profile = path.join(cfg.workDir, 'profile'); fs.mkdirSync(profile, { recursive: true });
  const prefs = { 'app.update.auto': false, 'browser.shell.checkDefaultBrowser': false, 'layout.css.devPixelsPerPx': '1.0',
    'toolkit.telemetry.enabled': false, 'privacy.reduceTimerPrecision': false, 'privacy.resistFingerprinting': false,
    'dom.send_after_paint_to_content': true, 'browser.tabs.warnOnClose': false };
  if (cfg.cls === 'sw') { prefs['gfx.webrender.software'] = true; prefs['layers.acceleration.disabled'] = true; }
  fs.writeFileSync(path.join(profile, 'user.js'), Object.entries(prefs).map(([k, v]) => `user_pref(${JSON.stringify(k)}, ${JSON.stringify(v)});`).join('\n') + '\n');
  const options = new ff.Options().setBinary(cfg.exe).addArguments('-headless', '-no-remote', '-profile', profile, '--width=800', '--height=600');
  for (const [k, v] of Object.entries(prefs)) options.setPreference(k, v);
  const service = new ff.ServiceBuilder(cfg.geckodriver).addArguments('--allow-system-access').setEnvironment(env).setStdio(['ignore', 'ignore', 'ignore']);
  const t0 = clock.now();
  const driver = await new selenium.Builder().forBrowser('firefox').setFirefoxOptions(options).setFirefoxService(service).build();
  await driver.manage().setTimeouts({ pageLoad: 180000, script: 300000 });
  await driver.manage().window().setRect({ width: 800, height: 600 });
  const cs = await driver.executeScript('return [outerWidth-innerWidth, outerHeight-innerHeight]');
  await driver.manage().window().setRect({ width: 800 + cs[0], height: 600 + cs[1] });
  const caps = await driver.getCapabilities(); const home = await driver.getWindowHandle();
  const info = { version: caps.get('browserVersion'), launchMs: clock.now() - t0, prefs };
  const run = body => driver.executeAsyncScript('const done=arguments[arguments.length-1];(function(){' + body + '})().then(done);');
  async function page(op) {
    const w0 = clock.now();
    await driver.switchTo().newWindow('tab');
    try {
      await driver.get('about:blank');
      await mark('nav');
      const navStart = clock.now();
      await driver.get(pathToFileURL(op.probe).href);
      const metrics = await run(SETTLE);
      const navWallMs = clock.now() - navStart;
      await mark('loaded');
      if (op.steadyMs) { await sleep(op.steadyMs); await mark('steady_end'); }
      const p0 = clock.now();
      const probe = op.cases ? await run(PROBE(op.cases)) : null;
      const probeWallMs = clock.now() - p0;
      const cpu = {}; const c0 = clock.now();
      for (const c of op.cpuCases || []) { await mark('cpu_' + c + '_0'); cpu[c] = await run(LOOP(c, op.cpuFrames)); await mark('cpu_' + c + '_1'); }
      return { navWallMs, probeWallMs, cpuWallMs: clock.now() - c0, pageWallMs: clock.now() - w0, ...metrics, probe, cpu };
    } finally { await driver.close().catch(() => {}); await driver.switchTo().window(home).catch(() => {}); }
  }
  return { info, page, quit: () => Promise.race([driver.quit().catch(() => {}), sleep(5000)]) };
}

// Servo: servoshell's own WebDriver server (--webdriver=PORT), driven like Firefox in one window (no new tab per page).
async function servo() {
  const selenium = require(cfg.selenium); const { spawn } = require('node:child_process'); const net = require('node:net');
  const port = await new Promise((ok, no) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => ok(p)); }).on('error', no); });
  const profile = path.join(cfg.workDir, 'profile'); fs.mkdirSync(profile, { recursive: true });
  const t0 = clock.now();
  const child = spawn(cfg.exe, ['--headless', `--webdriver=${port}`, '--window-size=800x600', '--screen-size=800x600',
    '--device-pixel-ratio=1', '--temporary-storage', '--config-dir', profile], { env, stdio: 'ignore', windowsHide: true });
  let driver = null;
  for (let i = 0; i < 100 && !driver; i++) {
    try { driver = await new selenium.Builder().usingServer(`http://127.0.0.1:${port}`).withCapabilities({ browserName: 'servo' }).build(); }
    catch (e) { if (child.exitCode !== null) throw new Error('servoshell exited: ' + child.exitCode); await sleep(200); }
  }
  if (!driver) throw new Error('servoshell WebDriver did not answer within 20 s');
  await driver.manage().setTimeouts({ pageLoad: 180000, script: 300000 }).catch(() => {});
  const info = { version: 'servoshell', launchMs: clock.now() - t0, webdriverPort: port };
  const run = body => driver.executeAsyncScript('const done=arguments[arguments.length-1];(function(){' + body + '})().then(done);');
  // Request -> pixels for Servo: the edit (one probe loop frame) then a WebDriver screenshot, which Servo answers only
  // once the rendering is up to date with no pending frame (components/servo/webview.rs take_screenshot); the same with
  // no edit ('idle') is the transport + encode cost the caller subtracts. S: one 120 px step from the top per sample.
  async function screenshots(cases, n) {
    const out = {}; const time = async f => { const a = clock.now(); await f(); await driver.takeScreenshot(); return clock.now() - a; };
    for (const k of ['idle'].concat(cases)) {
      const t = []; let moved = 0;
      if (k === 'S') await driver.executeScript('window.scrollTo(0,0)');
      for (let i = 0; i < n; i++) {
        if (k === 'S') {
          t.push(await time(async () => { moved += await driver.executeScript('var a=scrollY;window.scrollBy(0,120);return Math.abs(scrollY-a-120)<0.5?1:0;'); }));
        } else {
          t.push(await time(() => run(LOOP(k, 1))));
        }
      }
      if (k === 'S') { await driver.executeScript('window.scrollTo(0,0)'); out[k] = { ms: t.slice(0, moved), moved }; }
      else out[k] = { ms: t };
    }
    return out;
  }
  async function page(op) {    const w0 = clock.now();
    await driver.get('about:blank');
    await mark('nav');
    const navStart = clock.now();
    await driver.get(pathToFileURL(op.probe).href);
    const metrics = await run(SETTLE);
    const navWallMs = clock.now() - navStart;
    await mark('loaded');
    if (op.steadyMs) { await sleep(op.steadyMs); await mark('steady_end'); }
    const p0 = clock.now();
    const probe = op.cases ? await run(PROBE(op.cases)) : null;
    const probeWallMs = clock.now() - p0;
    const s0 = clock.now(); const shots = op.cases ? await screenshots(op.cases.split(','), op.shots || 5) : null;
    const shotWallMs = clock.now() - s0;
    const cpu = {}; const c0 = clock.now();
    for (const c of op.cpuCases || []) { await mark('cpu_' + c + '_0'); cpu[c] = await run(LOOP(c, op.cpuFrames)); await mark('cpu_' + c + '_1'); }
    return { navWallMs, probeWallMs, shotWallMs, cpuWallMs: clock.now() - c0, pageWallMs: clock.now() - w0, ...metrics, probe, shots, cpu };
  }
  const quit = () => Promise.race([driver.quit().catch(() => {}), sleep(5000)]).then(() => { try { child.kill(); } catch (e) { /* gone */ } });
  return { info, page, quit };
}

(async () => {  let b;
  try { b = engine === 'firefox' ? await firefox() : engine === 'servo' ? await servo() : await chromium(); out('READY', b.info); }
  catch (e) { out('READY', { error: String(e && e.stack || e) }); process.exit(2); }
  while (true) {
    const l = await nextLine(); let op;
    try { op = JSON.parse(l); } catch (e) { continue; }
    if (op.op === 'quit') break;
    if (op.op !== 'page') continue;
    let r;
    try { r = { status: 'ok', ...(await b.page(op)) }; } catch (e) { r = { status: 'error', error: String(e && e.stack || e) }; }
    out('RESULT', r);
  }
  await b.quit(); process.exit(0);
})();
