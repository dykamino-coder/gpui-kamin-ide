// engine-compare browser driver: ONE long-lived browser per lane, a fresh context (Chromium) or tab (Firefox) per page.
// Usage: node browser-driver.cjs <chrome|edge|firefox> <driver-config.json>
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
    const ctx = await browser.newContext({ viewport: { width: 800, height: 600 }, deviceScaleFactor: 1 });
    try {
      const pg = await ctx.newPage(); await pg.goto('about:blank');
      const cdp = op.trace ? await ctx.newCDPSession(pg) : null;
      let done = null;
      if (cdp) {
        await cdp.send('Tracing.start', { categories: 'devtools.timeline,blink,loading,cc,viz,gpu,disabled-by-default-devtools.timeline', transferMode: 'ReturnAsStream' });
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
      const probe = op.cases ? await pg.evaluate(new Function(PROBE(op.cases))) : null;
      let trace = null;
      if (cdp) {
        const completed = new Promise(r => cdp.once('Tracing.tracingComplete', r));
        await cdp.send('Tracing.end'); const ev = await completed; const chunks = [];
        while (true) { const c = await cdp.send('IO.read', { handle: ev.stream }); chunks.push(c.base64Encoded ? Buffer.from(c.data, 'base64') : Buffer.from(c.data)); if (c.eof) break; }
        await cdp.send('IO.close', { handle: ev.stream });
        const data = JSON.parse(Buffer.concat(chunks).toString('utf8')); trace = summarize(data.traceEvents || data);
      }
      const cpu = {};
      for (const c of op.cpuCases || []) { await mark('cpu_' + c + '_0'); cpu[c] = await pg.evaluate(new Function(LOOP(c, op.cpuFrames))); await mark('cpu_' + c + '_1'); }
      return { navWallMs, ...metrics, probe, cpu, trace };
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
      const probe = op.cases ? await run(PROBE(op.cases)) : null;
      const cpu = {};
      for (const c of op.cpuCases || []) { await mark('cpu_' + c + '_0'); cpu[c] = await run(LOOP(c, op.cpuFrames)); await mark('cpu_' + c + '_1'); }
      return { navWallMs, ...metrics, probe, cpu };
    } finally { await driver.close().catch(() => {}); await driver.switchTo().window(home).catch(() => {}); }
  }
  return { info, page, quit: () => Promise.race([driver.quit().catch(() => {}), sleep(5000)]) };
}

(async () => {
  let b;
  try { b = engine === 'firefox' ? await firefox() : await chromium(); out('READY', b.info); }
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
