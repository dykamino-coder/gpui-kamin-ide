// What this client host can execute, declared to the Bridge server at session
// create/resume (INC-2026-0097). The server serves exactly these built-in tools
// and words its instructions from `platform` and `place`; it knows nothing
// about the host otherwise.
import { findBashExecutable, findPowerShellExecutable, whichExecutable, type RuntimeLookupOptions } from '../utils/command-runtime'

export interface HostDeclaration {
  tools: string[]
  platform: string
  place: string
}

/** Properties of the embedding host that cannot be probed from this module. */
export interface HostTraits {
  /** Noun phrase naming where the tools run. */
  place: string
  /** `kaminBridge.preview.*` commands have an implementation. */
  preview: boolean
}

export const USER_MACHINE_TRAITS: HostTraits = { place: "the user's machine", preview: false }

const PREVIEW_TOOLS = ['preview_mount', 'preview_update', 'preview_unmount']
const BACKGROUND_SHELL_TOOLS = ['BashOutput', 'KillShell']
const GIT_TOOLS = ['EnterWorktree', 'ExitWorktree']

async function resolves(find: () => Promise<string | null>): Promise<boolean> {
  try {
    return Boolean(await find())
  } catch {
    return false
  }
}

/** Registered tool names reduced to the ones this host has an executor for. */
export async function probeHostDeclaration(
  registered: string[],
  traits: HostTraits,
  lookup: RuntimeLookupOptions = {},
): Promise<HostDeclaration> {
  const platform = lookup.platform ?? process.platform
  const which = lookup.which ?? ((command: string) => whichExecutable(command, platform))
  const [bash, powerShell, git] = await Promise.all([
    resolves(() => findBashExecutable(lookup)),
    resolves(() => findPowerShellExecutable(lookup)),
    resolves(() => which('git')),
  ])
  const missing = new Set<string>([
    ...(bash ? [] : ['Bash']),
    ...(powerShell ? [] : ['PowerShell']),
    ...(bash || powerShell ? [] : BACKGROUND_SHELL_TOOLS),
    ...(git ? [] : GIT_TOOLS),
    ...(traits.preview ? [] : PREVIEW_TOOLS),
  ])
  return { tools: registered.filter((name) => !missing.has(name)), platform, place: traits.place }
}
