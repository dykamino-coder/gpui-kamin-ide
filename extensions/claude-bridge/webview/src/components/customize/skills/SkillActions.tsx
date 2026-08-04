import type { JSX } from 'preact'
import { useBridge } from '../../../hooks/useBridge'
import styles from './SkillActions.module.css'

interface Props {
  path: string
  fileName: string
  canDelete: boolean
  onDeleted: () => void
}

export function SkillActions({ path, fileName, canDelete, onDeleted }: Props): JSX.Element {
  const bridge = useBridge()

  async function handleDelete(): Promise<void> {
    await bridge.deleteSkill(fileName)
    onDeleted()
  }

  return (
    <div class={styles.actions}>
      <button class={styles.primary} onClick={() => bridge.openSkill(path)}>
        Open in editor
      </button>
      <button class={styles.secondary} onClick={() => bridge.showSkillInFolder(path)}>
        <i class="fas fa-folder-open" /> Show in Explorer
      </button>
      {canDelete && (
        <button class={styles.danger} onClick={handleDelete}>Delete</button>
      )}
    </div>
  )
}
