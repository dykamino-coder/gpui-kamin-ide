/** Fixed argv only: account writes belong to the shared app-server API. */
export function codexTerminalArgs(command: string): string[] {
  switch (command) {
    case 'interactive':
    case 'usage':
      // Bridge owns this PTY and its CODEX_HOME lease; no background writer may outlive it.
      return ['--no-daemon']
    case 'resume':
      return ['--no-daemon', 'resume', '--last']
    case 'status':
      return ['login', 'status']
    default:
      throw new Error('Unknown Codex terminal command')
  }
}
