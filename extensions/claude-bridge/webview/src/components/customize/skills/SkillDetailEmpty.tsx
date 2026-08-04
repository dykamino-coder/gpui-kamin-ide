import type { JSX } from 'preact'
import styles from './SkillDetailEmpty.module.css'

export function SkillDetailEmpty(): JSX.Element {
  return (
    <div class={styles.empty}>
      Select a skill to view details
    </div>
  )
}
