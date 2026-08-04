// Tiny shared helpers for the SyncedDataPanel split (Sprint 5 / Stage E1).

export function formatSize(n?: number): string {
  if (n == null) return ''
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}

export function joinServerPath(parts: string[]): string {
  // Server paths are POSIX style (/home/bridge/...)
  return parts.join('/')
}

export interface TreeNode {
  name: string
  type: 'dir' | 'file'
  size?: number
  children?: TreeNode[]
}

export interface SelectedFile {
  absPath: string
  /** human-readable short label like "users/<hash>/skills/docx/SKILL.md" */
  label: string
}

export interface SyncTreePayload {
  tokenId: string
  basePath: string
  user: { path: string; exists: boolean; tree: TreeNode[] }
  projects: Array<{ projectPath: string; path: string; tree: TreeNode[] }>
}

export interface FilePayload {
  path: string
  size: number
  truncated: boolean
  content: string
}
