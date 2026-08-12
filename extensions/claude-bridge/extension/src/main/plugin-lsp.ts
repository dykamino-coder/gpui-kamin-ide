import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { listEnabledInstalledPlugins, loadPluginOptions, readEffectivePluginManifest, substituteUserConfig } from './plugin-helpers'

interface PluginLspConfig {
  id: string
  pluginId: string
  pluginRoot: string
  serverKey: string
  command: string
  args: string[]
  extensionToLanguage: Record<string, string>
  env: Record<string, string>
  initializationOptions?: unknown
  settings?: unknown
  workspaceFolder?: string
  transport: 'stdio' | 'socket'
  startupTimeout: number
  shutdownTimeout: number
  restartOnCrash: boolean
  maxRestarts: number
  diagnostics: boolean
}

export interface PluginLspSummary {
  id: string
  pluginId: string
  serverKey: string
  extensions: string[]
  initialized: boolean
  error?: string
}

type JsonRpcMessage = { jsonrpc?: string; id?: number | string; method?: string; params?: any; result?: any; error?: any }
type Pending = { resolve: (value: any) => void; reject: (err: Error) => void; timer: ReturnType<typeof setTimeout> }

export const LSP_MAX_HEADER_BYTES = 16 * 1024
export const LSP_MAX_CONTENT_BYTES = 16 * 1024 * 1024
export const LSP_MAX_STDOUT_BUFFER_BYTES = LSP_MAX_HEADER_BYTES + 4 + LSP_MAX_CONTENT_BYTES

/** Stateful Content-Length frame decoder with strict memory/protocol bounds. */
export class LspFrameDecoder {
  private buffer = Buffer.alloc(0)

  reset(): void { this.buffer = Buffer.alloc(0) }

  push(chunk: Buffer): JsonRpcMessage[] {
    if (chunk.length > LSP_MAX_STDOUT_BUFFER_BYTES - this.buffer.length) {
      throw new Error(`stdout buffer exceeded ${LSP_MAX_STDOUT_BUFFER_BYTES} bytes`)
    }
    this.buffer = Buffer.concat([this.buffer, chunk])
    const messages: JsonRpcMessage[] = []

    while (true) {
      const marker = this.buffer.indexOf('\r\n\r\n')
      if (marker < 0) {
        if (this.buffer.length > LSP_MAX_HEADER_BYTES) {
          throw new Error(`header exceeded ${LSP_MAX_HEADER_BYTES} bytes`)
        }
        return messages
      }
      if (marker > LSP_MAX_HEADER_BYTES) throw new Error(`header exceeded ${LSP_MAX_HEADER_BYTES} bytes`)

      const header = this.buffer.subarray(0, marker).toString('ascii')
      const lengthHeaders = header.split('\r\n').filter(line => /^Content-Length\s*:/i.test(line))
      if (lengthHeaders.length !== 1) throw new Error('frame must contain exactly one Content-Length header')
      const value = lengthHeaders[0].slice(lengthHeaders[0].indexOf(':') + 1).trim()
      if (!/^\d+$/.test(value)) throw new Error('frame contains an invalid Content-Length header')
      const length = Number(value)
      if (!Number.isSafeInteger(length)) throw new Error('frame Content-Length is not a safe integer')
      if (length > LSP_MAX_CONTENT_BYTES) throw new Error(`frame Content-Length exceeded ${LSP_MAX_CONTENT_BYTES} bytes`)

      const frameBytes = marker + 4 + length
      if (this.buffer.length < frameBytes) return messages
      const body = this.buffer.subarray(marker + 4, frameBytes).toString('utf-8')
      this.buffer = this.buffer.subarray(frameBytes)
      let message: unknown
      try { message = JSON.parse(body) } catch { throw new Error('frame body is not valid JSON') }
      if (!message || typeof message !== 'object' || Array.isArray(message)) {
        throw new Error('frame body is not a JSON-RPC object')
      }
      messages.push(message as JsonRpcMessage)
    }
  }
}

let resolveProject: (tabId: string | null | undefined, file: string) => string | undefined = (_tabId, file) => path.dirname(file)

export function setPluginLspProjectResolver(resolver: typeof resolveProject): void {
  resolveProject = resolver
}

function isInside(root: string, candidate: string): boolean {
  const rel = path.relative(path.resolve(root), path.resolve(candidate))
  return rel === '' || (rel !== '..' && !rel.startsWith('..' + path.sep) && !path.isAbsolute(rel))
}

function readJson(filePath: string): unknown {
  try { return JSON.parse(fs.readFileSync(filePath, 'utf-8')) } catch { return null }
}

function collectConfigMap(target: Record<string, unknown>, value: unknown): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return
  for (const [key, config] of Object.entries(value as Record<string, unknown>)) {
    if (config && typeof config === 'object' && !Array.isArray(config)) target[key] = config
  }
}

async function discoverPluginLspConfigs(): Promise<PluginLspConfig[]> {
  const out: PluginLspConfig[] = []
  for (const plugin of await listEnabledInstalledPlugins()) {
    const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
    const rawMap: Record<string, unknown> = {}
    collectConfigMap(rawMap, readJson(path.join(plugin.installPath, '.lsp.json')))

    const declared = manifest.lspServers
    const declarations = Array.isArray(declared) ? declared : declared === undefined ? [] : [declared]
    for (const item of declarations) {
      if (typeof item === 'string') {
        const resolved = path.resolve(plugin.installPath, item)
        if (isInside(plugin.installPath, resolved)) collectConfigMap(rawMap, readJson(resolved))
      } else {
        collectConfigMap(rawMap, item)
      }
    }

    const options = loadPluginOptions(plugin.key)
    for (const [serverKey, raw] of Object.entries(rawMap)) {
      const config = raw as Record<string, any>
      if (typeof config.command !== 'string' || !config.command.trim()) continue
      if (!config.extensionToLanguage || typeof config.extensionToLanguage !== 'object' || Array.isArray(config.extensionToLanguage)) continue
      const subst = (value: string, project = '${CLAUDE_PROJECT_DIR}'): string => substituteUserConfig(value, options)
        .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, plugin.installPath)
        .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, path.join(os.homedir(), '.claude', 'plugins', 'data', plugin.key.replace(/[^a-zA-Z0-9\-_]/g, '-')))
        .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, project)
        .replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (match: string, name: string) =>
          name === 'CLAUDE_PROJECT_DIR' ? match : (process.env[name] ?? ''))
      const env: Record<string, string> = {}
      for (const [key, value] of Object.entries(config.env ?? {})) if (typeof value === 'string') env[key] = subst(value)
      for (const [key, value] of Object.entries(options)) env[`CLAUDE_PLUGIN_OPTION_${key.toUpperCase()}`] = String(value)
      out.push({
        id: `${plugin.key}/${serverKey}`,
        pluginId: plugin.key,
        pluginRoot: plugin.installPath,
        serverKey,
        command: subst(config.command),
        args: Array.isArray(config.args) ? config.args.filter((v: unknown): v is string => typeof v === 'string').map(v => subst(v)) : [],
        extensionToLanguage: Object.fromEntries(Object.entries(config.extensionToLanguage).filter(([, v]) => typeof v === 'string')) as Record<string, string>,
        env,
        initializationOptions: config.initializationOptions,
        settings: config.settings,
        workspaceFolder: typeof config.workspaceFolder === 'string' ? config.workspaceFolder : undefined,
        transport: config.transport === 'socket' ? 'socket' : 'stdio',
        startupTimeout: typeof config.startupTimeout === 'number' && config.startupTimeout > 0 ? config.startupTimeout : 15_000,
        shutdownTimeout: typeof config.shutdownTimeout === 'number' && config.shutdownTimeout > 0 ? config.shutdownTimeout : 5_000,
        restartOnCrash: config.restartOnCrash !== false,
        maxRestarts: typeof config.maxRestarts === 'number' && config.maxRestarts >= 0 ? Math.floor(config.maxRestarts) : 3,
        diagnostics: config.diagnostics !== false,
      })
    }
  }
  return out.sort((a, b) => a.id.localeCompare(b.id))
}

function fileUri(filePath: string): string {
  return pathToFileURL(path.resolve(filePath)).href
}

export function matchLspLanguage(extensionToLanguage: Record<string, string>, file: string): string | null {
  const lower = file.toLowerCase()
  const entries = Object.entries(extensionToLanguage).sort((a, b) => b[0].length - a[0].length)
  for (const [rawExt, language] of entries) {
    const ext = rawExt.toLowerCase().startsWith('.') ? rawExt.toLowerCase() : `.${rawExt.toLowerCase()}`
    if (lower.endsWith(ext)) return language
  }
  return null
}

class LspProcess {
  private child: ChildProcessWithoutNullStreams | null = null
  private nextId = 1
  private pending = new Map<number | string, Pending>()
  private frameDecoder = new LspFrameDecoder()
  private documents = new Map<string, { content: string; version: number; languageId: string }>()
  private startPromise: Promise<void> | null = null
  private stopping = false
  private restartAttempts = 0
  private restartTimer: ReturnType<typeof setTimeout> | null = null
  private stableTimer: ReturnType<typeof setTimeout> | null = null
  readonly diagnostics = new Map<string, any[]>()
  initialized = false
  error: string | undefined

  constructor(readonly config: PluginLspConfig, readonly projectRoot: string) {}

  private substitute(value: string): string {
    return value.replace(/\$\{CLAUDE_PROJECT_DIR\}/g, this.projectRoot)
  }

  async start(): Promise<void> {
    if (this.initialized) return
    if (this.startPromise) return this.startPromise
    if (this.restartAttempts > this.config.maxRestarts) throw new Error(`LSP ${this.config.id} exceeded its restart limit`)
    this.startPromise = this.startInner()
    try {
      await this.startPromise
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err)
      const child = this.child
      this.child = null
      this.stopping = true
      try { child?.kill('SIGTERM') } catch {}
      throw err
    } finally {
      this.startPromise = null
    }
  }

  private async startInner(): Promise<void> {
    this.stopping = false
    this.frameDecoder.reset()
    if (this.config.transport === 'socket') {
      throw new Error(`LSP ${this.config.id} uses socket transport, which requires an external socket adapter and is not supported by the Kamin bridge host`)
    }
    if (this.child) {
      try { this.child.kill('SIGTERM') } catch {}
      this.child = null
    }
    const dataDir = path.join(os.homedir(), '.claude', 'plugins', 'data', this.config.pluginId.replace(/[^a-zA-Z0-9\-_]/g, '-'))
    fs.mkdirSync(dataDir, { recursive: true })
    let cwd = this.projectRoot
    if (this.config.workspaceFolder) {
      const requested = path.resolve(this.projectRoot, this.substitute(this.config.workspaceFolder))
      if (isInside(this.projectRoot, requested)) cwd = requested
    }
    const pluginBin = path.join(this.config.pluginRoot, 'bin')
    const env = {
      ...process.env,
      ...Object.fromEntries(Object.entries(this.config.env).map(([k, v]) => [k, this.substitute(v)])),
      CLAUDE_PLUGIN_ROOT: this.config.pluginRoot,
      CLAUDE_PLUGIN_DATA: dataDir,
      CLAUDE_PROJECT_DIR: this.projectRoot,
      PATH: [fs.existsSync(pluginBin) ? pluginBin : '', process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
    }
    this.child = spawn(this.substitute(this.config.command), this.config.args.map(arg => this.substitute(arg)), {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
      shell: process.platform === 'win32',
    })
    this.child.stdout.on('data', (chunk: Buffer) => this.onData(chunk))
    this.child.stderr.on('data', (chunk: Buffer) => { this.error = chunk.toString().trim().slice(-1000) || this.error })
    this.child.on('error', (err) => { this.error = err.message; this.failAll(err) })
    this.child.on('exit', (code, signal) => {
      this.initialized = false
      this.child = null
      this.documents.clear()
      this.failAll(new Error(`LSP ${this.config.id} exited (code=${code}, signal=${signal})`))
      if (!this.stopping && this.config.restartOnCrash && this.restartAttempts < this.config.maxRestarts) {
        this.restartAttempts++
        this.restartTimer = setTimeout(() => {
          this.restartTimer = null
          void this.start().catch(() => { /* surfaced through list()/next request */ })
        }, Math.min(1000 * 2 ** (this.restartAttempts - 1), 10_000))
      }
    })

    const rootUri = fileUri(this.projectRoot)
    await this.request('initialize', {
      processId: process.pid,
      rootUri,
      workspaceFolders: [{ uri: rootUri, name: path.basename(this.projectRoot) }],
      capabilities: {
        workspace: { workspaceFolders: true, configuration: true },
        textDocument: {
          synchronization: { didSave: true, dynamicRegistration: false },
          hover: { contentFormat: ['markdown', 'plaintext'] },
          definition: { linkSupport: true },
          references: {},
          publishDiagnostics: { relatedInformation: true },
        },
      },
      initializationOptions: this.config.initializationOptions,
    }, this.config.startupTimeout)
    this.notify('initialized', {})
    if (this.config.settings !== undefined) this.notify('workspace/didChangeConfiguration', { settings: this.config.settings })
    this.initialized = true
    this.error = undefined
    if (this.stableTimer) clearTimeout(this.stableTimer)
    this.stableTimer = setTimeout(() => { this.restartAttempts = 0; this.stableTimer = null }, 30_000)
  }

  private send(message: JsonRpcMessage): void {
    if (!this.child) throw new Error(`LSP ${this.config.id} is not running`)
    const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }), 'utf-8')
    this.child.stdin.write(Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`), body]))
  }

  notify(method: string, params: unknown): void { this.send({ method, params }) }

  request(method: string, params: unknown, timeoutMs = 15_000): Promise<any> {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        reject(new Error(`LSP ${this.config.id} timed out waiting for ${method}`))
      }, timeoutMs)
      this.pending.set(id, { resolve, reject, timer })
      try { this.send({ id, method, params }) } catch (err) {
        clearTimeout(timer)
        this.pending.delete(id)
        reject(err instanceof Error ? err : new Error(String(err)))
      }
    })
  }

  private onData(chunk: Buffer): void {
    try {
      for (const message of this.frameDecoder.push(chunk)) this.onMessage(message)
    } catch (err) {
      const protocolError = new Error(`LSP ${this.config.id} protocol error: ${err instanceof Error ? err.message : String(err)}`)
      this.error = protocolError.message
      this.initialized = false
      this.stopping = true
      this.frameDecoder.reset()
      const child = this.child
      this.child = null
      this.failAll(protocolError)
      try { child?.kill('SIGTERM') } catch {}
    }
  }

  private onMessage(message: JsonRpcMessage): void {
    if (message.id !== undefined && !message.method) {
      const pending = this.pending.get(message.id)
      if (!pending) return
      clearTimeout(pending.timer)
      this.pending.delete(message.id)
      if (message.error) pending.reject(new Error(String(message.error.message ?? JSON.stringify(message.error))))
      else pending.resolve(message.result)
      return
    }
    if (message.method === 'textDocument/publishDiagnostics') {
      const uri = String(message.params?.uri ?? '')
      if (uri) this.diagnostics.set(uri, Array.isArray(message.params?.diagnostics) ? message.params.diagnostics : [])
      return
    }
    if (message.id !== undefined && message.method) {
      let result: unknown = null
      if (message.method === 'workspace/configuration') {
        const items = Array.isArray(message.params?.items) ? message.params.items : []
        result = items.map(() => this.config.settings ?? null)
      } else if (message.method === 'workspace/workspaceFolders') {
        const uri = fileUri(this.projectRoot)
        result = [{ uri, name: path.basename(this.projectRoot) }]
      } else if (message.method === 'workspace/applyEdit') {
        result = { applied: false, failureReason: 'Plugin LSP edits are not applied automatically by the bridge' }
      }
      try { this.send({ id: message.id, result }) } catch { /* process closed */ }
    }
  }

  async syncDocument(file: string, languageId: string): Promise<string> {
    const uri = fileUri(file)
    const content = await fs.promises.readFile(file, 'utf-8')
    const previous = this.documents.get(uri)
    if (!previous) {
      this.documents.set(uri, { content, version: 1, languageId })
      this.notify('textDocument/didOpen', { textDocument: { uri, languageId, version: 1, text: content } })
    } else if (previous.content !== content) {
      const version = previous.version + 1
      this.documents.set(uri, { content, version, languageId })
      this.notify('textDocument/didChange', { textDocument: { uri, version }, contentChanges: [{ text: content }] })
    }
    return uri
  }

  private failAll(err: Error): void {
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(err) }
    this.pending.clear()
  }

  async stop(): Promise<void> {
    this.stopping = true
    if (this.restartTimer) { clearTimeout(this.restartTimer); this.restartTimer = null }
    if (this.stableTimer) { clearTimeout(this.stableTimer); this.stableTimer = null }
    const child = this.child
    if (!child) return
    try { if (this.initialized) await this.request('shutdown', null, this.config.shutdownTimeout) } catch { /* force below */ }
    try { this.notify('exit', null) } catch {}
    this.initialized = false
    this.child = null
    try { child.kill('SIGTERM') } catch {}
    this.failAll(new Error(`LSP ${this.config.id} stopped`))
  }
}

class PluginLspManager {
  private definitions: PluginLspConfig[] = []
  private instances = new Map<string, LspProcess>()
  private instanceOwners = new Map<string, Set<string>>()
  private tabInstances = new Map<string, Set<string>>()
  private activeTabs = new Set<string>()

  async refresh(): Promise<void> { this.definitions = await discoverPluginLspConfigs() }

  activateTab(tabId: string): void { this.activeTabs.add(tabId) }

  async stopTab(tabId: string): Promise<void> {
    this.activeTabs.delete(tabId)
    const keys = this.tabInstances.get(tabId)
    this.tabInstances.delete(tabId)
    if (!keys) return
    const stopping: Promise<void>[] = []
    for (const key of keys) {
      const owners = this.instanceOwners.get(key)
      owners?.delete(tabId)
      if (owners && owners.size > 0) continue
      this.instanceOwners.delete(key)
      const instance = this.instances.get(key)
      this.instances.delete(key)
      if (instance) stopping.push(instance.stop())
    }
    await Promise.all(stopping)
  }

  private ownInstance(tabId: string, instanceKey: string): boolean {
    if (!this.activeTabs.has(tabId)) return false
    if (!this.instanceOwners.has(instanceKey)) this.instanceOwners.set(instanceKey, new Set())
    this.instanceOwners.get(instanceKey)!.add(tabId)
    if (!this.tabInstances.has(tabId)) this.tabInstances.set(tabId, new Set())
    this.tabInstances.get(tabId)!.add(instanceKey)
    return true
  }

  private async matching(file: string, tabId?: string | null): Promise<Array<{ process: LspProcess; languageId: string }>> {
    if (this.definitions.length === 0) await this.refresh()
    const projectRoot = resolveProject(tabId, file) ?? path.dirname(file)
    for (const config of this.definitions) {
      const languageId = matchLspLanguage(config.extensionToLanguage, file)
      if (!languageId) continue
      const instanceKey = `${config.id}::${path.resolve(projectRoot)}`
      let process = this.instances.get(instanceKey)
      if (!process) {
        process = new LspProcess(config, projectRoot)
        this.instances.set(instanceKey, process)
      }
      try {
        await process.start()
        await process.syncDocument(file, languageId)
        if (tabId && !this.ownInstance(tabId, instanceKey)) {
          // The tab closed while the server was starting. If nobody else owns
          // this process, tear it down instead of leaving an orphan child.
          if ((this.instanceOwners.get(instanceKey)?.size ?? 0) === 0) {
            this.instances.delete(instanceKey)
            await process.stop()
          }
          continue
        }
        // Claude Code assigns an extension to the first valid LSP server;
        // later matching servers remain idle.
        return [{ process, languageId }]
      } catch { /* invalid server does not claim the extension; try the next */ }
    }
    return []
  }

  async request(file: string, method: string, params: Record<string, unknown>, tabId?: string | null): Promise<any[]> {
    const matches = await this.matching(file, tabId)
    const uri = fileUri(file)
    return Promise.all(matches.map(({ process }) => process.request(method, { textDocument: { uri }, ...params })))
  }

  async getDiagnostics(file: string, tabId?: string | null): Promise<any[]> {
    const matches = await this.matching(file, tabId)
    const uri = fileUri(file)
    await new Promise(resolve => setTimeout(resolve, 150))
    return matches.flatMap(({ process }) => process.config.diagnostics ? (process.diagnostics.get(uri) ?? []) : [])
  }

  async list(): Promise<PluginLspSummary[]> {
    await this.refresh()
    return this.definitions.map(config => {
      const running = [...this.instances.values()].find(instance => instance.config.id === config.id)
      return {
        id: config.id,
        pluginId: config.pluginId,
        serverKey: config.serverKey,
        extensions: Object.keys(config.extensionToLanguage),
        initialized: running?.initialized ?? false,
        error: config.transport === 'socket'
          ? 'Socket transport requires an external adapter and is not supported by the Kamin bridge host'
          : running?.error,
      }
    })
  }

  async restart(): Promise<void> {
    const running = [...this.instances.values()]
    this.instances.clear()
    this.instanceOwners.clear()
    this.tabInstances.clear()
    await Promise.all(running.map(instance => instance.stop()))
    await this.refresh()
  }
}

export const pluginLspManager = new PluginLspManager()
