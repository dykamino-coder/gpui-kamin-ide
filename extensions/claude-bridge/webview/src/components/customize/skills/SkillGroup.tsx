import type { JSX, ComponentChildren } from 'preact'
import styles from './SkillGroup.module.css'

interface Props {
  title: string
  children: ComponentChildren
}

export function SkillGroup({ title, children }: Props): JSX.Element {
  return (
    <>
      <div class={styles.header}>{title}</div>
      {children}
    </>
  )
}
