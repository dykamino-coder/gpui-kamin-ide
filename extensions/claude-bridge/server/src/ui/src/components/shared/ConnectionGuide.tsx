import { signal } from '@preact/signals'
import { Tabs } from './Tabs'
import { CodeBlock } from './CodeBlock'
import { getConnectionGuides, getGuideById } from '../../utils/connection-guides'
import type { Guide } from '../../utils/connection-guides'
import styles from './ConnectionGuide.module.css'

const activeTab = signal('claude-code')

interface ConnectionGuideProps {
  /** Show only this plugin's guide (no tabs). Used in plugin detail modal. */
  pluginId?: string
}

function GuideContent({ guide }: { guide: Guide }) {
  return (
    <div class={styles.guide}>
      {guide.sections.map((section, si) => (
        <div key={si} class={styles.section}>
          {section.heading && <h4 class={styles.heading}>{section.heading}</h4>}
          {section.text && <p class={styles.text}>{section.text}</p>}
          {section.blocks.length > 0 && (
            <div class={styles.blocks}>
              {section.blocks.map((block, bi) => (
                <CodeBlock key={bi} code={block.code} copyCode={block.copyCode} label={block.label} />
              ))}
            </div>
          )}
        </div>
      ))}
      {guide.hint && (
        <div class={styles.hint}>
          <i class="fa-solid fa-circle-info" />
          <span>{guide.hint}</span>
        </div>
      )}
    </div>
  )
}

export function ConnectionGuide({ pluginId }: ConnectionGuideProps) {
  // Single plugin mode (plugin detail modal)
  if (pluginId) {
    const guide = getGuideById(pluginId)
    if (!guide) return null
    return <GuideContent guide={guide} />
  }

  // Tabbed mode (tokens page)
  const guides = getConnectionGuides()
  const tabs = guides.map(g => ({ id: g.id, label: g.label }))
  const current = guides.find(g => g.id === activeTab.value) || guides[0]

  return (
    <div>
      <Tabs
        tabs={tabs}
        activeTab={activeTab.value}
        onChange={(id) => { activeTab.value = id }}
        variant="pills"
        size="sm"
      />
      <div style={{ marginTop: '12px' }}>
        <GuideContent guide={current} />
      </div>
    </div>
  )
}
