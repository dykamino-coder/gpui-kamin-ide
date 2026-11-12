// Repeatable latency bench of the client-side MCP tools (Read/Write/Edit/Glob/
// Grep/Bash/PowerShell) against the bare primitive each one wraps. Run with
// `node bench/run-tool-latency.mjs [--out <dir>] [--runs N] [--tree <dir>]`.
// It measures only the extension host part of a call (handler entry → result);
// the Bridge transport is measured separately by `bench/mcp-roundtrip.mjs`.
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { performance } from 'node:perf_hooks'
import { readFile, writeFile, editFile, globSearch, grepSearch } from '../src/main/mcp/tools/file-system'
import { bashExecute, powerShellExecute } from '../src/main/mcp/tools/shell'
import { findBashExecutable, findPowerShellExecutable } from '../src/main/utils/command-runtime'
import { listEnabledPluginBinDirs } from '../src/main/plugin-helpers'
import { resetRipgrepPathForTests } from '../src/main/mcp/tools/ripgrep-path'
import { prewarmShellTools } from '../src/main/mcp/tools/shell-prewarm'

type Row = { tool: string; op: string; runs: number; p50: number; p95: number; first: number; note?: string }
const rows: Row[] = []

function arg(name: string, fallback: string): string {
  const i = process.argv.indexOf(name)
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1]! : fallback
}
const RUNS = Number(arg('--runs', '15'))
const OUT = arg('--out', path.join(os.tmpdir(), 'kamin-tool-bench'))
const TREE = path.resolve(arg('--tree', path.join(__dirname, '..', 'src')))
const BIG_TREE = path.resolve(arg('--big-tree', path.join(__dirname, '..', '..', '..', '..')))
const RG_DIR = process.env.BENCH_RG_DIR

function pct(values: number[], p: number): number {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]!
}

async function measure(tool: string, op: string, fn: () => Promise<unknown>, runs = RUNS, note?: string): Promise<unknown> {
  let last: unknown
  const times: number[] = []
  for (let i = 0; i < runs; i++) {
    const t = performance.now()
    last = await fn()
    times.push(performance.now() - t)
  }
  rows.push({ tool, op, runs, first: times[0]!, p50: pct(times.slice(1).length ? times.slice(1) : times, 50), p95: pct(times, 95), note })
  return last
}

function run(bin: string, args: string[], env: NodeJS.ProcessEnv = process.env): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, args, { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, env })
    const out: Buffer[] = []
    child.stdout.on('data', (c: Buffer) => out.push(c))
    child.on('error', reject)
    child.on('close', () => resolve(Buffer.concat(out).toString('utf8')))
  })
}

function text(result: unknown): string {
  const content = (result as { content?: Array<{ text?: string }> })?.content
  return content?.map((c) => c.text ?? '').join('') ?? String(result)
}

function fixtures(dir: string): Record<string, string> {
  fs.mkdirSync(dir, { recursive: true })
  const line = (i: number): string => `export const value${i} = computeSomething(${i}, 'текст', "quoted") // ${'x'.repeat(40)}\n`
  const small = Array.from({ length: 60 }, (_, i) => line(i)).join('')
  const medium = Array.from({ length: 2500 }, (_, i) => line(i)).join('')
  const large = Array.from({ length: 50_000 }, (_, i) => line(i)).join('')
  const files: Record<string, string> = {
    small: path.join(dir, 'small.ts'),
    medium: path.join(dir, 'medium.ts'),
    large: path.join(dir, 'large.ts'),
    crlf: path.join(dir, 'crlf.ts'),
    edit: path.join(dir, 'edit.ts'),
    editMedium: path.join(dir, 'edit-medium.ts'),
    write: path.join(dir, 'write.ts'),
  }
  fs.writeFileSync(files.small!, small)
  fs.writeFileSync(files.medium!, medium)
  fs.writeFileSync(files.large!, large)
  fs.writeFileSync(files.crlf!, small.replaceAll('\n', '\r\n'))
  fs.writeFileSync(files.edit!, small)
  fs.writeFileSync(files.editMedium!, medium)
  return files
}

async function main(): Promise<void> {
  fs.mkdirSync(OUT, { recursive: true })
  const files = fixtures(path.join(OUT, 'fixtures'))
  const ctx = { cwd: TREE }

  // ── Read ──
  await measure('Read', 'small 4 KB (first = cold cache)', () => readFile({ file_path: files.small }, ctx))
  await measure('Read', 'medium 200 KB', () => readFile({ file_path: files.medium }, ctx))
  await measure('Read', 'large 4 MB, offset 25000 limit 50', () => readFile({ file_path: files.large, offset: 25_000, limit: 50 }, ctx), 5)
  await measure('baseline', 'fs.readFile 200 KB + split', async () => (await fs.promises.readFile(files.medium!, 'utf8')).split('\n'))

  // ── Write / Edit ──
  await measure('Write', 'small 4 KB', () => writeFile({ file_path: files.write, content: fs.readFileSync(files.small!, 'utf8') }, ctx))
  await measure('baseline', 'fs.writeFile 4 KB', () => fs.promises.writeFile(files.write!, fs.readFileSync(files.small!, 'utf8')))
  let flip = false
  await measure('Edit', 'small file, unique line', () => {
    flip = !flip
    return flip
      ? editFile({ file_path: files.edit, old_string: 'value7 =', new_string: 'value7b =' }, ctx)
      : editFile({ file_path: files.edit, old_string: 'value7b =', new_string: 'value7 =' }, ctx)
  })
  flip = false
  await measure('Edit', 'medium 200 KB file', () => {
    flip = !flip
    return flip
      ? editFile({ file_path: files.editMedium, old_string: 'value1777 =', new_string: 'value1777b =' }, ctx)
      : editFile({ file_path: files.editMedium, old_string: 'value1777b =', new_string: 'value1777 =' }, ctx)
  })

  // ── Glob ──
  await measure('Glob', `**/*.ts in extension src`, () => globSearch({ pattern: '**/*.ts', path: TREE }, ctx), 7)
  await measure('Glob', `**/*.rs in repo`, () => globSearch({ pattern: '**/*.rs', path: BIG_TREE }, ctx), 5)

  // ── Grep ──
  const originalPath = process.env.PATH ?? ''
  const variants: Array<[string, string]> = [['PATH as-is', originalPath]]
  if (RG_DIR) variants.push(['rg on PATH', `${RG_DIR}${path.delimiter}${originalPath}`])
  for (const [label, envPath] of variants) {
    process.env.PATH = envPath
    resetRipgrepPathForTests()
    await measure('Grep', `files_with_matches, src (${label})`, () => grepSearch({ pattern: 'export function', path: TREE }, ctx), 7)
    await measure('Grep', `content -n head 50, src (${label})`, () => grepSearch({ pattern: 'export function', path: TREE, output_mode: 'content', head_limit: 50 }, ctx), 7)
    await measure('Grep', `files_with_matches, repo (${label})`, () => grepSearch({ pattern: 'fn layout_block', path: BIG_TREE }, ctx), label === 'PATH as-is' ? 1 : 3)
  }
  process.env.PATH = originalPath
  if (RG_DIR) {
    const rg = path.join(RG_DIR, 'rg.exe')
    await measure('baseline', 'rg -l direct, src', () => run(rg, ['--files-with-matches', '--', 'export function', TREE]), 7)
    await measure('baseline', 'rg -l direct, repo', () => run(rg, ['--files-with-matches', '--', 'fn layout_block', BIG_TREE]), 3)
    await measure('baseline', 'rg --files *.rs, repo', () => run(rg, ['--files', '--glob', '*.rs', BIG_TREE]), 5)
  }

  // ── Bash: breakdown then tool ──
  const bash = await findBashExecutable()
  await measure('Bash parts', 'findBashExecutable (where git)', () => findBashExecutable(), 5)
  await measure('Bash parts', 'listEnabledPluginBinDirs (per call)', () => listEnabledPluginBinDirs(), 10)
  await measure('Bash parts', 'snapshot capture bash -ilc', () => run(bash, ['-ilc', 'declare -p; declare -fp; alias -p']), 3)
  if (process.env.BENCH_PREWARM === '1') await measure('Bash parts', 'prewarm at session connect (background)', () => prewarmShellTools(), 1)
  await measure('baseline', 'bash -c true (bare spawn)', () => run(bash, ['-c', 'true']))
  await measure('Bash', 'echo hi (first = cold: lookup + snapshot)', () => bashExecute({ command: 'echo hi' }, ctx))
  await measure('Bash', 'git status --short', () => bashExecute({ command: 'git status --short | head -5' }, ctx), 7)
  const snap = path.join(os.tmpdir(), `ocb-shell-snapshot-${process.pid}.sh`)
  if (fs.existsSync(snap)) {
    const size = fs.statSync(snap).size
    await measure('Bash parts', `bash -c "source snapshot (${Math.round(size / 1024)} KB); true"`, () => run(bash, ['-c', `source '${snap}' 2>/dev/null || true\ntrue`]))
  }
  const jobrun = process.env.KAMIN_JOBRUN_EXE
  if (jobrun) await measure('Bash parts', 'kamin-jobrun bash -c true', () => run(jobrun, [bash, '-c', 'true']))

  // ── PowerShell ──
  const pwsh = await findPowerShellExecutable()
  await measure('baseline', 'pwsh -NoProfile -Command 1 (bare)', () => run(pwsh, ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', '1']), 7)
  await measure('baseline', 'pwsh -Command 1 (with profile)', () => run(pwsh, ['-NoLogo', '-NonInteractive', '-Command', '1']), 7)
  await measure('PowerShell', 'Write-Output hi (first = cold)', () => powerShellExecute({ command: 'Write-Output hi' }, ctx), 7)

  // ── Quality probes (recorded, not timed) ──
  const probes: Record<string, string> = {}
  probes.bashAliasFromSnapshot = text(await bashExecute({ command: 'type -t ll; echo PATH0=${PATH%%:*}; shopt expand_aliases' }, ctx)).slice(0, 300)
  probes.crlfRead = JSON.stringify(text(await readFile({ file_path: files.crlf, limit: 1 }, ctx)))
  fs.writeFileSync(path.join(OUT, 'fixtures', 'dup.ts'), 'a = 1\na = 1\n')
  probes.editDuplicate = text(await editFile({ file_path: path.join(OUT, 'fixtures', 'dup.ts'), old_string: 'a = 1', new_string: 'a = 2' }, ctx))
  probes.editDuplicateFile = JSON.stringify(fs.readFileSync(path.join(OUT, 'fixtures', 'dup.ts'), 'utf8'))
  const utf16 = path.join(OUT, 'fixtures', 'utf16.txt')
  fs.writeFileSync(utf16, Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('héllo\r\nмир\r\n', 'utf16le')]))
  probes.utf16Read = JSON.stringify(text(await readFile({ file_path: utf16 }, ctx)).slice(0, 120))
  const bin = path.join(OUT, 'fixtures', 'blob.bin')
  fs.writeFileSync(bin, Buffer.from([0, 1, 2, 3, 0xff, 0xfe, 0, 0, 65, 66]))
  probes.binaryRead = JSON.stringify(text(await readFile({ file_path: bin }, ctx)).slice(0, 120))
  probes.missingReadIsError = JSON.stringify(await readFile({ file_path: path.join(OUT, 'nope.txt') }, ctx))
  probes.bashExit3 = JSON.stringify(await bashExecute({ command: 'echo out; echo err >&2; exit 3' }, ctx))
  probes.bashUtf8Split = JSON.stringify(text(await bashExecute({ command: `printf '%.0sж' {1..40000} | wc -c` }, ctx)))

  const md = [
    `| tool | op | runs | first ms | p50 ms | p95 ms |`,
    `|---|---|---:|---:|---:|---:|`,
    ...rows.map((r) => `| ${r.tool} | ${r.op} | ${r.runs} | ${r.first.toFixed(1)} | ${r.p50.toFixed(1)} | ${r.p95.toFixed(1)} |`),
  ].join('\n')
  fs.writeFileSync(path.join(OUT, 'tool-latency.json'), JSON.stringify({ host: os.hostname(), platform: process.platform, rows, probes }, null, 2))
  fs.writeFileSync(path.join(OUT, 'tool-latency.md'), md + '\n')
  console.log(md)
  console.log(JSON.stringify(probes, null, 2))
}

main().catch((err) => {
  console.error(err)
  process.exitCode = 1
})
