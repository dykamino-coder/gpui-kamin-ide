// ============================================================================
// Sync Types — shared between the VSIX bridge host and bridge server
// ============================================================================

export interface SyncUserData {
  skills: Record<string, string>   // relativePath → fileContent
  agents: Record<string, string>
  commands: Record<string, string> // ~/.claude/commands/*.md — custom slash commands
  settings?: string
  claudeMd?: string
}

export interface SyncProjectData {
  skills: Record<string, string>
  rules: Record<string, string>
  agents: Record<string, string>
  commands: Record<string, string> // .claude/commands/*.md
  claudeMd?: string        // root CLAUDE.md
  dotClaudeMd?: string     // .claude/CLAUDE.md
  claudeJson?: string      // .claude.json
  projectPath: string      // for identification
}
