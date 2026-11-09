import type { JSX } from 'preact'
import { copyWithToast } from '../../../lib/clipboard'
import { Button } from '../../ui/Button'

/** Container files are already readable in the detail panel; expose their path. */
export function ContainerPathButton({ path }: { path: string }): JSX.Element {
  return <Button icon="fa-copy" title={path} onClick={() => void copyWithToast(path, 'Container path copied')}>
    Copy container path
  </Button>
}
