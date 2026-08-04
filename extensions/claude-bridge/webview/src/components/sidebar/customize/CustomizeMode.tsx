import type { JSX } from 'preact'
import { activeCustomizePanel } from '../../../signals/ui'
import { SECTIONS, type CustomizeSectionId } from '../../customize/sections'
import { CustomizeBackButton } from './CustomizeBackButton'
import { CustomizeItem } from './CustomizeItem'

export function CustomizeMode(): JSX.Element {
  const panel = activeCustomizePanel.value

  function handleItemClick(p: CustomizeSectionId): void {
    activeCustomizePanel.value = p
    window.dispatchEvent(new CustomEvent('customize-panel', { detail: p }))
  }

  return (
    <>
      <CustomizeBackButton />
      <div class="sidebar-customize-items">
        {SECTIONS.map(sec => (
          <CustomizeItem
            key={sec.panel}
            id={`sidebar-${sec.panel}-item`}
            icon={sec.icon}
            label={sec.label}
            panel={sec.panel}
            isActive={panel === sec.panel}
            onClick={() => handleItemClick(sec.panel)}
          />
        ))}
      </div>
    </>
  )
}
