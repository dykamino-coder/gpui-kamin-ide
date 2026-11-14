// Native Bash conditions use the CLI's startup home, before settings.env is
// applied. Hook children and HTTP header expansion see the later environment.
export const BASH_HOME_ENV = 'CLAUDE_BRIDGE_BASH_HOME'
export const BASH_HOME_HEADER = 'X-Bridge-Hook-Bash-Home'
