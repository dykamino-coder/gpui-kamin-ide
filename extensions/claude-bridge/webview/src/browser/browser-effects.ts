import type { WebviewApi } from '../lib/webview-api'

interface Effect { id: string; operation: string; payload: Record<string, unknown> }

/** Browser primitives only. Host paths, exports and attachment policy stay in Node. */
export function installBrowserEffects(api: WebviewApi): void {
  const dialogs = new Set<HTMLDialogElement>()
  const dispose = () => { for (const dialog of dialogs) dialog.close(); dialogs.clear() }
  api.onDisconnect?.(dispose)
  api.onHostMessage?.(frame => {
    const event = frame as { kind?: string; channel?: string; args?: unknown[] }
    if (event?.kind !== 'event' || event.channel !== 'workspace:effect') return
    const effect = event.args?.[0] as Effect | undefined
    if (!effect || typeof effect.id !== 'string' || !effect.payload || dialogs.size >= 32) return
    void render(effect).then(value => reply(effect.id, value), error => reply(effect.id, undefined, error instanceof Error ? error.message : 'Browser effect failed'))
  })
  function reply(id: string, value: unknown, error?: string) {
    try { api.postMessage({ kind: 'send', channel: 'workspace:effect-result', args: error === undefined ? [id, value] : [id, value, error] }) }
    catch { /* A detached view cannot complete an effect. Host rejects on detach. */ }
  }
  async function render({ operation, payload }: Effect): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const dialog = document.createElement('dialog')
      dialog.setAttribute('aria-label', 'Bridge workspace action')
      const title = document.createElement('p')
      title.textContent = operation === 'message' ? String(payload.text ?? '') : ({
        upload: 'Upload attachments to the workspace', download: `Download ${String(payload.name)}`,
        'clipboard-read': 'Paste from your clipboard', 'clipboard-write': 'Copy to your clipboard',
        'container-folder': 'Choose a container directory', 'open-external': 'Open external link',
      } as Record<string, string>)[operation] ?? 'Workspace action'
      dialog.append(title)
      let completed = false
      function finish(value: unknown, error?: unknown) {
        if (completed) return
        completed = true
        dialogs.delete(dialog)
        dialog.remove()
        if (error) reject(error)
        else resolve(value)
      }
      function button(label: string, action: () => unknown | Promise<unknown>) {
        const control = document.createElement('button')
        control.type = 'button'
        control.textContent = label
        control.addEventListener('click', () => {
          control.disabled = true
          try { Promise.resolve(action()).then(value => finish(value), error => finish(undefined, error)) }
          catch (error) { finish(undefined, error) }
        })
        dialog.append(control)
      }
      if (operation === 'message') {
        for (const action of Array.isArray(payload.actions) ? payload.actions : []) button(String(action), () => action)
      } else if (operation === 'clipboard-read') {
        button('Paste', () => navigator.clipboard.readText())
      } else if (operation === 'clipboard-write') {
        button('Copy', () => navigator.clipboard.writeText(String(payload.text ?? '')))
      } else if (operation === 'container-folder') {
        const input = document.createElement('input')
        input.setAttribute('aria-label', 'Absolute container directory')
        input.value = String(payload.defaultPath ?? '/workspace')
        dialog.append(input)
        button('Select', () => input.value)
      } else if (operation === 'upload') {
        const input = document.createElement('input')
        input.type = 'file'
        input.multiple = payload.multiple === true
        input.setAttribute('aria-label', 'Attachments')
        if (payload.filters && typeof payload.filters === 'object') {
          input.accept = Object.values(payload.filters).flat().filter((ext): ext is string => typeof ext === 'string').map(ext => `.${ext}`).join(',')
        }
        dialog.append(input)
        button('Upload', async () => {
          const files = [...(input.files ?? [])]
          if (!files.length) return null
          const results = []
          let total = 0
          for (const file of files) {
            total += file.size
            // The whole result must also fit the bounded WebSocket frame.
            if (total > Number(payload.maxBytes)) throw new Error('Upload selection exceeds 8 MiB')
            const base64 = await new Promise<string>((done, fail) => {
              const reader = new FileReader()
              reader.onload = () => done(String(reader.result).split(',')[1] ?? '')
              reader.onerror = () => fail(new Error('Cannot read selected file'))
              reader.readAsDataURL(file)
            })
            results.push({ name: file.name, base64 })
          }
          return results
        })
      } else if (operation === 'download') {
        button('Download', () => {
          const bytes = Uint8Array.from(atob(String(payload.base64)), character => character.charCodeAt(0))
          const url = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }))
          const link = document.createElement('a')
          link.download = String(payload.name)
          link.href = url
          link.click()
          setTimeout(() => URL.revokeObjectURL(url), 30_000)
          return true
        })
      } else if (operation === 'open-external') {
        const link = document.createElement('a')
        link.href = String(payload.url)
        link.textContent = String(payload.url)
        link.target = '_blank'
        link.rel = 'noopener noreferrer'
        link.addEventListener('click', () => finish(true))
        dialog.append(link)
      } else { finish(undefined, new Error('Unknown browser effect')); return }
      button(operation === 'message' ? 'Dismiss' : 'Cancel', () => {
        if (['upload', 'container-folder', 'message', 'open-external'].includes(operation)) return null
        throw new Error('Browser action cancelled')
      })
      dialog.addEventListener('cancel', event => { event.preventDefault(); finish(undefined, new Error('Browser action cancelled')) })
      dialog.addEventListener('close', () => finish(undefined, new Error('Browser connection closed')))
      dialogs.add(dialog)
      document.body.append(dialog)
      dialog.showModal()
    })
  }
}
