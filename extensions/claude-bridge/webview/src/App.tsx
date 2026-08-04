import { Fragment } from 'preact'
import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { useSignal } from '@preact/signals'

// Signals
import { tabs, activeTabId } from './signals/tabs'
import { tabActivity, activeCustomizePanel } from './signals/ui'

// Hooks
import { useBridge } from './hooks/useBridge'
import { useDragDrop } from './hooks/useDragDrop'
import { useBridgeListeners } from './hooks/useBridgeListeners'
import { useInit } from './hooks/useInit'
import { useVersionPoller } from './hooks/useVersionPoller'
import { useGlobalEscapeCancel } from './hooks/useGlobalEscapeCancel'
import { useMcpElicitationPrompt } from './hooks/useMcpElicitationPrompt'
import { useThemeHotkey } from './hooks/useThemeHotkey'
import { useCodeCopyButtons } from './hooks/useCodeCopyButtons'

// Layout
import { AppLayout } from './components/layout/AppLayout'
import { AppMainContent } from './components/layout/AppMainContent'

// UI Components
import { Sidebar } from './components/sidebar/Sidebar'

// Overlays
import { ConfirmModal } from './components/overlays/ConfirmModal'
import { DropOverlay } from './components/overlays/DropOverlay'
import { Tooltip } from './components/overlays/Tooltip'
import { ToastContainer } from './components/overlays/ToastContainer'
import { NoTokenGateModal } from './components/overlays/NoTokenGateModal'
import { PluginHookApprovalModal } from './components/customize/hooks/PluginHookApprovalModal'

export function App(): JSX.Element {
  const bridge = useBridge()
  const { isDragging } = useDragDrop()

  const confirmModal = useSignal<{
    isOpen: boolean
    title: string
    bodyHtml: string
    confirmLabel: string
    isDanger: boolean
    onConfirm: () => void
  }>({
    isOpen: false,
    title: '',
    bodyHtml: '',
    confirmLabel: 'Delete',
    isDanger: true,
    onConfirm: () => {},
  })

  const vscodeAvailable = useSignal(false)
  const promptDebounceTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())

  // Derived from active tab
  const activeTab = tabs.value.find(t => t.id === activeTabId.value) ?? null
  const activity = activeTabId.value ? (tabActivity.value.get(activeTabId.value) ?? null) : null
  const isCustomizeOpen = activeCustomizePanel.value !== null && activeCustomizePanel.value !== 'landing'

  // Expose confirm modal helper globally for imperative usage from other components
  const showConfirmRef = useRef<(opts: {
    title: string
    bodyHtml: string
    confirmLabel?: string
    isDanger?: boolean
  }) => Promise<boolean>>(() => Promise.resolve(false))

  useEffect(() => {
    showConfirmRef.current = (opts) => {
      return new Promise<boolean>((resolve) => {
        confirmModal.value = {
          isOpen: true,
          title: opts.title,
          bodyHtml: opts.bodyHtml,
          confirmLabel: opts.confirmLabel ?? 'Delete',
          isDanger: opts.isDanger ?? true,
          onConfirm: () => {
            confirmModal.value = { ...confirmModal.value, isOpen: false }
            resolve(true)
          },
        }
      })
    }
    ;(window as any).__showConfirmModal = showConfirmRef.current
  }, [])

  useBridgeListeners(bridge, promptDebounceTimers, vscodeAvailable)
  useGlobalEscapeCancel(bridge)
  useCodeCopyButtons()
  useMcpElicitationPrompt(bridge)
  useThemeHotkey()
  useInit(bridge)
  useVersionPoller()

  const tabId = activeTabId.value
  const connStatus = activeTab?.status ?? 'disconnected'

  return (
    <Fragment>
      <AppLayout
        sidebar={<Sidebar />}
        mainContent={
          <AppMainContent
            bridge={bridge}
            isCustomizeOpen={isCustomizeOpen}
            tabId={tabId}
            activeTab={activeTab}
            connStatus={connStatus}
            activity={activity}
            vscodeAvailable={vscodeAvailable.value}
          />
        }
      />
      <ConfirmModal
        isOpen={confirmModal.value.isOpen}
        title={confirmModal.value.title}
        bodyHtml={confirmModal.value.bodyHtml}
        confirmLabel={confirmModal.value.confirmLabel}
        isDanger={confirmModal.value.isDanger}
        onConfirm={confirmModal.value.onConfirm}
        onCancel={() => {
          confirmModal.value = { ...confirmModal.value, isOpen: false }
        }}
      />
      <DropOverlay active={isDragging} />
      <Tooltip />
      <ToastContainer />
      <NoTokenGateModal />
      <PluginHookApprovalModal />
    </Fragment>
  )
}
