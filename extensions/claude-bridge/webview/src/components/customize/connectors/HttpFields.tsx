import type { JSX } from 'preact'
import { Field } from '../../ui/Field'
import styles from './HttpFields.module.css'

interface Props {
  url: string
  headers: string
  onUrlChange: (v: string) => void
  onHeadersChange: (v: string) => void
}

export function HttpFields({ url, headers, onUrlChange, onHeadersChange }: Props): JSX.Element {
  return (
    <>
      <Field label="URL" required>
        <input
          class={styles.input}
          type="text"
          placeholder="https://api.example.com/mcp"
          value={url}
          onInput={(e) => onUrlChange((e.target as HTMLInputElement).value)}
        />
      </Field>
      <Field label="Headers (JSON, optional)">
        <input
          class={styles.input}
          type="text"
          placeholder='{"Authorization": "Bearer ..."}'
          value={headers}
          onInput={(e) => onHeadersChange((e.target as HTMLInputElement).value)}
        />
      </Field>
    </>
  )
}
