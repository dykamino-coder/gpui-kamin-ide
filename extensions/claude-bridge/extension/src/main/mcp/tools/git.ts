import { spawn } from 'node:child_process'
import path from 'node:path'
import fs from 'node:fs'
import crypto from 'node:crypto'

export type McpResult = { content: Array<{ type: string; text: string }> }

function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

function errorResult(message: string): McpResult {
  return { content: [{ type: 'text', text: `Error: ${message}` }] }
}

/** Async git spawn — main-process event loop must stay responsive while
 *  worktree operations run (they can take seconds). */
function runGitLocal(args: string[], opts: { cwd?: string; timeoutMs?: number } = {}): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const proc = spawn('git', args, { cwd: opts.cwd, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })
    let stdout = '', stderr = ''
    const timer = opts.timeoutMs ? setTimeout(() => { try { proc.kill('SIGKILL') } catch {} reject(new Error(`git timeout after ${opts.timeoutMs}ms`)) }, opts.timeoutMs) : null
    proc.stdout?.on('data', d => { stdout += d.toString('utf-8') })
    proc.stderr?.on('data', d => { stderr += d.toString('utf-8') })
    proc.on('error', err => { if (timer) clearTimeout(timer); reject(err) })
    proc.on('close', code => {
      if (timer) clearTimeout(timer)
      if (code === 0) resolve({ stdout, stderr })
      else reject(new Error(`git exited with code ${code}: ${stderr.slice(0, 300)}`))
    })
  })
}

async function getGitRoot(cwd?: string): Promise<string | null> {
  try {
    const { stdout } = await runGitLocal(['rev-parse', '--show-toplevel'], { cwd: cwd || process.cwd(), timeoutMs: 5000 })
    return stdout.trim()
  } catch {
    return null
  }
}

export async function exitWorktree(input: Record<string, unknown>): Promise<McpResult> {
  const worktreePath = input.path as string
  if (!worktreePath) return errorResult('path is required')

  // Validate worktree path: extract basename and check for injection
  const worktreeName = path.basename(worktreePath)
  if (!/^[a-zA-Z0-9_-]+$/.test(worktreeName)) {
    return errorResult('Invalid worktree name — only alphanumeric characters, hyphens, and underscores are allowed')
  }

  try {
    const gitRoot = await getGitRoot()
    if (!gitRoot) return errorResult('Not inside a git repository.')

    if (!fs.existsSync(worktreePath)) {
      return errorResult(`Worktree path does not exist: ${worktreePath}`)
    }

    await runGitLocal(['worktree', 'remove', worktreePath, '--force'], { cwd: gitRoot, timeoutMs: 30000 })

    return textResult(`Removed worktree: ${worktreePath}`)
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    return errorResult(`Failed to remove worktree: ${msg}`)
  }
}

export async function enterWorktree(input: Record<string, unknown>): Promise<McpResult> {
  const name = (input.name as string) || `worktree-${crypto.randomBytes(4).toString('hex')}`

  // Validate worktree name to prevent command injection
  if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
    return errorResult('Invalid worktree name — only alphanumeric characters, hyphens, and underscores are allowed')
  }

  try {
    const gitRoot = await getGitRoot()
    if (!gitRoot) {
      return errorResult('Not inside a git repository. Cannot create worktree.')
    }

    // Create .claude/worktrees directory if it doesn't exist
    const worktreesDir = path.join(gitRoot, '.claude', 'worktrees')
    fs.mkdirSync(worktreesDir, { recursive: true })

    const worktreePath = path.join(worktreesDir, name)

    // Check if worktree already exists
    if (fs.existsSync(worktreePath)) {
      return errorResult(`Worktree already exists at ${worktreePath}`)
    }

    // Create a new branch name based on the worktree name
    const branchName = `worktree/${name}`

    await runGitLocal(['worktree', 'add', '-b', branchName, worktreePath, 'HEAD'], { cwd: gitRoot, timeoutMs: 30000 })

    // Verify the worktree was created
    if (!fs.existsSync(worktreePath)) {
      return errorResult('Worktree creation command succeeded but directory was not found')
    }

    return textResult(
      `Created worktree at: ${worktreePath}\n` +
      `Branch: ${branchName}\n` +
      `Based on: HEAD`
    )
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    return errorResult(`Failed to create worktree: ${msg}`)
  }
}
