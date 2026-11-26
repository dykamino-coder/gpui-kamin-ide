// Open user-clicked text links in the native editor of the current chat.
import { realpath, stat } from 'node:fs/promises'
import path from 'node:path'

export function parseWorkspaceLocation(destination: string) {
  if (typeof destination !== 'string' || destination.length > 4096 || /[\r\n]/.test(destination)) return null
  const url = /^file:/i.test(destination)
  if (url && !/^file:\/\/\/[a-z]:\//i.test(destination)) return null
  const value = url ? destination.slice(8) : destination.replace(/^\/(?=[a-z]:\/)/i, '')
  const match = /^(.*?)(?:#L(\d+)(?:C(\d+))?|:(\d+)(?::(\d+))?)?$/i.exec(value)
  if (!match) return null
  let file = match[1]!
  if (url) {
    try { file = decodeURIComponent(file) } catch { return null }
  }
  const line = Number(match[2] ?? match[4] ?? 1)
  const column = Number(match[3] ?? match[5] ?? 1)
  if (!/^[a-z]:[\\/]/i.test(file) || /[<>:"|?*]/.test(file.slice(2))) return null
  if ([...file].some(c => c.charCodeAt(0) < 32)) return null
  if (![line, column].every(n => Number.isSafeInteger(n) && n > 0 && n <= 0xffffffff)) return null
  return { path: file, line, column }
}

function contained(root: string, target: string): boolean {
  const relative = path.relative(root, target)
  return !!relative && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative)
}

export async function projectTextFile(root: string, file: string) {
  if (!path.isAbsolute(root) || !path.isAbsolute(file) || !contained(root, file)) return null
  try {
    const [project, target] = await Promise.all([realpath(root), realpath(file)])
    return contained(project, target) && (await stat(target)).isFile() ? target : null
  } catch { return null }
}

export async function workspaceLocation(root: string, destination: string) {
  if (process.platform !== 'win32') return null
  const location = parseWorkspaceLocation(destination)
  if (!location) return null
  const target = await projectTextFile(root, location.path)
  return target ? { ...location, path: target } : null
}
