// Persistence note (KaminIDE VSIX): the Electron app used `electron-store`; the
// extension persists the same `rules` map to a JSON file in the global-storage
// dir injected at activation via setPermissionStorageDir().
import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'

export type PermissionDecision = 'allow' | 'deny' | 'ask'

export type ToolCategory = 'read-only' | 'write' | 'shell' | 'git' | 'network' | 'agent' | 'task' | 'team' | 'ui'

interface PermissionRule {
  decision: 'allow' | 'deny'
}

interface PermissionStoreSchema {
  rules: Record<string, PermissionRule>
}

/**
 * Tool → category mapping.
 * Categories determine default permission behavior.
 */
const TOOL_CATEGORIES: Record<string, ToolCategory> = {
  // Read-only (auto-allow)
  Read: 'read-only',
  Glob: 'read-only',
  Grep: 'read-only',
  ToolSearch: 'read-only',

  // Network (auto-allow)
  WebFetch: 'network',

  // Write (ask)
  Write: 'write',
  Edit: 'write',
  NotebookEdit: 'write',

  // Shell (always ask)
  Bash: 'shell',

  // Git (ask)
  EnterWorktree: 'git',

  // Agent & Tasks (auto-allow — sandboxed by Claude CLI)
  Agent: 'agent',
  TaskCreate: 'task',
  TaskGet: 'task',
  TaskUpdate: 'task',
  TaskList: 'task',
  TaskOutput: 'task',
  TaskStop: 'task',

  // Teams (auto-allow)
  TeamCreate: 'team',
  TeamDelete: 'team',
  SendMessage: 'team',

  // UI (auto-allow — just shows widgets)
  AskUserQuestion: 'ui',
  EnterPlanMode: 'ui',
  ExitPlanMode: 'ui',
  Skill: 'ui',
  ShowWidget: 'ui', // renders sandboxed HTML inline — no host side-effects
}

/**
 * Default decisions by category.
 * - read-only: auto-allow (no risk of side-effects)
 * - network: auto-allow (read-only web fetch)
 * - write: ask by default (modifies files)
 * - git: ask by default (modifies repository)
 * - shell: always ask (arbitrary command execution)
 */
const CATEGORY_DEFAULTS: Record<ToolCategory, PermissionDecision> = {
  'read-only': 'allow',
  'network': 'allow',
  'write': 'ask',
  'git': 'ask',
  'shell': 'ask',
  'agent': 'allow',
  'task': 'allow',
  'team': 'allow',
  'ui': 'allow',
}

let permissionStorageDir = os.tmpdir()
/** Set the dir for the persisted permission-rules JSON file. Called once at
 *  extension activation with the global-storage path. */
export function setPermissionStorageDir(dir: string): void {
  permissionStorageDir = dir
}

/** Tiny JSON-file store replacing electron-store for the `rules` map. */
class RulesStore {
  private readonly file = path.join(permissionStorageDir, 'open-claude-bridge-permissions.json')
  private data: PermissionStoreSchema

  constructor() {
    try {
      this.data = JSON.parse(fs.readFileSync(this.file, 'utf8')) as PermissionStoreSchema
      if (!this.data.rules) this.data.rules = {}
    } catch {
      this.data = { rules: {} }
    }
  }

  get(key: 'rules'): Record<string, PermissionRule> {
    return this.data[key]
  }

  set(key: 'rules', value: Record<string, PermissionRule>): void {
    this.data[key] = value
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true })
      const tmp = `${this.file}.tmp`
      fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2), 'utf8')
      fs.renameSync(tmp, this.file)
    } catch {
      /* best-effort */
    }
  }
}

export class PermissionManager {
  private store: RulesStore

  /** Session-level overrides (not persisted) */
  private sessionRules = new Map<string, 'allow' | 'deny'>()

  constructor() {
    this.store = new RulesStore()
  }

  /**
   * Get the category for a tool.
   */
  getToolCategory(toolName: string): ToolCategory {
    return TOOL_CATEGORIES[toolName] || 'shell' // Unknown tools default to most restrictive
  }

  /**
   * Check if a tool invocation should be allowed, denied, or requires user approval.
   *
   * Priority:
   * 1. Session-level override (set via setSessionPermission)
   * 2. Persisted rule (stored in the bridge JSON store)
   * 3. Category default
   */
  checkPermission(toolName: string, _input: Record<string, unknown>): PermissionDecision {
    // 1. Session override
    const sessionDecision = this.sessionRules.get(toolName)
    if (sessionDecision) return sessionDecision

    // 2. Persisted rule
    const rules = this.store.get('rules')
    const persistedRule = rules[toolName]
    if (persistedRule) return persistedRule.decision

    // 3. Category default
    const category = this.getToolCategory(toolName)
    return CATEGORY_DEFAULTS[category]
  }

  /**
   * Set a persistent permission rule for a tool.
   * Survives app restarts.
   */
  setPermission(toolName: string, decision: 'allow' | 'deny'): void {
    const rules = this.store.get('rules')
    rules[toolName] = { decision }
    this.store.set('rules', rules)
  }

  /**
   * Set a session-level permission override.
   * Cleared when the app restarts.
   */
  setSessionPermission(toolName: string, decision: 'allow' | 'deny'): void {
    this.sessionRules.set(toolName, decision)
  }

  /**
   * Remove a persisted permission rule, reverting to category default.
   */
  removePermission(toolName: string): void {
    const rules = this.store.get('rules')
    delete rules[toolName]
    this.store.set('rules', rules)
  }

  /**
   * Clear all session-level overrides.
   */
  clearSessionPermissions(): void {
    this.sessionRules.clear()
  }

  /**
   * Get all persisted permission rules.
   */
  getPersistedRules(): Record<string, PermissionRule> {
    return { ...this.store.get('rules') }
  }

  /**
   * Reset all permissions to defaults (clear persisted + session).
   */
  resetAll(): void {
    this.store.set('rules', {})
    this.sessionRules.clear()
  }
}
