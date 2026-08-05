import fs from 'fs'
import path from 'path'

function copyOverlay(source: string, destination: string): void {
  fs.mkdirSync(destination, { recursive: true })
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const sourcePath = path.join(source, entry.name)
    const destinationPath = path.join(destination, entry.name)
    if (entry.isDirectory()) {
      try {
        if (fs.existsSync(destinationPath) && !fs.statSync(destinationPath).isDirectory()) {
          fs.rmSync(destinationPath, { force: true })
        }
      } catch {
        fs.rmSync(destinationPath, { recursive: true, force: true })
      }
      copyOverlay(sourcePath, destinationPath)
    } else {
      try {
        if (fs.existsSync(destinationPath) && fs.statSync(destinationPath).isDirectory()) {
          fs.rmSync(destinationPath, { recursive: true, force: true })
        }
      } catch {
        fs.rmSync(destinationPath, { recursive: true, force: true })
      }
      fs.mkdirSync(path.dirname(destinationPath), { recursive: true })
      fs.copyFileSync(sourcePath, destinationPath)
    }
  }
}

/** Build an exact user + project skills snapshot in a staging directory and
 * replace the session-local tree only after the overlay is complete. */
export function replaceSkillsOverlay(destination: string, userSource: string, projectSource?: string | null): void {
  fs.mkdirSync(path.dirname(destination), { recursive: true })
  const staging = fs.mkdtempSync(`${destination}.staging-`)
  let installed = false
  try {
    if (fs.existsSync(userSource)) {
      copyOverlay(userSource, staging)
      installed = true
    }
    if (projectSource && fs.existsSync(projectSource)) {
      copyOverlay(projectSource, staging)
      installed = true
    }

    fs.rmSync(destination, { recursive: true, force: true })
    if (installed) fs.renameSync(staging, destination)
    else fs.rmSync(staging, { recursive: true, force: true })
  } catch (err) {
    fs.rmSync(staging, { recursive: true, force: true })
    throw err
  }
}
