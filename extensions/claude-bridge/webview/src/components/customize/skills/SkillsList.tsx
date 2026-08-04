import type { JSX } from 'preact'
import type { SkillItem } from '../../../signals/customize'
import { prettifyPluginName } from '../../../utils/prettify-plugin-name'
import { SkillGroup } from './SkillGroup'
import { SkillItem as SkillItemComponent } from './SkillItem'
import styles from './SkillsList.module.css'

interface Props {
  skills: SkillItem[]
  filter: string
  selectedPath: string | null
  onSelect: (path: string) => void
}

export function SkillsList({ skills, filter, selectedPath, onSelect }: Props): JSX.Element {
  const q = filter.toLowerCase()
  const filtered = skills.filter(
    (s) => !q || s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)
  )

  const groups: Record<string, SkillItem[]> = {}
  for (const s of filtered) {
    let group: string
    if (s.source === 'project') {
      group = 'Project commands'
    } else if (s.source === 'user') {
      group = 'User skills'
    } else if (s.source.startsWith('plugin:')) {
      group = prettifyPluginName(s.source.replace('plugin:', ''))
    } else {
      group = 'Other'
    }
    if (!groups[group]) groups[group] = []
    groups[group].push(s)
  }

  if (filtered.length === 0) {
    return <div class={styles.empty}>No skills found</div>
  }

  return (
    <>
      {Object.entries(groups).map(([group, items]) => (
        <SkillGroup key={group} title={group}>
          {items.map((s) => (
            <SkillItemComponent
              key={s.path}
              skill={s}
              isActive={selectedPath === s.path}
              onClick={() => onSelect(s.path)}
            />
          ))}
        </SkillGroup>
      ))}
    </>
  )
}
