import fs from 'fs'
import os from 'os'
import path from 'path'
import { readKnownMarketplaces, writeKnownMarketplaces } from './known-store'

export function removeMarketplace(name: string): void {
  const known = readKnownMarketplaces() as any
  const entry = known[name]
  if (entry) {
    // A directory marketplace is the user's own folder, used in place: forget
    // it, never delete it. Only a clone made by addMarketplace is ours to remove.
    const ownsFiles = entry.source?.source !== 'directory'
    if (ownsFiles && entry.installLocation && fs.existsSync(entry.installLocation)) {
      fs.rmSync(entry.installLocation, { recursive: true, force: true })
    }
    delete known[name]
    writeKnownMarketplaces(known)
  }

  // Remove installed plugins from this marketplace
  const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
  if (fs.existsSync(installedFile)) {
    try {
      const data = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
      if (data.plugins) {
        for (const key of Object.keys(data.plugins)) {
          if (key.endsWith(`@${name}`)) {
            const pluginEntry = data.plugins[key][0]
            if (pluginEntry?.installPath && fs.existsSync(pluginEntry.installPath)) {
              fs.rmSync(pluginEntry.installPath, { recursive: true, force: true })
            }
            delete data.plugins[key]
          }
        }
        fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8')
      }
    } catch {}
  }
}
