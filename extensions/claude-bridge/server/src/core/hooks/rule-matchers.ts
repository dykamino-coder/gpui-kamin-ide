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
export function simpleShellCommands(command: string, home?: string): string[] | null {
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
        } else if (command[j] === '$' && /^[A-Za-z_]/.test(command[j + 1] ?? '')) {
          const name = command.slice(j + 1).match(/^[A-Za-z_][A-Za-z0-9_]*/)![0]
          if (name !== 'HOME' || home === undefined) return null
          word += home
          j += name.length
          continue
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
    // The native matcher knows HOME (os.homedir), rather than expanding every
    // environment variable. An unknown variable makes the parse conservative,
    // even when that variable exists in settings.env or the process environment.
    if (ch === '$' && /^[A-Za-z_]/.test(command[i + 1] ?? '')) {
      const name = command.slice(i + 1).match(/^[A-Za-z_][A-Za-z0-9_]*/)![0]
      if (name !== 'HOME' || !home || /[\s*?\[\]]/.test(home)) return null
      word += home
      inWord = true
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
export function bashRuleMatches(rule: string, command: string, home?: string): boolean {
  const commands = simpleShellCommands(command, home)
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

/** Find a group's end without treating quoted delimiters as syntax. */
function powershellGroupEnd(command: string, open: number): number | null {
  const opening = command[open]!
  const closing = opening === '{' ? '}' : ')'
  let depth = 1
  let quote = ''
  for (let i = open + 1; i < command.length; i++) {
    const ch = command[i]!
    if (ch === '`' && quote !== "'") {
      i++
    } else if (quote) {
      if (ch === quote) {
        if (command[i + 1] === quote) i++
        else quote = ''
      }
    } else if (ch === "'" || ch === '"') {
      quote = ch
    } else if (command.startsWith('<#', i)) {
      const close = powershellCommentEnd(command, i)
      if (close === null) return null
      i = close
    } else if (ch === '#') {
      while (i < command.length && command[i] !== '\n') i++
    } else if (ch === opening) {
      depth++
    } else if (ch === closing && --depth === 0) {
      return i
    }
  }
  return null
}

/** The sampled native matcher ends a block comment at its first terminator. */
function powershellCommentEnd(command: string, open: number): number | null {
  const close = command.indexOf('#>', open + 2)
  return close === -1 ? null : close + 1
}

// These scalar expressions have no CommandAst children. Other expression
// syntax remains outside this sampled matcher rather than becoming a command.
const POWERSHELL_SCALAR = String.raw`(?:'(?:[^']|'')*'|"(?:[^"\x60$]|\$(?!\()|\x60[\s\S]|"")*"|\$[\w:]+|[-+]?\d+(?:\.\d+)?)`
const POWERSHELL_SCALAR_LIST = new RegExp(`^${POWERSHELL_SCALAR}(?:\\s*,\\s*${POWERSHELL_SCALAR})*$`)

/** Hashtable keys are expressions, not command names. Only their values
 *  contribute command children for the sampled literal-key syntax. */
function powershellHashtableCommands(body: string): string[] | null {
  const statements: string[] = []
  let start = 0,
    valueStart: number | null = null,
    quote = ''
  const endEntry = (end: number): boolean => {
    if (valueStart === null) return !body.slice(start, end).trim()
    const value = body.slice(valueStart, end).trim()
    if (!value) return false
    // Scalar arithmetic contains no CommandAst, just like scalar literals.
    if (!POWERSHELL_SCALAR_LIST.test(value) && !/^[-+]?\d+(?:\s*[-+*/%]\s*\d+)+$/.test(value)) {
      const nested = powershellCommands(value)
      if (nested === null) return false
      statements.push(...nested)
    }
    return true
  }
  for (let i = 0; i < body.length; i++) {
    const ch = body[i]!
    if (ch === '`' && quote !== "'") i++
    else if (quote) {
      if (ch === quote) {
        if (body[i + 1] === quote) i++
        else quote = ''
      }
    } else if (ch === "'" || ch === '"') quote = ch
    else if (body.startsWith('<#', i)) {
      const close = powershellCommentEnd(body, i)
      if (close === null) return null
      i = close
    } else if (ch === '#') {
      while (i < body.length && body[i] !== '\n') i++
      i--
    } else if (ch === '(' || ch === '{') {
      const close = powershellGroupEnd(body, i)
      if (close === null) return null
      i = close
    } else if (ch === '=' && valueStart === null) {
      const key = body.slice(start, i).trim()
      if (!/^[A-Za-z_]\w*$/.test(key) && !POWERSHELL_SCALAR_LIST.test(key)) return null
      valueStart = i + 1
    } else if (ch === ';' || ch === '\n') {
      if (!endEntry(i)) return null
      start = i + 1
      valueStart = null
    }
  }
  if (quote || !endEntry(body.length)) return null
  return statements
}

/** Sampled CommandAst candidates, including commands inside grouped pipelines
 *  and subexpressions. null keeps the fallback for unsupported syntax. */
function powershellCommands(command: string): string[] | null {
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
    if (words.length) statements.push(words.join(' '))
    words = []
    skipRedirectTarget = false
  }
  for (let i = 0; i < command.length; i++) {
    const ch = command[i]!
    const assignment =
      ch === '$' && !active && words.length === 0 && !skipRedirectTarget
        ? /^\$(?:[A-Za-z_]\w*(?::[A-Za-z_]\w*)?|\{[A-Za-z_]\w*(?::[A-Za-z_]\w*)?\})\s*(?:\?\?|[+*/%\-])?=(?!=)/.exec(
            command.slice(i),
          )
        : null
    const adjacentVariable =
      ch === '$' && !active && words.length && !skipRedirectTarget
        ? /^\$[A-Za-z_]\w*(?::[A-Za-z_]\w*)?(?==)/.exec(command.slice(i))
        : null
    const bracedVariable =
      ch === '$' && (words.length || skipRedirectTarget)
        ? /^\$\{[A-Za-z_]\w*(?::[A-Za-z_]\w*)?\}/.exec(command.slice(i))
        : null
    if (assignment) {
      // Assignment operators and their variable are outside the RHS CommandAst.
      // Consume the prefix before braced variables become expression groups.
      i += assignment[0].length - 1
    } else if (ch === "'" || ch === '"') {
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
        if (quote === '"' && command.startsWith('$(', i)) {
          const close = powershellGroupEnd(command, i + 1)
          if (close === null) return null
          const nested = powershellCommands(command.slice(i + 2, close))
          if (nested === null) return null
          statements.push(...nested)
          // ExpandableStringExpressionAst.Value keeps the subexpression's
          // source, while its CommandAst children are matched independently.
          word += command.slice(i, close + 1)
          i = close
          continue
        }
        word += c
      }
      if (!closed) return null
      active = true
    } else if (ch === '`' && (command[i + 1] === '\n' || command.slice(i + 1, i + 3) === '\r\n')) {
      // In an unquoted word the AST keeps the escaped newline in its value.
      if (active) word += command[i + 1] === '\r' ? '\r\n' : '\n'
      i += command[i + 1] === '\r' ? 2 : 1
    } else if (ch === '`' && i + 1 < command.length) {
      word += command[++i]
      active = true
    } else if (command.startsWith('<#', i) && !active) {
      const close = powershellCommentEnd(command, i)
      if (close === null) return null
      i = close
    } else if (command.startsWith('--%', i) && !active && words.length && /\s|^$/.test(command[i + 3] ?? '')) {
      // Stop-parsing leaves the rest of this line as one literal value,
      // including quotes and apparent subexpressions or redirections.
      words.push('--%')
      const newline = command.indexOf('\n', i + 3)
      const end = newline === -1 ? command.length : newline
      const literal = command.slice(i + 3, end).replace(/^[ \t]+/, '')
      if (literal) words.push(literal)
      endCommand()
      i = end
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
    } else if (adjacentVariable) {
      // An unquoted simple variable followed by '=' is a separate AST element.
      // Recognize it before consuming the word so quotes and escapes stay literal.
      words.push(adjacentVariable[0])
      i += adjacentVariable[0].length - 1
    } else if (bracedVariable) {
      // Braces keep the variable and an adjacent literal in one argument.
      word += bracedVariable[0]
      active = true
      i += bracedVariable[0].length - 1
    } else if (ch === '>') {
      // A stream number adjacent to `>` belongs to the redirection, not argv.
      if (active && /^[1-6*]$/.test(word)) {
        word = ''
        active = false
      }
      endWord()
      while (command[i + 1] === '>') i++
      skipRedirectTarget = true
    } else if (
      command.startsWith('$(', i) ||
      command.startsWith('@(', i) ||
      command.startsWith('@{', i) ||
      ch === '(' ||
      ch === '{'
    ) {
      if (active) return null
      const open = ch === '(' || ch === '{' ? i : i + 1
      const close = powershellGroupEnd(command, open)
      if (close === null) return null
      const body = command.slice(open + 1, close).trim()
      let nested: string[] | null = []
      if (command.startsWith('@{', i)) nested = powershellHashtableCommands(body)
      else if (body && !POWERSHELL_SCALAR_LIST.test(body)) nested = powershellCommands(body)
      if (nested === null) return null
      statements.push(...nested)
      if (words.length || skipRedirectTarget) {
        // Non-string AST arguments retain their original source text.
        word = command.slice(i, close + 1)
        active = true
      }
      if (close + 1 < command.length && !/[\s;|&>]/.test(command[close + 1]!)) return null
      i = close
    } else if (ch === '}' || ch === ')') {
      return null
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
  return statements
}

/** Models sampled PowerShell command ASTs case-insensitively. Unsupported
 *  nested syntax still has parity gaps recorded in INC-0107. */
export function powershellRuleMatches(rule: string, command: string): boolean {
  // The native universal rule bypasses command extraction, including empty ASTs.
  if (rule.trim() === '*') return true
  const statements = powershellCommands(command)
  if (statements === null) return true
  const prefix = legacyPrefix(rule)
  return statements.some((candidate) => {
    const [name, ...args] = candidate.split(' ')
    // A module qualifier is omitted by the native matcher, while an absolute
    // or relative executable path remains part of the command's name.
    const base = /^[A-Za-z]:|^\\\\|^\.\.?\\/.test(name!) ? name! : name!.split('\\').at(-1)!
    const normalized = [base, ...args].join(' ')
    if (prefix !== null)
      return (
        normalized.toLowerCase() === prefix.toLowerCase() ||
        normalized.toLowerCase().startsWith(prefix.toLowerCase() + ' ')
      )
    return wildcardMatches(rule, normalized, true, true)
  })
}
