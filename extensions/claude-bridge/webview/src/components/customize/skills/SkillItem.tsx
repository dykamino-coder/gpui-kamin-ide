import type { JSX } from 'preact'
import type { SkillItem as SkillItemType } from '../../../signals/customize'
import styles from './SkillItem.module.css'

interface Props {
  skill: SkillItemType
  isActive: boolean
  onClick: () => void
}

export function SkillItem({ skill, isActive, onClick }: Props): JSX.Element {
  // Distinct icons so skills (magic wand) don't visually collide with plugin
  // commands (terminal). `project` = bundled skill, `user` = personal skill,
  // plugin-sourced skills fall through to the wand too.
  const icon = skill.source === 'project'
    ? 'fa-wand-magic-sparkles'
    : skill.source === 'user'
      ? 'fa-bolt'
      : 'fa-wand-magic-sparkles'

  return (
    <div
      class={`${styles.item} ${isActive ? styles.active : ''}`}
      onClick={onClick}
    >
      <div class={styles.icon}><i class={`fas ${icon}`} /></div>
      <div class={styles.info}>
        <div class={styles.name}>/{skill.name}</div>
        <div class={styles.desc}>{skill.description}</div>
      </div>
    </div>
  )
}
