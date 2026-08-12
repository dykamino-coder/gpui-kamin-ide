import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'

/** Renders an HTML file via the host-provided `<webview>` guest element at the
 *  real `file://` URL. Because the webview itself runs in the file://
 *  origin, it can load relative `<link>`/`<script>`/`<img>` assets
 *  from the same directory without Chromium's "Not allowed to load
 *  local resource" block. `disablewebsecurity` keeps mixed-origin
 *  assets working too.
 *
 *  Hot-reload: instead of a hard `webview.reload()` on every external
 *  change, we inject a small bootstrap script into the guest page on
 *  `dom-ready`. When a CSS / image / SVG file inside the watched
 *  directory changes, main fires `file-tree:change` with the exact
 *  path; we forward it to the guest via `executeJavaScript`, and the
 *  bootstrap patches the affected `<link>` / `<img>` cache-bust style,
 *  preserving DOM state, scroll position and JS variables. Only when
 *  the HTML itself changes (or anything we don't know how to swap)
 *  do we do a real reload. */
export function HtmlInlinePreview({ filePath }: { filePath: string }): JSX.Element {
  const bridge = useBridge()
  const containerRef = useRef<HTMLDivElement | null>(null)
  const url = 'file:///' + filePath.replace(/\\/g, '/').replace(/^\//, '')
  const dir = filePath.replace(/[\\/][^\\/]+$/, '')

  // Mount the <webview> imperatively — Preact JSX has no guest-element tag
  // type, and we want a stable reference to the element for
  // executeJavaScript() calls.
  useEffect(() => {
    const host = containerRef.current
    if (!host) return
    host.innerHTML = `<webview src="${escapeAttr(url)}" disablewebsecurity allowpopups style="display:flex;flex:1;min-height:0;width:100%;border:none;background:#fff"></webview>`

    const wv = host.querySelector('webview') as GuestWebviewElement | null
    if (!wv) return

    // Inject the live-reload bootstrap once per page load. `dom-ready`
    // fires after each navigation/reload, so this runs again if the
    // user (or our HTML-change reload below) reloads the guest.
    function inject(): void {
      try { wv?.executeJavaScript(BOOTSTRAP) } catch { /* webview not ready */ }
    }
    wv.addEventListener('dom-ready', inject)

    return () => {
      wv.removeEventListener('dom-ready', inject)
      host.innerHTML = ''
    }
  }, [url])

  // Subscribe to recursive watch on the file's directory so asset
  // edits (CSS, JS, images alongside the HTML) trigger hot-reload.
  // The same watcher also serves the file tree, ref-counted in main.
  useEffect(() => {
    if (!dir) return
    bridge.fileTreeWatch(dir)
    const off = bridge.onFileTreeChange((e) => {
      if (e.root !== dir) return
      const changed = e.changedPath
      if (!changed) return
      const wv = containerRef.current?.querySelector('webview') as GuestWebviewElement | null
      if (!wv) return
      // The HTML itself was changed → full reload. Anything else
      // (CSS / image / generic asset) is hot-patched in the guest.
      if (changed === filePath) {
        try { wv.reload() } catch { /* noop */ }
      } else {
        try {
          wv.executeJavaScript(`window.__hotReload && window.__hotReload(${JSON.stringify(changed)})`)
        } catch { /* noop */ }
      }
    })
    return () => {
      off()
      bridge.fileTreeUnwatch(dir)
    }
  }, [bridge, dir, filePath])

  return <div ref={containerRef} style="flex:1;min-height:0;display:flex;flex-direction:column" />
}

interface GuestWebviewElement extends HTMLElement {
  reload: () => void
  loadURL: (url: string) => void
  executeJavaScript: (code: string) => Promise<unknown>
}

function escapeAttr(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

const PREVIEWABLE_EXT = new Set(['html', 'htm', 'xhtml'])
export function hasInlinePreview(filePath: string): boolean {
  const ext = filePath.toLowerCase().match(/\.([a-z0-9]+)$/)?.[1] ?? ''
  return PREVIEWABLE_EXT.has(ext)
}

/** Bootstrap injected into the guest page on every load. Receives the
 *  absolute path of a changed file from the host and patches the
 *  matching <link> / <img> elements with a cache-busting query
 *  parameter — DOM state, scroll, and script variables stay intact.
 *  Falls back to `location.reload()` for unsupported file types. */
const BOOTSTRAP = `
(function() {
  if (window.__hotReload) return
  function basename(p) { return String(p).replace(/^.*[\\\\/]/, '') }
  function bust(href) {
    if (!href) return href
    var i = href.indexOf('?')
    var base = i >= 0 ? href.slice(0, i) : href
    return base + '?_hr=' + Date.now()
  }
  function matches(attr, name) {
    if (!attr) return false
    var clean = attr.split('?')[0]
    return clean === name || clean.endsWith('/' + name) || clean.endsWith('\\\\' + name)
  }
  window.__hotReload = function(changedPath) {
    var name = basename(changedPath)
    var ext = name.split('.').pop().toLowerCase()
    if (ext === 'css') {
      var hit = false
      document.querySelectorAll('link[rel="stylesheet"]').forEach(function(l) {
        var href = l.getAttribute('href')
        if (matches(href, name)) {
          l.setAttribute('href', bust(href))
          hit = true
        }
      })
      if (!hit) location.reload()
    } else if (/^(png|jpe?g|gif|webp|bmp|ico|svg|avif)$/.test(ext)) {
      document.querySelectorAll('img').forEach(function(i) {
        var src = i.getAttribute('src')
        if (matches(src, name)) i.setAttribute('src', bust(src))
      })
      // CSS background-image isn't easy to patch granularly — refresh
      // the stylesheets that may reference it.
      document.querySelectorAll('link[rel="stylesheet"]').forEach(function(l) {
        var href = l.getAttribute('href')
        if (href) l.setAttribute('href', bust(href))
      })
    } else {
      // JS or unknown → cleanest is a full reload.
      location.reload()
    }
  }
  // Toast in the corner so the dev sees the swap landed.
  var toast = document.createElement('div')
  toast.style.cssText = 'position:fixed;bottom:12px;right:12px;padding:6px 10px;background:rgba(0,0,0,0.7);color:#fff;font:11px ui-monospace,monospace;border-radius:4px;z-index:2147483647;pointer-events:none;opacity:0;transition:opacity 200ms'
  toast.textContent = 'hot reload ready'
  document.body.appendChild(toast)
  toast.style.opacity = '1'
  setTimeout(function() { toast.style.opacity = '0' }, 800)
})();
`
