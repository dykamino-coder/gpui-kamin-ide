import type { JSX } from 'preact'
import { activeTabId, activeSessionTitle } from '../../signals/tabs'
import { tabActivity, pinnedTitleColors } from '../../signals/ui'
import { activeThinkingPreview } from '../../signals/jsonl'
import { ActivityIndicator } from '../titlebar/ActivityIndicator'
import { getAgentColor, resolvePinnedColor } from '../../utils/agent-color'

/** Полоса активности над инпутом — ОТДЕЛЬНЫЙ компонент. Читает per-frame
 *  сигналы (thinking-превью растёт каждым rAF-флашем, tabActivity тикает ~1Гц
 *  OSC-спиннером любой вкладки) внутри себя: раньше их читал КОРЕНЬ чата и
 *  ререндерил всё дерево — хедер, сегмент-табы, вьювер, виджеты, инпут — на
 *  каждый кадр стрима (аудит #70 D1). */
export function ActivityStrip(): JSX.Element {
  const tabId = activeTabId.value
  const activity = tabId ? (tabActivity.value.get(tabId) ?? null) : null
  const title = activeSessionTitle.value
  return (
    <div style="position:relative;max-width:800px;width:100%;margin:0 auto;box-sizing:border-box">
      <ActivityIndicator
        visible={!!(activity?.isWorking)}
        text={activity?.rawTitle ?? ''}
        thinking={activeThinkingPreview.value}
        color={
          title
            ? (resolvePinnedColor(pinnedTitleColors.value[title]) ?? getAgentColor(title) ?? undefined)
            : undefined
        }
      />
    </div>
  )
}
