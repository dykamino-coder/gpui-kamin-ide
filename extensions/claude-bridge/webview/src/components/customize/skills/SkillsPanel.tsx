import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { readPanelCache, writePanelCache } from '../../../lib/panel-cache'
import { ListSkeleton } from '../../ui/ListSkeleton'
import { PanelError } from '../../ui/PanelError'
import type { SkillItem } from '../../../signals/customize'
import { ThreeColumnPanel } from '../shared/ThreeColumnPanel'
import { ThreeColumnList } from '../shared/ThreeColumnList'
import { ThreeColumnDetail } from '../shared/ThreeColumnDetail'
import { ThreeColumnListFooter } from '../shared/ThreeColumnListFooter'
import { SkillsList } from './SkillsList'
import { SkillDetail } from './SkillDetail'
import { SkillDetailEmpty } from './SkillDetailEmpty'
import { AddSkillForm } from './AddSkillForm'

type DetailView = 'empty' | 'detail' | 'add'

export function SkillsPanel(): JSX.Element {
  const bridge = useBridge()
  const [skills, setSkills] = useState<SkillItem[]>([])
  const [filter, setFilter] = useState('')
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [view, setView] = useState<DetailView>('empty')
  // `loading` distinguishes the cold-open fetch from a genuinely empty install
  // (the list would otherwise show "No skills found" while still loading).
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState(false)

  async function refresh(): Promise<void> {
    setLoadError(false)
    try {
      const result = await bridge.listSkills()
      // The runtime result includes 'source' field even though the type definition omits it
      const list = (result ?? []) as unknown as SkillItem[]
      setSkills(list)
      writePanelCache('skills.list.v1', list)
    } catch {
      setLoadError(true)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    // Cache-first: paint the last-known list instantly (clears loading), then
    // revalidate. Only cold opens (no cache) show the skeleton.
    const cached = readPanelCache<SkillItem[]>('skills.list.v1')
    if (cached) { setSkills(cached); setLoading(false) }
    void refresh()
  }, [bridge])

  function handleSelect(path: string): void {
    setSelectedPath(path)
    setView('detail')
  }

  function handleAddClick(): void {
    setSelectedPath(null)
    setView('add')
  }

  function handleFormCancel(): void {
    setView('empty')
  }

  async function handleFormSaved(): Promise<void> {
    await refresh()
    setView('empty')
  }

  async function handleDeleted(): Promise<void> {
    setSelectedPath(null)
    setView('empty')
    await refresh()
  }

  const selectedSkill = skills.find((s) => s.path === selectedPath) ?? null

  return (
    <ThreeColumnPanel>
      <ThreeColumnList
        title="Skills"
        searchValue={filter}
        onSearchChange={setFilter}
        searchPlaceholder="Search skills..."
        footer={
          <ThreeColumnListFooter label="New skill" onClick={handleAddClick} />
        }
      >
        {loading && skills.length === 0 ? (
          <ListSkeleton rows={7} />
        ) : loadError && skills.length === 0 ? (
          <PanelError message="Could not load skills." onRetry={() => { setLoading(true); void refresh() }} />
        ) : (
          <SkillsList
            skills={skills}
            filter={filter}
            selectedPath={selectedPath}
            onSelect={handleSelect}
          />
        )}
      </ThreeColumnList>

      <ThreeColumnDetail>
        {view === 'empty' && <SkillDetailEmpty />}
        {view === 'detail' && selectedSkill && (
          <SkillDetail skill={selectedSkill} onDeleted={handleDeleted} />
        )}
        {view === 'add' && (
          <AddSkillForm onCancel={handleFormCancel} onSaved={handleFormSaved} />
        )}
      </ThreeColumnDetail>
    </ThreeColumnPanel>
  )
}
