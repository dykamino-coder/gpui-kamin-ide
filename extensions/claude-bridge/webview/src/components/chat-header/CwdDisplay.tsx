import type { JSX } from 'preact'

interface CwdDisplayProps {
  cwd: string
}

export function CwdDisplay({ cwd }: CwdDisplayProps): JSX.Element {
  return (
    <span class="meta-cwd" title={cwd}>{cwd}</span>
  )
}
