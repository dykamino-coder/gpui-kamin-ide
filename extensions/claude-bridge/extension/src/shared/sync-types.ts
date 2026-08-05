// ============================================================================
// Sync Types — shared between the VSIX bridge host and bridge server
// ============================================================================

/** Declarative part of an enabled plugin that the remote Claude CLI can load
 *  safely. Executable components (hooks/MCP/LSP/monitors) stay on the local
 *  extension host and are represented separately by `hooks`; the server-side
 *  proxy plugin contains only prompt/config assets. */
export interface SyncPluginData {
  id: string
  name: string
  marketplace: string
  sourceRoot: string
  manifest: Record<string, unknown>
  skills: Record<string, string>
  agents: Record<string, string>
  commands: Record<string, string>
  workflows: Record<string, string>
  outputStyles: Record<string, string>
  themes: Record<string, string>
  hooks: Record<string, unknown>
  settings?: string
}

export interface SyncUserData {
  skills: Record<string, string>   // relativePath → fileContent
  agents: Record<string, string>
  commands: Record<string, string> // ~/.claude/commands/*.md — custom slash commands
  plugins: Record<string, SyncPluginData> // pluginId → namespaced proxy-plugin snapshot
  settings?: string
  claudeMd?: string
}

export interface SyncProjectData {
  skills: Record<string, string>
  rules: Record<string, string>
  agents: Record<string, string>
  commands: Record<string, string> // .claude/commands/*.md
  settings?: string       // .claude/settings.json (hooks and project config)
  claudeMd?: string        // root CLAUDE.md
  dotClaudeMd?: string     // .claude/CLAUDE.md
  claudeJson?: string      // .claude.json
  projectPath: string      // for identification
}
