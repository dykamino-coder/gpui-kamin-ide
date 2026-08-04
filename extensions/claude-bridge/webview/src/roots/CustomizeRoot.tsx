import { Fragment } from 'preact'
import type { JSX } from 'preact'
import { useRef } from 'preact/hooks'
import { useSignal } from '@preact/signals'

import { activeCustomizePanel } from '../signals/ui'
import { useBridge } from '../hooks/useBridge'
import { useInit } from '../hooks/useInit'
import { useBridgeListeners } from '../hooks/useBridgeListeners'
import { useThemeHotkey } from '../hooks/useThemeHotkey'

import { CustomizeContentPanel, sectionFromPageUrl } from '../components/customize/CustomizeContentPanel'
import { Tooltip } from '../components/overlays/Tooltip'
import { ConfirmModalHost } from '../components/overlays/ConfirmModalHost'

// Claude Bridge "Customize" PAGE — one section (Settings/Skills/MCP/…), rendered
// inside KaminIDE's own Customize area. Each section is its OWN webview view
// (a separate page) under the `claudeBridgeCustomize` container; KaminIDE renders
// the container's views as a TOC tree (see docs/BRIDGE_VSIX_INTEGRATION.md). All
// pages load this same bundle — we read WHICH section from the iframe URL (the
// view id is the path) and render only that panel, no internal nav.

// The iframe's URL never changes — pin the panel signal once at module load
// (a useSignalEffect here read no signals, so it ran exactly once and could
// never RE-pin the panel if legacy code nulled it later).
activeCustomizePanel.value = sectionFromPageUrl() ?? 'settings'

export function CustomizeRoot(): JSX.Element {
  const bridge = useBridge()
  const promptDebounceTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const vscodeAvailable = useSignal(false)

  useBridgeListeners(bridge, promptDebounceTimers, vscodeAvailable, 'customize')
  useThemeHotkey()
  useInit(bridge, 'customize')

  return (
    <Fragment>
      <div style="height:100%;width:100%;overflow:auto;box-sizing:border-box">
        <CustomizeContentPanel />
      </div>
      <Tooltip />
      <ConfirmModalHost />
    </Fragment>
  )
}
