import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

/** Shared by native and browser hosts; callers choose the upload location. */
export async function saveAttachment(base64: string, name: string, root = os.tmpdir()): Promise<string> {
  if (typeof base64 !== 'string' || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(base64)) {
    throw new Error('Invalid attachment encoding')
  }
  const safe = String(name || 'attachment').replace(/[/\\\x00-\x1f<>:"|?*]/g, '_').slice(-120)
  await fs.mkdir(root, { recursive: true })
  const directory = await fs.mkdtemp(path.join(root, 'bridge-upload-'))
  const file = path.join(directory, safe === '.' || safe === '..' ? 'attachment' : safe)
  await fs.writeFile(file, Buffer.from(base64, 'base64'), { flag: 'wx', mode: 0o600 })
  return file
}
