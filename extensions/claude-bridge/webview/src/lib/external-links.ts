// Route clicks on markdown-rendered links (tagged `data-ext-link` by
// render-markdown) to the host's openExternal so they open in the OS browser.
// A plain <a> can't navigate out of the sandboxed webview iframe (no
// allow-top-navigation / allow-popups), so the click is intercepted here and
// the URL handed to the extension host over the `open-external` channel.
import { inv } from "./bridge-transport.js"

let installed = false

export function installExternalLinkHandler(): void {
  if (installed) return
  installed = true
  document.addEventListener("click", (e) => {
    // Ignore modified clicks (the sandbox blocks new windows anyway, but be tidy).
    if (e.defaultPrevented || e.button !== 0) return
    const start = e.target as Element | null
    const a = start?.closest?.("a[data-ext-link]") as HTMLAnchorElement | null
    const href = a?.getAttribute("href")
    if (!href) return
    e.preventDefault()
    void inv("open-external", href).catch(() => { /* host logs failures */ })
  })
}
