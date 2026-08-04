import type { JSX } from 'preact'
import { Field } from '../../ui/Field'
import styles from './StdioFields.module.css'

interface Props {
  command: string
  args: string
  env: string
  onCommandChange: (v: string) => void
  onArgsChange: (v: string) => void
  onEnvChange: (v: string) => void
}

export function StdioFields({ command, args, env, onCommandChange, onArgsChange, onEnvChange }: Props): JSX.Element {
  return (
    <>
      <Field label="Command" required>
        <input
          class={styles.input}
          type="text"
          placeholder="npx -y @modelcontextprotocol/server-github"
          value={command}
          onInput={(e) => onCommandChange((e.target as HTMLInputElement).value)}
        />
      </Field>
      <Field label="Args (comma-separated, optional)">
        <input
          class={styles.input}
          type="text"
          placeholder="-y, @upstash/context7-mcp"
          value={args}
          onInput={(e) => onArgsChange((e.target as HTMLInputElement).value)}
        />
      </Field>
      <Field label="Env vars (JSON, optional)">
        <input
          class={styles.input}
          type="text"
          placeholder='{"GITHUB_TOKEN": "ghp_..."}'
          value={env}
          onInput={(e) => onEnvChange((e.target as HTMLInputElement).value)}
        />
      </Field>
    </>
  )
}
