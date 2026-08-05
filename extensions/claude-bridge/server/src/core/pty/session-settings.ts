// ============================================================================
// PTY Session — settings dir / CLAUDE.md / sync data plumbing.
// All filesystem-touching helpers extracted from `session-core.ts` so the
// core file stays under the size ceiling and these helpers can be tested
// or replaced (e.g. mocked) independently.
// ============================================================================

import fs from 'fs'
import path from 'path'
import os from 'os'
import { config, DEFAULT_SESSION_MODEL } from '../config'
import { debugLog, warnLog } from '../logging'
import { getUserSyncDir, getProjectSyncDir } from '../sync/routes'
import { mergeAllHooks, rewriteHooksForCli } from '../hooks'
import { bridgeStatusHookMatchers } from '../hooks/bridge-status-hooks'
import type { HookMatcher } from '../hooks/types'
import { DISALLOWED_BUILTIN_TOOLS } from './session-env'
import { replaceSkillsOverlay } from './skills-snapshot'

export const SESSIONS_BASE = path.join(os.homedir(), '.claude', 'bridge-sessions')

/**
 * Convert a user path like "C:\Users\alice\Projects\my-app" into a safe dir name suffix.
 * Result: "C_Users_alice_Projects_my-app" (truncated to 80 chars max).
 */
export function sanitizeDirName(userPath: string): string {
  return userPath
    .replace(/^[A-Za-z]:/, (m) => m.charAt(0))   // C: → C
    .replace(/[\\/:*?"<>|]+/g, '_')      // unsafe chars → underscore
    .replace(/_+/g, '_')                 // collapse multiple underscores
    .replace(/^_|_$/g, '')               // trim leading/trailing underscores
    .slice(0, 80)
}

/**
 * Ensure trust flags are set in ~/.claude.json for the settingsDir.
 * Without this, Claude CLI shows "Do you trust the files in this folder?" on first run.
 */
export function ensureTrustFlags(settingsDir: string): void {
  const claudeJsonPath = path.join(os.homedir(), '.claude.json')

  let claudeJson: Record<string, unknown> = {}
  try {
    claudeJson = JSON.parse(fs.readFileSync(claudeJsonPath, 'utf8'))
  } catch {
    // File doesn't exist or invalid JSON — start fresh
  }

  let changed = false

  // Global onboarding flags
  if (!claudeJson.hasCompletedOnboarding) { claudeJson.hasCompletedOnboarding = true; changed = true }
  if (!claudeJson.hasTrustDialogAccepted) { claudeJson.hasTrustDialogAccepted = true; changed = true }
  if (!claudeJson.hasCompletedProjectOnboarding) { claudeJson.hasCompletedProjectOnboarding = true; changed = true }

  // Suppress effort level selection dialog
  if (!claudeJson.effortCalloutDismissed) { claudeJson.effortCalloutDismissed = true; changed = true }
  if (!claudeJson.effortCalloutV2Dismissed) { claudeJson.effortCalloutV2Dismissed = true; changed = true }
  if (!claudeJson.numStartups || (claudeJson.numStartups as number) <= 1) { claudeJson.numStartups = 5; changed = true }

  // Per-project trust for the settingsDir
  if (!claudeJson.projects || typeof claudeJson.projects !== 'object') {
    claudeJson.projects = {}
  }
  const projects = claudeJson.projects as Record<string, Record<string, unknown>>
  const normalizedDir = path.resolve(settingsDir)

  if (!projects[normalizedDir]) {
    projects[normalizedDir] = {}
  }
  if (!projects[normalizedDir]!.hasTrustDialogAccepted) {
    projects[normalizedDir]!.hasTrustDialogAccepted = true
    projects[normalizedDir]!.hasCompletedProjectOnboarding = true
    changed = true
  }
  // Pre-approve project MCP servers
  if (!projects[normalizedDir]!.mcpServerApprovals) {
    projects[normalizedDir]!.mcpServerApprovals = {}
  }
  const approvals = projects[normalizedDir]!.mcpServerApprovals as Record<string, string>
  if (approvals['user-tools'] !== 'approved') {
    approvals['user-tools'] = 'approved'
    changed = true
  }

  // Also trust the base sessions directory
  const normalizedBase = path.resolve(SESSIONS_BASE)
  if (!projects[normalizedBase]) {
    projects[normalizedBase] = {}
  }
  if (!projects[normalizedBase]!.hasTrustDialogAccepted) {
    projects[normalizedBase]!.hasTrustDialogAccepted = true
    projects[normalizedBase]!.hasCompletedProjectOnboarding = true
    changed = true
  }

  if (changed) {
    fs.writeFileSync(claudeJsonPath, JSON.stringify(claudeJson, null, 2))
    debugLog('Trust flags set', { settingsDir: normalizedDir })
  }
}

export function writeSessionSettings(
  sessionId: string,
  settingsDir: string,
  mcpToken: string,
  sessionModel?: string,
  userCwd?: string,
  bearerHash?: string,
): void {
  const port = config.port
  const mcpUrl = `http://127.0.0.1:${port}/mcp/${sessionId}`

  // ─── 1. MCP server config → .mcp.json (project root) ───
  const mcpConfig = {
    mcpServers: {
      'user-tools': {
        type: 'http',
        url: mcpUrl,
        headers: { Authorization: `Bearer ${mcpToken}` },
      },
    },
  }
  fs.writeFileSync(path.join(settingsDir, '.mcp.json'), JSON.stringify(mcpConfig, null, 2))

  // ─── 2. Project settings → .claude/settings.json ───
  const claudeDir = path.join(settingsDir, '.claude')
  fs.mkdirSync(claudeDir, { recursive: true })

  // Merge hooks (user-global / synced / project / plugins) and rewrite each
  // command-type hook into an http proxy pointing at /api/hooks/<sid>/...
  // The synced copies are the ones that actually exist in the container —
  // the client uploads its ~/.claude/settings.json (and the project's) to
  // the per-token sync dir; container-local paths are empty by design.
  const syncedSettingsFiles: string[] = []
  if (bearerHash) {
    syncedSettingsFiles.push(path.join(getUserSyncDir(bearerHash), 'settings.json'))
    if (userCwd) {
      syncedSettingsFiles.push(path.join(getProjectSyncDir(bearerHash, userCwd), 'settings.json'))
    }
  }
  const merged = mergeAllHooks(userCwd, syncedSettingsFiles)
  // Inject bridge-owned status hooks (deterministic session state — see
  // hooks/bridge-status-hooks.ts). Appended AFTER the user/project/plugin merge
  // so both coexist: the CLI runs every matcher for an event, and these no-op
  // hooks' webhook firing is what drives session:activity.
  for (const { event, matcher, source } of bridgeStatusHookMatchers()) {
    const bucket = (merged.hooks as Record<string, HookMatcher[]>)[event] ?? []
    bucket.push(matcher)
    ;(merged.hooks as Record<string, HookMatcher[]>)[event] = bucket
    merged.sourceByMatcher.set(matcher, source)
  }
  const rewrittenHooks = rewriteHooksForCli(
    sessionId,
    merged.hooks,
    (m) => merged.sourceByMatcher.get(m) ?? { kind: 'user' },
    mcpToken,
  )

  const settings: Record<string, unknown> = {
    permissions: { deny: DISALLOWED_BUILTIN_TOOLS },
  }
  if (Object.keys(rewrittenHooks).length > 0) {
    settings.hooks = rewrittenHooks
  }
  fs.writeFileSync(path.join(claudeDir, 'settings.json'), JSON.stringify(settings, null, 2))

  // ─── 3. Local settings → .claude/settings.local.json ───
  // fallbackModel (CLI 2.1.166+): an ordered chain the CLI tries when the
  // primary is unavailable (overloaded / not-found), so a 529 on Opus doesn't
  // dead-end the session. It does NOT merge across scopes — this file (highest
  // precedence) supplies the whole chain. Exclude whatever the session's own
  // model resolves to so the chain never repeats the primary.
  const primaryBase = (sessionModel || DEFAULT_SESSION_MODEL).replace(/\[1m\]$/, '')
  const fallbackModel = ['claude-opus-5', 'claude-sonnet-5', 'claude-haiku-4-5']
    .filter((m) => m !== primaryBase)
  const localSettings = {
    model: sessionModel || DEFAULT_SESSION_MODEL,
    fallbackModel,
    skipDangerousModePermissionPrompt: true,
    env: {
      CLAUDE_AUTOCOMPACT_PCT_OVERRIDE: '92',
      CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: '1',
      // Default CLI value is ~27.8h; pin explicitly so future CLI updates don't shrink it.
      MCP_TOOL_TIMEOUT: '100000000',
    },
  }
  fs.writeFileSync(
    path.join(claudeDir, 'settings.local.json'),
    JSON.stringify(localSettings, null, 2),
  )

  ensureTrustFlags(settingsDir)

  debugLog('Session settings written', {
    sessionId,
    mcpUrl,
    settings: path.join(claudeDir, 'settings.json'),
  })
}

/** Tenant-agnostic preamble describing Open Claude Bridge's split architecture
 *  (Linux container holds CLI; user's machine holds files + tools). Used both
 *  as the per-session CLAUDE.md header (with cwd appended) and as the global
 *  ~/.claude/CLAUDE.md default (cwd omitted) — global is shared across all
 *  tokens, so it MUST stay tenant-agnostic. */
export const BRIDGE_DEFAULT_CLAUDE_MD = `# Open Claude Bridge — Remote Session

You are running inside Open Claude Bridge: a Linux Docker container that hosts
the Claude Code CLI process, while files, shell, and the user's editor live on
their local machine (Windows / macOS / Linux).

## Important

- All file operations (Read, Write, Edit, Glob, Grep) execute on the USER's local machine via MCP tools, not on this server.
- Bash commands also execute on the user's machine.
- The current server-side directory (\`/home/bridge/.claude/bridge-sessions/...\`) is NOT the user's filesystem — do not attempt to access files there directly.
- Always use absolute paths under the user's working directory as the project root.
- When the user asks about "current directory" or "project", they mean their local cwd, not this container.
- Network access from this container goes through the bridge's outbound proxy if configured; the user's local machine has its own network.

## Editor context (auto-attached messages)

When the user has the FilePanel editor open and the **attach file** switch is ON,
every message they send carries an extra fenced block at the end:

    ---
    <!-- editor-context: auto-attached by Open Claude Bridge -->
    The user is currently looking at the file below in the file viewer.
    \`\`\`json editor-context
    {
      "openedFile": "<absolute path>",
      "selectedRange": { "line": N }                    // single-line (caret only)
                      | { "startLine": A, "endLine": B }, // multi-line selection
      "textInSelectedRange": "<selected text>" | null
    }
    \`\`\`

Treat that block as the **implicit subject** of the user's question:
- "this file", "this function", "fix this", "why is it slow" → refers to \`openedFile\`.
- "this line", "the selection", "what does this do" → refers to \`selectedRange\` /
  \`textInSelectedRange\` inside that file.
- The user's natural-language message is the actual instruction; the JSON block
  is purely contextual metadata. Don't echo it back verbatim.
- If the JSON block is absent, the user is asking a general question — do NOT
  assume any particular file is in focus.
`

export function writeSessionClaudeMd(settingsDir: string, userCwd: string): void {
  const claudeMd = `${BRIDGE_DEFAULT_CLAUDE_MD}
## Working Directory (this session)

\`\`\`
${userCwd}
\`\`\`

- Treat \`${userCwd}\` as your primary working directory for all operations.
- All paths in this conversation are relative to \`${userCwd}\` unless explicitly absolute.
`
  fs.writeFileSync(path.join(settingsDir, 'CLAUDE.md'), claudeMd)
  debugLog('Session CLAUDE.md written', { userCwd })
}

/** Initialise the shared ~/.claude/CLAUDE.md to the bridge default. Called once
 *  at server boot so the file has a sane tenant-agnostic baseline before any
 *  PTY session starts. Per-session CLAUDE.md (settingsDir/CLAUDE.md) takes
 *  precedence for the actual conversation; this file is just the fallback. */
export function ensureGlobalBridgeClaudeMd(): void {
  try {
    const globalClaudeMd = path.join(os.homedir(), '.claude', 'CLAUDE.md')
    fs.mkdirSync(path.dirname(globalClaudeMd), { recursive: true })
    const existing = fs.existsSync(globalClaudeMd) ? fs.readFileSync(globalClaudeMd, 'utf-8') : ''
    if (existing !== BRIDGE_DEFAULT_CLAUDE_MD) {
      fs.writeFileSync(globalClaudeMd, BRIDGE_DEFAULT_CLAUDE_MD, 'utf-8')
      debugLog('Global ~/.claude/CLAUDE.md set to bridge default')
    }
  } catch (e) {
    debugLog('Failed to write global bridge CLAUDE.md', { error: String(e) })
  }
}

/**
 * Remove `# User Instructions (synced)` and/or `# Project Instructions (synced)`
 * blocks from a CLAUDE.md so we can re-append fresh copies idempotently.
 *
 * Each block runs from its `# … (synced)` heading until either the next
 * heading of the same form or the end of the file.
 */
export function stripSyncedBlocks(
  content: string,
  opts: { stripUser?: boolean; stripProject?: boolean } = { stripUser: true, stripProject: true },
): string {
  const headings: string[] = []
  if (opts.stripUser !== false) headings.push('# User Instructions (synced)')
  if (opts.stripProject !== false) headings.push('# Project Instructions (synced)')
  if (headings.length === 0) return content

  const lines = content.split('\n')
  const out: string[] = []
  let skipping = false
  for (const line of lines) {
    const trimmed = line.trim()
    const isOurHeading = headings.includes(trimmed)
    const isOtherHeading = !isOurHeading && /^# [^\s]/.test(trimmed) && !trimmed.endsWith('(synced)')
    if (isOurHeading) { skipping = true; continue }
    if (skipping && isOtherHeading) { skipping = false; out.push(line); continue }
    if (skipping) continue
    out.push(line)
  }
  return out.join('\n')
}

/** Recursively copy directory contents (creates target if needed). */
export function copyDirRecursive(src: string, dest: string): void {
  fs.mkdirSync(dest, { recursive: true })
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name)
    const destPath = path.join(dest, entry.name)
    if (entry.isDirectory()) copyDirRecursive(srcPath, destPath)
    else fs.copyFileSync(srcPath, destPath)
  }
}

/** Rebuild the exact effective skills tree for one live or new session.
 * User skills form the base and project skills override matching paths. */
export function refreshSessionSkills(
  settingsDir: string,
  tokenId: string,
  userCwd: string | undefined,
): void {
  const destination = path.join(settingsDir, '.claude', 'skills')
  const userSource = path.join(getUserSyncDir(tokenId), 'skills')
  const projectSource = userCwd
    ? path.join(getProjectSyncDir(tokenId, userCwd), 'skills')
    : null

  replaceSkillsOverlay(destination, userSource, projectSource)
  debugLog('[sync] Rebuilt effective session skills', {
    tokenId,
    project: userCwd,
    target: destination,
  })
}

/**
 * Apply per-token synced data (user-level and project-level) to a session's CWD.
 * - User skills → exact session-local user + project overlay
 * - User agents/commands → symlinked into .claude/ (fallback: copy)
 * - User CLAUDE.md → appended to session CLAUDE.md (idempotent strip+append)
 * - Project files → copied into settingsDir (except .mcp.json and settings.json)
 */
export function applySyncData(settingsDir: string, tokenId: string, userCwd: string | undefined): void {
  try {
    const hash = tokenId
    const userDir = getUserSyncDir(hash)
    const claudeDir = path.join(settingsDir, '.claude')

    // Initial spawn and live reload must use identical overlay semantics. A
    // local copy also prevents a project overlay from following a user-skill
    // symlink and mutating the shared user snapshot.
    refreshSessionSkills(settingsDir, hash, userCwd)

    // ─── User-level sync ───
    if (fs.existsSync(userDir)) {
      const linkOrCopy = (src: string, dst: string, label: string) => {
        if (!fs.existsSync(src) || fs.existsSync(dst)) return
        try {
          fs.symlinkSync(src, dst, 'junction')
          debugLog(`[sync] Symlinked user ${label}`, { tokenId: hash, target: dst })
        } catch {
          copyDirRecursive(src, dst)
          debugLog(`[sync] Copied user ${label} (symlink failed)`, { tokenId: hash, target: dst })
        }
      }
      linkOrCopy(path.join(userDir, 'agents'), path.join(claudeDir, 'agents'), 'agents')
      linkOrCopy(path.join(userDir, 'commands'), path.join(claudeDir, 'commands'), 'commands')

      // Append user CLAUDE.md (idempotent strip + append)
      const userClaudeMd = path.join(userDir, 'CLAUDE.md')
      if (fs.existsSync(userClaudeMd)) {
        const sessionClaudeMd = path.join(settingsDir, 'CLAUDE.md')
        const userContent = fs.readFileSync(userClaudeMd, 'utf-8')
        if (userContent.trim()) {
          const existingRaw = fs.existsSync(sessionClaudeMd) ? fs.readFileSync(sessionClaudeMd, 'utf-8') : ''
          const cleaned = stripSyncedBlocks(existingRaw)
          fs.writeFileSync(sessionClaudeMd, cleaned.trimEnd() + '\n\n# User Instructions (synced)\n\n' + userContent, 'utf-8')
          debugLog('[sync] Appended user CLAUDE.md', { tokenId: hash })
        }
      }

      // NEVER mirror to global ~/.claude/CLAUDE.md — the bridge container
      // shares HOME across all tokens and a "last-token-wins" mirror leaks
      // one user's instructions (corp proxy whitelists, custom MCP routing,
      // company-internal rules) into every other concurrent session.
      // Session CLAUDE.md sits at <settingsDir>/CLAUDE.md and already has
      // the synced user content appended above — CLI picks it up via cwd.
      // Older bridge builds wrote per-token content into the global file;
      // re-pin it back to the tenant-agnostic default on every spawn.
      ensureGlobalBridgeClaudeMd()
    }

    // ─── Project-level sync ───
    if (userCwd) {
      const projDir = getProjectSyncDir(hash, userCwd)
      if (fs.existsSync(projDir)) {
        const copyIfPresent = (src: string, dst: string, label: string) => {
          if (!fs.existsSync(src)) return
          copyDirRecursive(src, dst)
          debugLog(`[sync] Copied project ${label}`, { tokenId: hash, project: userCwd })
        }
        copyIfPresent(path.join(projDir, 'rules'), path.join(claudeDir, 'rules'), 'rules')
        copyIfPresent(path.join(projDir, 'agents'), path.join(claudeDir, 'agents'), 'agents')
        copyIfPresent(path.join(projDir, 'commands'), path.join(claudeDir, 'commands'), 'commands')

        // Append project CLAUDE.md to session CLAUDE.md (idempotent — strips
        // any prior `# Project Instructions (synced)` block first).
        const projClaudeMd = path.join(projDir, 'CLAUDE.md')
        if (fs.existsSync(projClaudeMd)) {
          const sessionClaudeMd = path.join(settingsDir, 'CLAUDE.md')
          const projContent = fs.readFileSync(projClaudeMd, 'utf-8')
          if (projContent.trim()) {
            const existingRaw = fs.existsSync(sessionClaudeMd) ? fs.readFileSync(sessionClaudeMd, 'utf-8') : ''
            const cleaned = stripSyncedBlocks(existingRaw, { stripUser: false, stripProject: true })
            fs.writeFileSync(sessionClaudeMd, cleaned.trimEnd() + '\n\n# Project Instructions (synced)\n\n' + projContent, 'utf-8')
            debugLog('[sync] Appended project CLAUDE.md', { tokenId: hash, project: userCwd })
          }
        }

        // Copy .claude/CLAUDE.md into session's .claude/CLAUDE.md
        const projDotClaudeMd = path.join(projDir, '.claude', 'CLAUDE.md')
        if (fs.existsSync(projDotClaudeMd)) {
          const target = path.join(claudeDir, 'CLAUDE.md')
          const content = fs.readFileSync(projDotClaudeMd, 'utf-8')
          if (content.trim()) {
            fs.writeFileSync(target, content, 'utf-8')
            debugLog('[sync] Copied project .claude/CLAUDE.md', { tokenId: hash, project: userCwd })
          }
        }

        // Copy .claude.json (NOT .mcp.json or .claude/settings.json — bridge generates those)
        const projClaudeJson = path.join(projDir, '.claude.json')
        if (fs.existsSync(projClaudeJson)) {
          const target = path.join(settingsDir, '.claude.json')
          fs.copyFileSync(projClaudeJson, target)
          debugLog('[sync] Copied project .claude.json', { tokenId: hash, project: userCwd })
        }

        debugLog('[sync] Project sync applied', { tokenId: hash, project: userCwd })
      }
    }
  } catch (err) {
    // Sync failure should never block session creation
    warnLog('[sync] Failed to apply sync data (non-fatal)', { error: String(err), tokenId })
  }
}
