import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import net from 'node:net'
import { CheckFailure, check } from './report.mjs'

export const delay = ms => new Promise(resolve => setTimeout(resolve, ms))
export async function poll(test, { timeout = 30_000, code = 'TIMEOUT' } = {}) {
  const deadline = Date.now() + timeout
  do { if (await test()) return; await delay(250) } while (Date.now() < deadline)
  throw new CheckFailure(code)
}
// Neither command arguments nor output/error text are forwarded to reports.
export function command(file, args, { input, timeout = 60_000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(file, args, { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', oversized = false
    const timer = setTimeout(() => { child.kill(); reject(new CheckFailure('COMMAND_TIMEOUT')) }, timeout)
    child.on('error', () => { clearTimeout(timer); reject(new CheckFailure('COMMAND_START')) })
    child.stdout.on('data', b => {
      if (output.length + b.length <= 4 * 1024 * 1024) output += b
      else oversized = true
    })
    child.stderr.resume()
    child.stdin.on('error', () => {})
    child.stdin.end(input)
    child.on('close', code => {
      clearTimeout(timer)
      if (code !== 0 || oversized) reject(new CheckFailure('COMMAND_EXIT'))
      else resolve(output)
    })
  })
}
export async function freePort() {
  const server = net.createServer()
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve) })
  const port = server.address().port
  await new Promise(resolve => server.close(resolve))
  return port
}
export async function upstreamHealth(origin, pathname = '/health') {
  try { return (await fetch(new URL(pathname, origin), { signal: AbortSignal.timeout(5000), redirect: 'error' })).ok }
  catch { return false }
}
export async function issueToken(origin, name) {
  const response = await fetch(new URL('/api/dashboard/tokens', origin), {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name }),
    signal: AbortSignal.timeout(10_000), redirect: 'error',
  })
  check(![401, 403].includes(response.status), 'ADMIN_CREDENTIALS_REQUIRED')
  check(response.status === 201, 'TOKEN_ISSUE')
  const result = await response.json()
  check(typeof result.token === 'string' && result.token && typeof result.id === 'string', 'TOKEN_SHAPE')
  return result
}
export class OwnedResources {
  constructor(image) {
    this.name = `bridgeweb-acc-${randomUUID()}`
    this.image = image
    this.volumes = []
    this.started = false
  }
  async start(token, upstream) {
    const port = await freePort()
    this.origin = `http://127.0.0.1:${port}`
    this.port = port
    this.upstream = upstream
    for (const suffix of ['project', 'home']) {
      const name = `${this.name}-${suffix}`
      // Random exact names only. Never adopt or clean up an existing volume.
      try { await command('podman', ['volume', 'inspect', name]); throw new CheckFailure('RESOURCE_COLLISION') }
      catch (error) { if (error.code !== 'COMMAND_EXIT') throw error }
      await command('podman', ['volume', 'create', name])
      this.volumes.push(name)
    }
    await this.launch(token)
  }
  async launch(token) {
    this.exitCode = undefined
    this.launchError = false
    this.child = spawn('podman', ['run', '--name', this.name, '-i',
      '--network', 'host', '--add-host', 'host.containers.internal:127.0.0.1',
      '-v', `${this.volumes[0]}:/workspace`, '-v', `${this.volumes[1]}:/home/workspace`,
      '-e', `BRIDGE_SERVER_URL=${this.upstream}`, '-e', `WORKSPACE_ORIGIN=${this.origin}`,
      '-e', 'WORKSPACE_BIND=127.0.0.1', '-e', `WORKSPACE_PORT=${this.port}`,
      this.image, ...(token === undefined ? [] : ['--token-stdin'])], { windowsHide: true, stdio: ['pipe', 'ignore', 'ignore'] })
    this.started = true
    this.child.on('error', () => { this.launchError = true })
    this.closed = new Promise(resolve => this.child.once('close', code => { this.exitCode = code; resolve() }))
    this.child.stdin.on('error', () => {})
    // Secret crosses stdin once; it is never a Podman argument, file or configured env.
    this.child.stdin.end(token === undefined ? undefined : `${token}\n`)
    await this.healthy()
  }
  async healthy() {
    await poll(async () => {
      check(!this.launchError, 'COMMAND_START')
      if (this.exitCode !== undefined) throw new CheckFailure(this.exitCode === 78 ? 'CORE_ENTRYPOINT' : 'CONTAINER_EXIT')
      return upstreamHealth(this.origin, '/health')
    }, { timeout: 45_000, code: 'WORKSPACE_HEALTH' })
  }
  async restart() {
    await command('podman', ['restart', '--time', '10', this.name])
    await poll(() => upstreamHealth(this.origin, '/health'), { timeout: 45_000, code: 'WORKSPACE_HEALTH' })
  }
  async recreate() {
    await command('podman', ['stop', '--time', '10', this.name])
    await this.closed
    await command('podman', ['rm', this.name])
    await this.launch() // Same volumes, port and identity; no bootstrap secret.
  }
  exec(script, input) { return command('podman', ['exec', '-i', this.name, 'node', '-e', script], { input }) }
  async write(path, text) {
    await this.exec(`let s='';process.stdin.setEncoding('utf8');process.stdin.on('data',x=>s+=x);process.stdin.on('end',()=>require('fs').writeFileSync(${JSON.stringify(path)},s));`, text)
  }
  async read(path) {
    return this.exec(`try{process.stdout.write(require('fs').readFileSync(${JSON.stringify(path)}))}catch(e){if(e.code!=='ENOENT')process.exitCode=1}`)
  }
  async cleanup() {
    const failures = []
    if (this.started) {
      // Inspect only this run's exact container name.
      let exists = false
      try { await command('podman', ['container', 'inspect', this.name]); exists = true }
      catch (e) { if (e.code !== 'COMMAND_EXIT') failures.push('CONTAINER_INSPECT') }
      if (exists) {
        try { await command('podman', ['stop', '--time', '5', this.name]) }
        catch { failures.push('CONTAINER_STOP') }
        try { await command('podman', ['rm', this.name]) }
        catch { failures.push('CONTAINER_REMOVE') }
      }
      if (this.child && this.exitCode === undefined) {
        await Promise.race([this.closed, delay(10_000)])
        if (this.exitCode === undefined) { this.child.kill(); failures.push('OWN_CLIENT_EXIT') }
      }
    }
    for (const volume of this.volumes) {
      try { await command('podman', ['volume', 'rm', volume]) }
      catch { failures.push('VOLUME_REMOVE') }
    }
    return failures
  }
}

export const imageProbe = String.raw`
const fs=require('fs'),path=require('path'),z=require('zlib'),cp=require('child_process');
const root='/opt/bridge/extensions/claude-bridge';
let chunks=0,raw=0,gzip=0,brotli=0;
function walk(p){for(const e of fs.readdirSync(p,{withFileTypes:true})){const f=path.join(p,e.name);if(e.isDirectory())walk(f);else if(f.endsWith('.js')){const b=fs.readFileSync(f);chunks++;raw+=b.length;gzip+=z.gzipSync(b).length;brotli+=z.brotliCompressSync(b).length}}}
walk(root+'/webview/dist/browser');
fs.writeFileSync('/workspace/bridgeweb-acc-probe','ok');fs.unlinkSync('/workspace/bridgeweb-acc-probe');
fs.writeFileSync(process.env.HOME+'/bridgeweb-acc-probe','ok');fs.unlinkSync(process.env.HOME+'/bridgeweb-acc-probe');
cp.execFileSync('bash',['-c','true']);cp.execFileSync('pwsh',['-NoProfile','-Command','exit 0']);
const cli=fs.existsSync(root+'/workspace/dist/cli.cjs');
let credentialsMemoryOnly=false;
if(cli){
 const script="const fs=require('fs');const marker='bridgeweb-acc-synthetic-not-a-credential';const backend=require('./dist/runtime.cjs').createSharedBackend({root:'/workspace',storage:process.env.HOME+'/probe-state',serverUrl:'ws://127.0.0.1:1',token:marker,version:'0.0.0',environment:{}});function scan(p){return fs.readdirSync(p,{withFileTypes:true}).some(e=>e.isDirectory()?scan(p+'/'+e.name):e.isFile()&&fs.readFileSync(p+'/'+e.name).includes(marker))}const leaked=scan(process.env.HOME);backend.dispose();process.exit(leaked?42:0)";
 const result=cp.spawnSync('node',['-e',script],{cwd:root+'/workspace',timeout:15000,stdio:'ignore'});
 if(![0,42].includes(result.status))throw new Error('Credential probe failed');
 credentialsMemoryOnly=result.status===0;
}
process.stdout.write(JSON.stringify({uid:process.getuid(),cli,credentialsMemoryOnly,bundle:{chunks,raw,gzip,brotli}}));`
