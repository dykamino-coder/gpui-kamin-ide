import type { JSX } from 'preact'
import { renderMarkdown } from '../../../utils/render-markdown'
import styles from './SkillContent.module.css'

interface Props {
  content: string
}

export function SkillContent({ content }: Props): JSX.Element {
  return (
    <div
      class={styles.content}
      dangerouslySetInnerHTML={{ __html: renderMarkdown(content) }}
    />
  )
}
