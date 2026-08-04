import type { JSX } from 'preact'
import { useComputed } from '@preact/signals'
import { tabAgentTrees } from '../../../signals/agents'
import { buildAgentTreeNodes } from '../../../hooks/useAgentTree'
import { TreeNode } from '../tree/TreeNode'

interface SessionAgentTreeProps {
  tabId: string
}

export function SessionAgentTree({ tabId }: SessionAgentTreeProps): JSX.Element | null {
  const children = useComputed(() => {
    tabAgentTrees.value
    return buildAgentTreeNodes(tabId)
  })
  if (children.value.length === 0) return null
  return (
    <div class="tree-children">
      {children.value.map(child => (
        <TreeNode key={child.id} node={child} depth={1} />
      ))}
    </div>
  )
}
