import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import type { ExternalMcpServerInfo } from '../../../../shared/types'
import { ThreeColumnPanel } from '../shared/ThreeColumnPanel'
import { ThreeColumnList } from '../shared/ThreeColumnList'
import { ThreeColumnDetail } from '../shared/ThreeColumnDetail'
import { ThreeColumnListFooter } from '../shared/ThreeColumnListFooter'
import { ConnectorsList } from './ConnectorsList'
import { ConnectorDetail } from './ConnectorDetail'
import { ConnectorDetailEmpty } from './ConnectorDetailEmpty'
import { AddServerForm } from './AddServerForm'
import { ListSkeleton } from '../../ui/ListSkeleton'
import { PanelError } from '../../ui/PanelError'

type DetailView = 'empty' | 'detail' | 'add' | 'edit'

export function ConnectorsPanel(): JSX.Element {
  const bridge = useBridge()
  const [servers, setServers] = useState<ExternalMcpServerInfo[]>([])
  const [filter, setFilter] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [view, setView] = useState<DetailView>('empty')
  // Distinguish the initial fetch from a genuinely server-less state (the list
  // otherwise shows only the Built-in group with no "still loading" cue).
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  function loadServers(): void {
    setLoadError(false)
    bridge.getMcpServers()
      .then((s) => { setServers(s ?? []); setLoading(false) })
      .catch(() => { setLoadError(true); setLoading(false) })
  }

  useEffect(() => {
    // getMcpServers maps to an as-yet-unimplemented bridge channel (external-MCP
    // manager deferred) → can resolve null; coalesce so `servers.find` is safe.
    loadServers()
    return bridge.onMcpServersChanged((updated) => {
      setServers(updated ?? [])
      setLoading(false)
      setLoadError(false) // a fresh push supersedes any earlier fetch failure
    })
  }, [bridge])

  function handleSelect(id: string): void {
    setSelectedId(id)
    setView('detail')
  }

  function handleAddClick(): void {
    setSelectedId(null)
    setView('add')
  }

  function handleFormCancel(): void {
    setView(view === 'edit' && selectedId ? 'detail' : 'empty')
  }

  function handleEditClick(): void {
    // Selection already points at the server; just flip the detail column into
    // the pre-filled edit form.
    setView('edit')
  }

  function handleFormSaved(): void {
    // After an edit, jump back to the (updated) detail view for that server;
    // after an add, return to the empty state.
    setView(view === 'edit' && selectedId ? 'detail' : 'empty')
    bridge.getMcpServers().then((s) => setServers(s ?? [])).catch(() => {})
  }

  function handleRemoved(): void {
    setSelectedId(null)
    setView('empty')
    bridge.getMcpServers().then((s) => setServers(s ?? [])).catch(() => {})
  }

  const selectedServer = selectedId === '__builtin'
    ? '__builtin' as const
    : servers.find((s) => s.id === selectedId) ?? null

  return (
    <ThreeColumnPanel>
      <ThreeColumnList
        title="Connectors"
        searchValue={filter}
        onSearchChange={setFilter}
        searchPlaceholder="Search connectors..."
        footer={
          <ThreeColumnListFooter label="Add connector" onClick={handleAddClick} />
        }
      >
        {loading && servers.length === 0 ? (
          <ListSkeleton rows={6} />
        ) : loadError && servers.length === 0 ? (
          <PanelError message="Could not load connectors." onRetry={() => { setLoading(true); loadServers() }} />
        ) : (
          <ConnectorsList
            servers={servers}
            filter={filter}
            selectedId={selectedId}
            onSelect={handleSelect}
          />
        )}
      </ThreeColumnList>

      <ThreeColumnDetail>
        {view === 'empty' && <ConnectorDetailEmpty />}
        {view === 'detail' && selectedServer && (
          <ConnectorDetail
            key={selectedServer === '__builtin' ? '__builtin' : selectedServer.id}
            server={selectedServer}
            onRemoved={handleRemoved}
            onEdit={handleEditClick}
          />
        )}
        {view === 'add' && (
          <AddServerForm onCancel={handleFormCancel} onSaved={handleFormSaved} />
        )}
        {view === 'edit' && selectedServer && selectedServer !== '__builtin' && (
          <AddServerForm
            key={`edit-${selectedServer.id}`}
            initial={selectedServer}
            onCancel={handleFormCancel}
            onSaved={handleFormSaved}
          />
        )}
      </ThreeColumnDetail>
    </ThreeColumnPanel>
  )
}
