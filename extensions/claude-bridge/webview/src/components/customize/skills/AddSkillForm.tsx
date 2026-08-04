import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { Field } from '../../ui/Field'
import { Button } from '../../ui/Button'
import styles from './AddSkillForm.module.css'

interface Props {
  onCancel: () => void
  onSaved: () => void
}

export function AddSkillForm({ onCancel, onSaved }: Props): JSX.Element {
  const bridge = useBridge()
  const [name, setName] = useState('')
  const [skillContent, setSkillContent] = useState('')

  async function handleCreate(): Promise<void> {
    if (!name || !skillContent) return
    await bridge.createSkill(name, skillContent)
    setName('')
    setSkillContent('')
    onSaved()
  }

  return (
    <div class={styles.form}>
      <div class={styles.header}>
        <h2>New Skill</h2>
      </div>
      <div class={styles.section}>
        <Field label="Skill name (used as /command)" required>
          <input
            class={styles.input}
            type="text"
            placeholder="e.g. review-code"
            value={name}
            onInput={(e) => setName((e.target as HTMLInputElement).value)}
          />
        </Field>
        <Field label="Instructions (markdown)" required>
          <textarea
            class={styles.textarea}
            rows={6}
            placeholder="Describe what this skill should do..."
            value={skillContent}
            onInput={(e) => setSkillContent((e.target as HTMLTextAreaElement).value)}
          />
        </Field>
        <div class={styles.buttons}>
          <Button variant="primary" class={styles.grow} onClick={handleCreate}>Create</Button>
          <Button variant="secondary" class={styles.grow} onClick={onCancel}>Cancel</Button>
        </div>
      </div>
    </div>
  )
}
