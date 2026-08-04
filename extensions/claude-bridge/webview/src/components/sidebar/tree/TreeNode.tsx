import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import type { TreeNode as TreeNodeType } from '../../../../shared/types'
import { activeTabId } from '../../../signals/tabs'
import { TreeToggle } from './TreeToggle'
import { TreeIcon } from './TreeIcon'
import { TreeStatusDot } from './TreeStatusDot'

interface TreeNodeProps {
  node: TreeNodeType
  depth: number
}

export function TreeNode({ node, depth }: TreeNodeProps): JSX.Element {
  const [collapsed, setCollapsed] = useState(false)
  const hasChildren = node.children.length > 0
  const isActive = node.id === activeTabId.value
  const isSession = node.type === 'session'

  const isUnnamed = isSession && (!node.label || /^#\d+$/.test(node.label))
  const displayLabel = (node as any).sessionTitle || (isUnnamed ? 'New session' : node.label)
  const labelClass = (!(isUnnamed) || (node as any).sessionTitle) ? 'session-label' : 'session-label unnamed'
  const modelBadge = node.model ? node.model : null

  function handleClick(e: MouseEvent): void {
    const target = e.target as HTMLElement
    if (target.closest('.session-close') || target.closest('.session-rename')) return
    if (target.classList.contains('tree-toggle') && hasChildren) {
      setCollapsed(v => !v)
      return
    }
    if (node.type === 'team' || node.type === 'teammate' || node.type === 'agent') {
      // Agent chat panel — will be handled by parent via event bus or signal
      window.dispatchEvent(new CustomEvent('show-agent-panel', { detail: node }))
      return
    }
    const bridge = (window as any).electronBridge
    bridge?.switchTab(node.id)
  }

  function handleRename(e: MouseEvent): void {
    e.stopPropagation()
    const bridge = (window as any).electronBridge
    bridge?.sendInput(node.id, '/rename\r')
  }

  function handleClose(e: MouseEvent): void {
    e.stopPropagation()
    const bridge = (window as any).electronBridge
    bridge?.closeTab(node.id)
  }

  return (
    <div class="tree-node" data-depth={String(depth)}>
      <div
        class={`sidebar-session${isActive ? ' active' : ''}`}
        data-tab-id={node.id}
        data-type={node.type !== 'session' ? node.type : undefined}
        onClick={handleClick}
      >
        <TreeToggle
          hasChildren={hasChildren}
          isCollapsed={collapsed}
        />
        <TreeStatusDot status={node.status} />
        <TreeIcon type={node.type} />
        <span class={labelClass}>{displayLabel}</span>
        {modelBadge && <span class="session-model">{modelBadge}</span>}
        {isSession && (
          <span
            class="session-rename"
            data-tooltip="Auto-rename"
            onClick={handleRename}
            role="button"
            tabIndex={0}
          >
            <i class="fas fa-pen" />
          </span>
        )}
        {isSession && (
          <span
            class="session-close"
            data-tooltip="Close session"
            onClick={handleClose}
            role="button"
            tabIndex={0}
          >
            <i class="fas fa-xmark" />
          </span>
        )}
      </div>
      {hasChildren && (
        <div class={`tree-children${collapsed ? ' collapsed' : ''}`}>
          {node.children.map(child => (
            <TreeNode key={child.id} node={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  )
}
