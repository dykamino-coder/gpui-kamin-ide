// Хостовые черновики подключаются только к браузерной оболочке общего composer.
import { signal } from '@preact/signals'
import { inv, sub } from '../lib/bridge-transport'
import { vscodeApi } from '../lib/webview-api'
import { installInputDraftPort } from '../lib/input-draft-port'
import { WorkspaceDraftClient } from '../lib/workspace-draft-client'
import type { WorkspaceDraftReply, WorkspaceDraftState } from '../../shared/workspace-draft'
import { activeTabId } from '../signals/tabs'

const revision = signal(0)
const client = new WorkspaceDraftClient({
  get: id => inv('workspace:draft:get', id) as Promise<WorkspaceDraftState>,
  update: (id, base, value) => inv('workspace:draft:update', id, base, value) as Promise<WorkspaceDraftReply>,
}, () => { revision.value++ })

export function connectDrafts(): void {
  installInputDraftPort({ watch: (id, apply) => client.watch(id, apply), save: (id, value) => client.save(id, value),
    ready(id) { revision.value; return ['saved', 'saving'].includes(client.status(id)) } })
  sub('workspace:draft:state', (id, state) => client.observe(id as string, state as WorkspaceDraftState))
  sub('bridge:reconnected', () => client.refresh())
  sub('tab:list-changed', list => client.retain((list as { id: string }[]).map(tab => tab.id)))
  vscodeApi.onDisconnect?.(() => client.disconnected())
}

export function DraftNotice() {
  revision.value
  const id = activeTabId.value
  if (!id) return null
  const status = client.status(id)
  const text = { loading: 'Loading draft…', saved: 'Draft saved', saving: 'Saving draft…',
    conflict: 'Draft changed in another page. Your local text is still here.',
    unsaved: 'Draft could not be saved. Keep this page open.' }[status]
  return <div role="status" data-workspace-draft={status} class="ws-draft-status">{text}
    {status === 'conflict' && <><button onClick={() => client.resolve(id, false)}>Use workspace draft</button>
      <button onClick={() => client.resolve(id, true)}>Keep this draft</button></>}
    {status === 'unsaved' && <button onClick={() => client.refresh()}>Check saved draft</button>}
  </div>
}
