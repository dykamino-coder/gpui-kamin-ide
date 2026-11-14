/** Fixed argv only: account writes belong to the shared app-server API. */
export function codexTerminalArgs(command: string): string[] {
  switch (command) {
    case 'interactive':
    case 'usage':
      return []
    case 'resume':
      return ['resume', '--last']
    case 'status':
      return ['login', 'status']
    default:
      throw new Error('Unknown Codex terminal command')
  }
}
