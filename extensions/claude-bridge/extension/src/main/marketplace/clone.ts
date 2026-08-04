import fs from 'fs'
import os from 'os'
import path from 'path'
import { assertValidName, assertValidGitUrl } from '../validators'
import { silentRm } from '../fs-helpers'
import { applyAuthToUrl, redactUrl } from './url-auth'
import { readKnownMarketplaces, writeKnownMarketplaces } from './known-store'
import { runGit } from '../lib/git-async'

export interface CloneInput {
  name: string
  rawInput: string
  auth?: { username?: string; token?: string } | null
}

export interface CloneResult {
  name: string
  source: unknown
  installLocation: string
  lastUpdated: string
  isAnthropicOfficial: boolean
}

export async function cloneMarketplace({ name, rawInput, auth }: CloneInput): Promise<CloneResult> {
  assertValidName(name, 'marketplace name')
  if (typeof rawInput !== 'string') throw new Error('Invalid repository input')
  const input = rawInput.trim()
  if (!input) throw new Error('Repository is required')

  // Claude Code stores three source shapes in known_marketplaces.json:
  //   { source: 'git', url }        — arbitrary git URL
  //   { source: 'github', repo }    — shortform owner/repo, preferred when applicable
  //   { source: 'directory', path } — local filesystem
  // We detect the user's input and emit the matching shape so Claude Code
  // can pick the same marketplace up without translation.
  const SHORTFORM = /^([A-Za-z0-9][A-Za-z0-9_.-]*)\/([A-Za-z0-9][A-Za-z0-9_.-]*)$/
  let sourceBlock: { source: 'git'; url: string } | { source: 'github'; repo: string } | { source: 'directory'; path: string }
  let cloneUrl: string | null = null
  let localPath: string | null = null
  if (SHORTFORM.test(input)) {
    sourceBlock = { source: 'github', repo: input }
    cloneUrl = `https://github.com/${input}.git`
  } else if (path.isAbsolute(input) && fs.existsSync(input) && fs.statSync(input).isDirectory()) {
    sourceBlock = { source: 'directory', path: input }
    localPath = input
  } else {
    assertValidGitUrl(input)
    sourceBlock = { source: 'git', url: input }
    cloneUrl = input
  }

  const installLocation = sourceBlock.source === 'directory'
    ? localPath!
    : path.join(os.homedir(), '.claude', 'plugins', 'marketplaces', name)

  // Token handling — only for HTTP(S) clone URLs. SSH / git:// / local
  // directories ignore it. When provided, we bake the token straight into
  // the URL stored in known_marketplaces.json (plaintext), which is the
  // same file Claude Code CLI reads — so CLI `git clone`/`pull` works out
  // of the box too without any credential-helper setup. The file is
  // user-only on POSIX (chmod 600-ish via git) and per-user ACL'd on
  // Windows.
  const token = auth?.token?.trim() || ''
  const username = auth?.username?.trim() || 'oauth2'
  const hasToken = Boolean(token && cloneUrl && /^https?:\/\//i.test(cloneUrl))
  const authedUrl = hasToken && cloneUrl ? applyAuthToUrl(cloneUrl, { username, token }) : cloneUrl
  if (hasToken && sourceBlock.source === 'git') {
    sourceBlock = { source: 'git', url: authedUrl! }
  }
  // github shortform already resolves `https://github.com/owner/repo.git`;
  // we keep the shortform in source but store the authed URL by flipping
  // to a `git` source so CLI clone picks up the token. That's the only way
  // to attach credentials to a github-shortform entry.
  if (hasToken && sourceBlock.source === 'github') {
    sourceBlock = { source: 'git', url: authedUrl! }
  }

  if (cloneUrl) {
    // If a stale dir from a previous failed clone is in the way, git refuses.
    // Remove it only if empty / not a git repo to avoid wiping a live worktree.
    if (fs.existsSync(installLocation)) {
      const gitDir = path.join(installLocation, '.git')
      if (!fs.existsSync(gitDir)) {
        silentRm(installLocation)
      } else {
        throw new Error(`Marketplace "${name}" already cloned at ${installLocation}`)
      }
    }

    try {
      await runGit(['clone', '--', authedUrl!, installLocation], { timeoutMs: 60_000 })
    } catch (err: any) {
      const stderrRaw = typeof err?.stderr === 'string' ? err.stderr : ''
      const stderr = redactUrl(stderrRaw).slice(0, 2000)
      const stdoutRaw = typeof err?.stdout === 'string' ? err.stdout : ''
      const stdout = redactUrl(stdoutRaw).slice(0, 1000)
      let hint = ''
      if (/authentication failed|could not read (Username|Password)|fatal: unable to access/i.test(stderrRaw)) {
        hint = hasToken
          ? 'Token was rejected by the server. Check the token has read access and is not expired.'
          : 'No credentials — provide a personal access token in the Token field, or configure Git Credential Manager.'
      } else if (/terminal prompts disabled/i.test(stderrRaw)) {
        hint = 'Server asked for credentials but none were supplied. Add a token in the form.'
      }
      const message = [
        `git clone failed: ${err?.message || 'unknown error'}`,
        hint && `Hint: ${hint}`,
        stderr && `--- git stderr ---\n${stderr}`,
        stdout && `--- git stdout ---\n${stdout}`,
      ].filter(Boolean).join('\n')
      throw new Error(message)
    }
  }

  const known = readKnownMarketplaces()
  const now = new Date().toISOString()
  known[name] = {
    source: sourceBlock,
    installLocation,
    lastUpdated: now,
  }
  writeKnownMarketplaces(known)

  return {
    name,
    source: sourceBlock,
    installLocation,
    lastUpdated: now,
    isAnthropicOfficial: name.includes('anthropic') || name === 'claude-plugins-official',
  }
}
