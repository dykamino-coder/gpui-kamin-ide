// Credential redaction for text leaving the project session tools.
//
// The reader is the same token as the writer, but tool output goes to a model,
// into new transcripts and possibly into logs and pasted briefs; a credential a
// user once pasted should not be copied around again. Replacements contain no
// quotes or backslashes, so a redacted raw JSONL line stays valid JSON.
// This is pattern-based and therefore best effort: it is not a guarantee that
// no secret remains, and it never replaces the caller's own judgement.

const PATTERNS: Array<[string, RegExp]> = [
  ['private-key', /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g],
  ['anthropic-key', /\bsk-ant-[A-Za-z0-9_-]{20,}/g],
  ['openai-key', /\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}/g],
  ['github-token', /\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})/g],
  ['aws-access-key', /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g],
  ['slack-token', /\bxox[abposr]-[A-Za-z0-9-]{10,}/g],
  ['google-api-key', /\bAIza[0-9A-Za-z_-]{35}\b/g],
  ['jwt', /\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g],
  ['bearer', /(\b[Bb]earer\s+)[A-Za-z0-9._~+/=-]{16,}/g],
]

export interface Redacted {
  text: string
  count: number
}

export function redact(text: string): Redacted {
  let count = 0
  let out = text
  for (const [kind, re] of PATTERNS) {
    out = out.replace(re, (_m, prefix: unknown) => {
      count++
      return `${typeof prefix === 'string' ? prefix : ''}[REDACTED:${kind}]`
    })
  }
  return { text: out, count }
}
