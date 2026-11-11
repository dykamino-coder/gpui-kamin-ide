import fs from 'node:fs'
import path from 'node:path'

/** Empty browser config must not erase the host's configured project root. */
export function projectDirectory(root: string, requested: unknown): string {
  if (requested !== undefined && typeof requested !== 'string') throw new Error('Expected a project directory')
  const directory = fs.realpathSync(path.resolve(root, requested?.trim() || root))
  if (!fs.statSync(directory).isDirectory()) throw new Error('Project directory is not a directory')
  return directory
}

/** Declared to the Bridge server as the place where this host's tools run. */
export const WORKSPACE_HOST_PLACE = 'the workspace host'

/** What the declaration cannot say: whose machine this is and how to act on it. */
export function workspaceSessionInstructions(cwd: string): string {
  return `## Workspace host
The tool host of this session is the Bridge workspace host; the browser is only a thin interface to it. The workspace host is not the browser user's computer and not the Bridge server container.
The project working directory is ${JSON.stringify(cwd)} on the workspace host. Use it as the project root and resolve relative paths against it.
The user-tools of this session, hooks and client MCP tools operate on the workspace host's filesystem and network; shell tools start in the project directory.
Act directly on the project with these tools. The project is already selected: do not ask the user for a folder on their computer or ask them to run commands there.
Preserve the user's task, project instructions and tool permission decisions.`
}
