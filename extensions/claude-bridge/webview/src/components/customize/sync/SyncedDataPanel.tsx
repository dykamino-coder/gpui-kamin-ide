import type { JSX } from 'preact'
import { SyncTree } from './SyncTree'
import { SyncPathRow } from './SyncPathRow'
import { SyncFileViewer } from './SyncFileViewer'
import { useSyncedData } from './useSyncedData'

export function SyncedDataPanel(): JSX.Element {
  const {
    data, loading, error, selected, fileData, fileLoading, fileError, hasAnything,
    load, forceSyncAndLoad, openFile,
  } = useSyncedData()

  return (
    <div style="flex:1;display:flex;flex-direction:column;min-height:0;padding:16px;gap:12px;overflow:hidden">
      <div style="display:flex;align-items:center;gap:10px">
        <h2 style="margin:0;font-size: var(--fs-md);color:var(--text-primary);font-weight:600;flex:1">
          <i class="fas fa-cloud-arrow-up" style="margin-right:8px;color:var(--accent-primary)" />
          Synced data on server
        </h2>
        <button
          type="button"
          onClick={forceSyncAndLoad}
          disabled={loading}
          style="background:var(--bg-surface);border:1px solid var(--bg-overlay);color:var(--accent-green);padding:4px 12px;border-radius:var(--radius-sm);font-size:12px;cursor:pointer"
        >
          <i class="fas fa-upload" style="margin-right:6px" />
          Force sync now
        </button>
        <button
          type="button"
          onClick={load}
          disabled={loading}
          style="background:var(--bg-surface);border:1px solid var(--bg-overlay);color:var(--text-primary);padding:4px 12px;border-radius:var(--radius-sm);font-size:12px;cursor:pointer"
        >
          <i class={loading ? 'fas fa-spinner fa-spin' : 'fas fa-rotate'} style="margin-right:6px" />
          Reload
        </button>
      </div>

      {error && (
        <div style="padding:10px 14px;background:var(--tint-red-soft-2);border:1px solid var(--tint-red-border-strong);border-radius:var(--radius-sm);color:var(--accent-red);font-size:12px">
          <i class="fas fa-triangle-exclamation" style="margin-right:8px" />{error}
        </div>
      )}

      {data && (
        <div>
          <SyncPathRow label="Sync base" value={data.basePath} />
          <SyncPathRow label="Token dir" value={`users/${data.tokenId}`} />
        </div>
      )}

      <div style="flex:1;display:flex;gap:12px;min-height:0;overflow:hidden">
        <div style="flex:0 0 42%;max-width:520px;display:flex;flex-direction:column;min-height:0;overflow:auto;padding:8px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm)">
          {loading && !data && <div style="color:var(--text-muted);font-size:12px">Loading…</div>}
          {data && !hasAnything && (
            <div style="color:var(--text-muted);font-size:12px;padding:8px 2px">Empty — nothing synced yet</div>
          )}
          {data && data.user.tree.length > 0 && (
            <div style="margin-bottom:12px">
              <div style="display:flex;align-items:center;gap:6px;margin-bottom:4px">
                <i class="fas fa-user" style="color:var(--accent-green);font-size:10px" />
                <span style="color:var(--text-primary);font-weight:600;font-size:12px">User-level</span>
              </div>
              <SyncTree
                nodes={data.user.tree}
                depth={0}
                rootAbs={data.user.path}
                rootRel={`users/${data.tokenId}`}
                onSelect={openFile}
                selectedAbs={selected?.absPath ?? null}
              />
            </div>
          )}
          {data && data.projects.map(p => (
            <div key={p.path} style="margin-bottom:12px">
              <div style="display:flex;align-items:center;gap:6px;margin-bottom:4px">
                <i class="fas fa-folder" style="color:var(--accent-orange);font-size:10px" />
                <span style="color:var(--text-primary);font-weight:600;font-size:12px" title={p.projectPath}>
                  {p.projectPath.split(/[\\/]/).slice(-2).join('/')}
                </span>
              </div>
              <SyncTree
                nodes={p.tree}
                depth={0}
                rootAbs={p.path}
                rootRel={''}
                onSelect={openFile}
                selectedAbs={selected?.absPath ?? null}
              />
            </div>
          ))}
        </div>

        <div style="flex:1;display:flex;flex-direction:column;min-height:0;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);overflow:hidden">
          <SyncFileViewer
            selected={selected}
            fileData={fileData}
            fileLoading={fileLoading}
            fileError={fileError}
          />
        </div>
      </div>
    </div>
  )
}
