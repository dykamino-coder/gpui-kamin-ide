import type { JSX } from 'preact'
import { activeCustomizePanel } from '../../../signals/ui'
import { SECTIONS, type CustomizeSectionId } from '../../customize/sections'

export function CustomizeLanding(): JSX.Element {
  function handleCard(panel: CustomizeSectionId): void {
    activeCustomizePanel.value = panel
    window.dispatchEvent(new CustomEvent('customize-panel', { detail: panel }))
  }

  return (
    <div style="padding:12px 8px;">
      {SECTIONS.map(sec => (
        <div
          key={sec.panel}
          class="customize-card"
          onClick={() => handleCard(sec.panel)}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleCard(sec.panel) } }}
          role="button"
          tabIndex={0}
        >
          <span class="card-icon"><i class={sec.icon} /></span>
          <span class="card-info">
            <span class="card-title">{sec.label}</span>
            <span class="card-desc">{sec.desc}</span>
          </span>
          <span class="card-arrow"><i class="fas fa-chevron-right" /></span>
        </div>
      ))}
    </div>
  )
}
