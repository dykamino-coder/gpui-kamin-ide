// ============================================================================
// CLI Argument Parser
// ============================================================================

export type LogLevel = 'error' | 'warn' | 'info' | 'debug' | 'verbose'

export interface ParsedArgs {
  port?: number
  host?: string
  debug: boolean
  verbose: boolean
  help: boolean
  logLevel?: LogLevel
}

const VALID_LOG_LEVELS = new Set<LogLevel>(['error', 'warn', 'info', 'debug', 'verbose'])

function parse(): ParsedArgs {
  const argv = process.argv.slice(2)
  const result: ParsedArgs = { debug: false, verbose: false, help: false }

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if ((arg === "--port" || arg === "-p") && argv[i + 1]) {
      result.port = parseInt(argv[++i]!, 10)
    } else if ((arg === "--host" || arg === "-H") && argv[i + 1]) {
      result.host = argv[++i]!
    } else if (arg === "--log-level" && argv[i + 1]) {
      const val = argv[++i]! as LogLevel
      if (VALID_LOG_LEVELS.has(val)) {
        result.logLevel = val
      }
    } else if (arg === "--debug" || arg === "-d") {
      result.debug = true
    } else if (arg === "--verbose" || arg === "-v") {
      result.verbose = true
    } else if (arg === "--help" || arg === "-h") {
      result.help = true
    }
  }

  return result
}

export const args = parse()
