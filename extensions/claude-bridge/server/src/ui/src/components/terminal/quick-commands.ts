export type TerminalEngine = 'claude' | 'codex'
export type CodexTerminalCommand = 'interactive' | 'resume' | 'status' | 'usage'
export type QuickCommand = {
  label: string
  icon: string
  color: string
} & ({ cmd: string } | { codex: CodexTerminalCommand } | { account: 'login' | 'logout' })

const labels = ['Resume', 'Auth Status', 'Usage', 'Login', 'Logout']
const icons = ['rocket', 'rotate-left', 'circle-check', 'chart-pie', 'right-to-bracket', 'right-from-bracket']
const colors = ['blue', 'purple', 'green', 'yellow', 'cyan', 'red']

export function terminalEngines(codexAvailable: boolean): TerminalEngine[] {
  return codexAvailable ? ['claude', 'codex'] : ['claude']
}

export function quickCommands(engine: TerminalEngine): QuickCommand[] {
  const actions =
    engine === 'claude'
      ? ['', ' --resume', ' auth status', ' /usage', ' /login', ' /logout'].map((suffix) => ({
          cmd: `claude --dangerously-skip-permissions${suffix}`,
        }))
      : [
          { codex: 'interactive' as const },
          { codex: 'resume' as const },
          { codex: 'status' as const },
          { codex: 'usage' as const },
          { account: 'login' as const },
          { account: 'logout' as const },
        ]
  return actions.map((action, i) => ({
    ...action,
    label: [engine === 'claude' ? 'Claude' : 'Codex', ...labels][i],
    icon: `fa-solid fa-${icons[i]}`,
    color: `var(--accent-${colors[i]})`,
  }))
}

/** Auth mutations deliberately use the shared app-server, never terminal input. */
export async function codexAccountAction(
  action: 'login' | 'logout',
  api: {
    startCodexLogin(): Promise<unknown>
    logoutCodex(): Promise<unknown>
    getCodexAccount(force: boolean): Promise<unknown>
  },
): Promise<void> {
  try {
    await (action === 'login' ? api.startCodexLogin() : api.logoutCodex())
  } finally {
    await api.getCodexAccount(true)
  }
}
