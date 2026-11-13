// Argument patterns of Claude Code permission rules (`Bash(git *)`,
// `Edit(src/**)`), evaluated against a tool input. The CLI evaluates a hook's
// `if` with the matcher of the native tool; for the Bridge tool that replaces
// it (`mcp__user-tools__Bash`) the CLI has no matcher and the condition never
// holds (INC-2026-0107). These functions model sampled CLI 2.1.322 matchers;
// remaining parity gaps are recorded in the incident card. They implement
// the wildcard rule, the shell rule (each simple command of a compound
// command; a command the parser cannot split holds, as in the CLI) and the
// file rule (gitignore pattern relative to the rule's root, then wildcard).

const ESCAPED_STAR = '\x00ESCAPED_STAR\x00'
const ESCAPED_BACKSLASH = '\x00ESCAPED_BACKSLASH\x00'
const GLOBSTAR = '\x00GLOBSTAR\x00'

/** The CLI's wildcard rule: `*` matches any text, `\*` a literal star, and a
 *  single trailing ` *` also accepts the bare command (`ls *` matches `ls`). */
export function wildcardMatches(rule: string, value: string, ignoreCase = false, collapseSpaces = false): boolean {
  if (rule.includes('\x00')) return false
  const trimmed = rule.trim()
  const pattern = collapseSpaces ? trimmed.replace(/[ \t]+/g, ' ') : trimmed
  const subject = collapseSpaces ? value.replace(/[ \t]+/g, ' ') : value
  let marked = ''
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern[i]
    if (ch === '\\' && i + 1 < pattern.length) {
      const next = pattern[i + 1]
      if (next === '*') {
        marked += ESCAPED_STAR
        i++
        continue
      }
      if (next === '\\') {
        marked += ESCAPED_BACKSLASH
        i++
        continue
      }
    }
    marked += ch
  }
  let source = marked
    .replace(/[.+?^${}()|[\]\\'"]/g, '\\$&')
    .replace(/\/(?:\*\*\/)+/g, GLOBSTAR)
    .replaceAll('*', '.*')
    .replaceAll(GLOBSTAR, '/(?:.*/)?')
    .replaceAll(ESCAPED_STAR, '\\*')
    .replaceAll(ESCAPED_BACKSLASH, '\\\\')
  const stars = (marked.match(/\*/g) ?? []).length
  if (source.endsWith(' .*') && stars === 1) source = source.slice(0, -3) + '( .*)?'
  return new RegExp(`^${source}$`, 's' + (ignoreCase ? 'i' : '')).test(subject)
}

/** `npm run:*` — the legacy prefix form of a shell rule. */
function legacyPrefix(rule: string): string | null {
  return rule.match(/^(.+):\*$/)?.[1] ?? null
}

const SHELL_OPERATORS = ['&&', '||', '|&', ';;', ';', '|', '&', '\n']

/** The simple commands of a shell command as argv strings, or null when the
 *  command uses a construct this splitter does not model (substitution,
 *  heredoc, subshell, compound keyword). The CLI's matcher then holds. */
export function simpleShellCommands(command: string): string[] | null {
  const commands: string[][] = []
  let words: string[] = []
  let word = ''
  let inWord = false
  // The target word of a redirection is not an argument.
  let skipNext = false
  const endWord = () => {
    if (inWord) {
      if (skipNext) skipNext = false
      else words.push(word)
    }
    word = ''
    inWord = false
  }
  const endCommand = () => {
    endWord()
    if (words.length) commands.push(words)
    words = []
  }
  for (let i = 0; i < command.length; i++) {
    const ch = command[i]!
    if (ch === "'") {
      const close = command.indexOf("'", i + 1)
      if (close === -1) return null
      if (command.slice(i + 1, close).includes('\\\n')) return null
      word += command.slice(i + 1, close)
      inWord = true
      i = close
      continue
    }
    if (ch === '"') {
      let j = i + 1
      for (; j < command.length && command[j] !== '"'; j++) {
        if (command.startsWith('$(', j) || command.startsWith('${', j) || command[j] === '`') return null
        if (command[j] === '\\' && j + 1 < command.length && '"\\$`\n'.includes(command[j + 1]!)) {
          if (command[j + 1] === '\n') return null
          j++
        }
        word += command[j]
      }
      if (j >= command.length) return null
      inWord = true
      i = j
      continue
    }
    if (ch === '\\' && i + 1 < command.length) {
      // Native matching accepts an unsupported continuation inside a word;
      // one between words is discarded without creating an empty argument.
      if (command[i + 1] === '\n') {
        if (inWord) return null
      } else {
        word += command[i + 1]
        inWord = true
      }
      i++
      continue
    }
    // Only active shell syntax is complex. Single-quoted and escaped text
    // above is literal: `printf '$(pwd)'` must not satisfy `git *`.
    if (
      command.startsWith('$(', i) ||
      command.startsWith('${', i) ||
      ch === '`' ||
      command.startsWith('<(', i) ||
      command.startsWith('>(', i) ||
      command.startsWith('<<', i)
    )
      return null
    if (ch === '#' && !inWord) {
      const newline = command.indexOf('\n', i)
      if (newline === -1) break
      i = newline - 1
      continue
    }
    // Effective CLI environment expansion is not modeled yet (INC-2026-0107).
    // The current approximation omits unquoted simple expansions.
    if (ch === '$' && /^[A-Za-z_]/.test(command[i + 1] ?? '')) {
      const name = command.slice(i + 1).match(/^[A-Za-z_][A-Za-z0-9_]*/)![0]
      i += name.length
      continue
    }
    if (ch === '>' || ch === '<' || (ch === '&' && command[i + 1] === '>')) {
      if (/^\d+$/.test(word)) {
        word = ''
        inWord = false
      } else endWord()
      while (i + 1 < command.length && '<>&|'.includes(command[i + 1]!)) i++
      skipNext = true
      continue
    }
    const operator = SHELL_OPERATORS.find((op) => command.startsWith(op, i))
    if (operator) {
      endCommand()
      i += operator.length - 1
      continue
    }
    if (ch === ' ' || ch === '\t' || ch === '\r') {
      endWord()
      continue
    }
    if (ch === '(' || ch === ')' || ch === '{' || ch === '}') return null
    word += ch
    inWord = true
  }
  endCommand()
  const out: string[] = []
  for (const argv of commands) {
    const kept: string[] = []
    let assignments = true
    for (let k = 0; k < argv.length; k++) {
      const w = argv[k]!
      if (assignments && /^[A-Za-z_][A-Za-z0-9_]*=/.test(w)) continue
      assignments = false
      kept.push(w)
    }
    if (kept.length === 0) continue
    while (/^(?:if|then|else|elif|while|until|do)$/.test(kept[0] ?? '')) kept.shift()
    if (/^(?:fi|done)$/.test(kept[0] ?? '')) continue
    if (/^(?:for|case|esac|function|select)$/.test(kept[0] ?? '')) return null
    out.push(kept.join(' '))
  }
  return out
}

/** The CLI's Bash rule: the rule accepts the call when it accepts any simple
 *  command, also behind `xargs`. */
export function bashRuleMatches(rule: string, command: string): boolean {
  const commands = simpleShellCommands(command)
  if (commands === null || commands.length === 0) return true
  const prefix = legacyPrefix(rule)
  return commands.some((candidate) => {
    if (prefix !== null) {
      return (
        candidate === prefix ||
        candidate.startsWith(`${prefix} `) ||
        candidate === `xargs ${prefix}` ||
        candidate.startsWith(`xargs ${prefix} `)
      )
    }
    return wildcardMatches(rule, candidate, false, true) || wildcardMatches(`xargs ${rule}`, candidate, false, true)
  })
}

/** Models simple PowerShell commands case-insensitively, one candidate per
 *  pipeline element or statement. Nested-AST parity gaps remain in INC-0107. */
export function powershellRuleMatches(rule: string, command: string): boolean {
  const statements: string[] = []
  let words: string[] = [],
    word = '',
    active = false
  let skipRedirectTarget = false
  const endWord = () => {
    if (active) {
      if (skipRedirectTarget) skipRedirectTarget = false
      else words.push(word)
    }
    word = ''
    active = false
  }
  const endCommand = () => {
    endWord()
    // The command on the right of an assignment still has its own CommandAst.
    if (/^\$[A-Za-z_]\w*$/.test(words[0] ?? '') && words[1] === '=') words.splice(0, 2)
    if (words.length) statements.push(words.join(' '))
    words = []
    skipRedirectTarget = false
  }
  for (let i = 0; i < command.length; i++) {
    const ch = command[i]!
    if (ch === "'" || ch === '"') {
      const quote = ch
      let closed = false
      for (i++; i < command.length; i++) {
        const c = command[i]!
        if (c === quote) {
          if (command[i + 1] === quote) {
            word += quote
            i++
            continue
          }
          closed = true
          break
        }
        if (quote === '"' && c === '`' && i + 1 < command.length) {
          word += command[++i]
          continue
        }
        if (quote === '"' && command.startsWith('$(', i)) return true
        word += c
      }
      if (!closed) return true
      active = true
    } else if (ch === '`' && (command[i + 1] === '\n' || command.slice(i + 1, i + 3) === '\r\n')) {
      // In an unquoted word the AST keeps the escaped newline in its value.
      if (active) word += command[i + 1] === '\r' ? '\r\n' : '\n'
      i += command[i + 1] === '\r' ? 2 : 1
    } else if (ch === '`' && i + 1 < command.length) {
      word += command[++i]
      active = true
    } else if (ch === '#' && !active) {
      while (i < command.length && command[i] !== '\n') i++
      endCommand()
    } else if (
      !active &&
      words.length === 0 &&
      !skipRedirectTarget &&
      ((ch === '&' && command[i + 1] !== '&') || (ch === '.' && /\s/.test(command[i + 1] ?? '')))
    ) {
      // Invocation operators are outside CommandElements in the native AST.
      continue
    } else if (ch === '>') {
      // A stream number adjacent to `>` belongs to the redirection, not argv.
      if (active && /^[1-6*]$/.test(word)) {
        word = ''
        active = false
      }
      endWord()
      while (command[i + 1] === '>') i++
      skipRedirectTarget = true
    } else if (command.startsWith('$(', i) || command.startsWith('@(', i) || ch === '{' || ch === '}') {
      return true
    } else if (';|\n'.includes(ch) || command.startsWith('&&', i)) {
      endCommand()
      if (command[i + 1] === ch) i++
    } else if (/\s/.test(ch)) {
      endWord()
    } else {
      word += ch
      active = true
    }
  }
  endCommand()
  if (statements.length === 0) return true
  const prefix = legacyPrefix(rule)
  return statements.some((candidate) => {
    const [name, ...args] = candidate.split(' ')
    const base = name!.replaceAll('\\', '/').split('/').at(-1)!
    return [candidate, [base, ...args].join(' ')].some((c) => {
      if (prefix !== null)
        return c.toLowerCase() === prefix.toLowerCase() || c.toLowerCase().startsWith(prefix.toLowerCase() + ' ')
      return wildcardMatches(rule, c, true, true)
    })
  })
}
