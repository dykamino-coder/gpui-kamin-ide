import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import type { SkillItem } from '../../../signals/customize'
import { prettifyPluginName } from '../../../utils/prettify-plugin-name'
import { SkillActions } from './SkillActions'
import { SkillContent } from './SkillContent'
import styles from './SkillDetail.module.css'

interface Props {
  skill: SkillItem
  onDeleted: () => void
}

export function SkillDetail({ skill, onDeleted }: Props): JSX.Element {
  const bridge = useBridge()
  const [content, setContent] = useState<string | null>(null)

  useEffect(() => {
    setContent(null)
    bridge.readSkill(skill.path).then(setContent)
  }, [skill.path, bridge])

  const sourceBadge = skill.source === 'project'
    ? 'Project'
    : skill.source === 'user'
      ? 'User'
      : prettifyPluginName(skill.source.replace('plugin:', ''))

  const canDelete = skill.source === 'project'

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <h2>/{skill.name}</h2>
        <div class={styles.meta}>
          <span class={styles.badge}>{sourceBadge}</span>
          <span>{skill.fileName}</span>
        </div>
      </div>

      <p class={styles.description}>{skill.description}</p>

      <SkillActions
        path={skill.path}
        fileName={skill.fileName}
        canDelete={canDelete}
        onDeleted={onDeleted}
      />

      {content && <SkillContent content={content} />}
    </div>
  )
}
