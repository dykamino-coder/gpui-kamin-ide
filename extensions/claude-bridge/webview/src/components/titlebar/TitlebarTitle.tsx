import type { JSX } from 'preact'
import styles from './TitlebarTitle.module.css'

interface TitlebarTitleProps {
  text: string
}

export function TitlebarTitle({ text }: TitlebarTitleProps): JSX.Element {
  return (
    <span class={styles.title}>{text}</span>
  )
}
