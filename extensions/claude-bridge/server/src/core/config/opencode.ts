// ============================================================================
// OpenCode Configuration Manager
// ============================================================================

import { readFile, writeFile, mkdir } from "fs/promises"
import { readFileSync, existsSync } from "fs"
import path from "path"
import os from "os"
import { debugLog } from "../logging"

export interface OpenCodeProviderOptions {
  apiKey: string
  baseURL: string
}

export interface OpenCodeConfig {
  [key: string]: unknown
  provider?: {
    [key: string]: unknown
    anthropic?: {
      [key: string]: unknown
      options?: OpenCodeProviderOptions
    }
  }
}

export interface ConfigCheckResult {
  found: boolean
  path: string
  updated: boolean
  error?: string
}

/**
 * Get the OpenCode config directory path (cross-platform)
 * OpenCode uses xdg-basedir which resolves:
 *   Linux:   $XDG_CONFIG_HOME/opencode  or  ~/.config/opencode
 *   macOS:   ~/Library/Preferences/opencode
 *   Windows: $XDG_CONFIG_HOME/opencode  or  ~/.config/opencode  or  %APPDATA%/opencode
 */
function getConfigDir(): string {
  // XDG_CONFIG_HOME takes priority (all platforms)
  if (process.env.XDG_CONFIG_HOME) {
    return path.join(process.env.XDG_CONFIG_HOME, "opencode")
  }

  const platform = process.platform
  const home = os.homedir()

  if (platform === "darwin") {
    // macOS: ~/Library/Preferences/opencode
    return path.join(home, "Library", "Preferences", "opencode")
  }

  if (platform === "win32") {
    // Windows: check ~/.config/opencode first (common with Git Bash / MSYS2)
    const dotConfig = path.join(home, ".config", "opencode")
    if (existsSync(dotConfig)) {
      return dotConfig
    }
    // Fallback to %APPDATA%/opencode
    if (process.env.APPDATA) {
      return path.join(process.env.APPDATA, "opencode")
    }
    // Last resort
    return path.join(home, ".config", "opencode")
  }

  // Linux/other: ~/.config/opencode
  return path.join(home, ".config", "opencode")
}

/**
 * Get the full path to opencode.json
 */
export function getConfigPath(): string {
  return path.join(getConfigDir(), "opencode.json")
}

/**
 * Read existing OpenCode config, returns null if not found
 */
async function readConfig(configPath: string): Promise<OpenCodeConfig | null> {
  try {
    const content = await readFile(configPath, "utf-8")
    return JSON.parse(content)
  } catch {
    return null
  }
}

/**
 * Write config back to file, preserving formatting
 */
async function writeConfig(configPath: string, config: OpenCodeConfig): Promise<void> {
  const dir = path.dirname(configPath)
  if (!existsSync(dir)) {
    await mkdir(dir, { recursive: true })
  }
  await writeFile(configPath, JSON.stringify(config, null, 2) + "\n", "utf-8")
}

/**
 * Read bridge settings (host/port) from existing opencode.json synchronously.
 * Used at startup to pick up user-configured values before the server starts.
 */
export function readBridgeSettings(): { port?: number; host?: string } {
  try {
    const configPath = getConfigPath()
    if (!existsSync(configPath)) return {}
    const content = readFileSync(configPath, "utf-8")
    const config: OpenCodeConfig = JSON.parse(content)
    const baseURL = config.provider?.anthropic?.options?.baseURL
    if (!baseURL) return {}
    const url = new URL(baseURL)
    return {
      port: url.port ? parseInt(url.port, 10) : undefined,
      host: url.hostname || undefined,
    }
  } catch {
    return {}
  }
}

/**
 * Ensure OpenCode config points to our proxy.
 * Updates only the provider.anthropic.options block, preserves everything else.
 */
export async function ensureOpenCodeConfig(host: string, port: number): Promise<ConfigCheckResult> {
  const configPath = getConfigPath()
  const baseURL = `http://${host}:${port}`
  const result: ConfigCheckResult = {
    found: false,
    path: configPath,
    updated: false,
  }

  try {
    let config = await readConfig(configPath)

    if (config) {
      result.found = true

      // Check if already configured correctly
      const currentBaseURL = config.provider?.anthropic?.options?.baseURL
      const currentApiKey = config.provider?.anthropic?.options?.apiKey
      if (currentBaseURL === baseURL && currentApiKey) {
        debugLog("OpenCode config already up to date", { path: configPath, baseURL })
        result.updated = true
        return result
      }
    } else {
      // Create new config
      config = {
        "$schema": "https://opencode.ai/config.json",
      }
    }

    // Ensure provider.anthropic.options exists
    if (!config.provider) {
      config.provider = {}
    }
    if (!config.provider.anthropic) {
      config.provider.anthropic = {}
    }
    if (!config.provider.anthropic.options) {
      config.provider.anthropic.options = { apiKey: "create-token-at-dashboard", baseURL: "" }
    }

    // Update only baseURL and ensure apiKey exists
    config.provider.anthropic.options.baseURL = baseURL
    if (!config.provider.anthropic.options.apiKey) {
      config.provider.anthropic.options.apiKey = "dummy"
    }

    await writeConfig(configPath, config)
    debugLog("OpenCode config updated", { path: configPath, baseURL })
    result.found = true
    result.updated = true
    return result
  } catch (error) {
    result.error = error instanceof Error ? error.message : "Unknown error"
    debugLog("OpenCode config error", { path: configPath, error: result.error })
    return result
  }
}
