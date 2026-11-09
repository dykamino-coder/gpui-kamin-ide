import fs from 'node:fs'
import path from 'node:path'

/** Empty browser config must not erase the host's configured project root. */
export function projectDirectory(root: string, requested: unknown): string {
  if (requested !== undefined && typeof requested !== 'string') throw new Error('Expected a project directory')
  const directory = fs.realpathSync(path.resolve(root, requested?.trim() || root))
  if (!fs.statSync(directory).isDirectory()) throw new Error('Project directory is not a directory')
  return directory
}

/** Clarify the legacy server template's "user machine" for a container client. */
export function containerSessionInstructions(cwd: string): string {
  return `## Browser workspace execution environment
This session's tool host is the Linux workspace container. The browser is only a thin interface.
The Bridge server's remote-session instructions use "user's machine" to mean this MCP client host: here that is the workspace container, not the browser user's computer and not the separate Bridge server container.
The project working directory is ${JSON.stringify(cwd)} in the workspace container. Use this directory as the project root and resolve relative paths against it.
Read, Write, Edit, Glob, Grep, Bash, PowerShell, hooks and client MCP tools operate on this container's filesystem and network. Bash starts in this project directory; use Linux paths and shell syntax.
Act directly on the container project using these tools, as you would on a local machine. The project is already selected; do not ask the user for a folder on their computer or ask them to run commands there.
The whole workspace container filesystem is available when the task requires it. The separate Bridge server's CLI/settings filesystem is not the project.
Preserve the user's task, project instructions and tool permission decisions.`
}
