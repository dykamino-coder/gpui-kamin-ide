// One-click connector presets extracted from AddServerForm.tsx (Sprint 5
// / Stage E1). Each preset fills the form for a known MCP server. The
// `apply` callback receives a small context bag of setters so the preset
// row stays decoupled from the form's internal state shape.

export type ServerType = 'stdio' | 'http' | 'sse' | 'ws'

export type PresetKey = 'figma' | 'context7' | 'playwright' | 'pencil' | 'chrome-devtools'

export interface PresetContext {
  setName: (v: string) => void
  setType: (v: ServerType) => void
  setUrl: (v: string) => void
  setHeaders: (v: string) => void
  setCommand: (v: string) => void
  setArgs: (v: string) => void
  setEnv: (v: string) => void
  setUseOAuth: (v: boolean) => void
  setAuthServerUrl: (v: string) => void
  setClientId: (v: string) => void
  setClientSecret: (v: string) => void
  setScope: (v: string) => void
}

export interface PresetDefinition {
  key: PresetKey
  label: string
  icon: string
  accent: string
  apply: (ctx: PresetContext) => void
}

export const PRESETS: PresetDefinition[] = [
  {
    key: 'figma',
    label: 'Figma',
    icon: 'fab fa-figma',
    accent: 'var(--accent-purple)',
    apply: (ctx) => {
      ctx.setName('figma')
      ctx.setType('http')
      ctx.setUrl('https://mcp.figma.com/mcp')
      ctx.setHeaders('')
      ctx.setUseOAuth(true)
      ctx.setAuthServerUrl('https://api.figma.com')
      ctx.setClientId('')
      ctx.setClientSecret('')
      ctx.setScope('')
    },
  },
  {
    key: 'context7',
    label: 'Context7',
    icon: 'fas fa-book',
    accent: 'var(--accent-primary)',
    apply: (ctx) => {
      ctx.setName('context7')
      ctx.setType('stdio')
      ctx.setCommand('npx')
      ctx.setArgs('-y,@upstash/context7-mcp@latest')
      ctx.setEnv('')
      ctx.setUseOAuth(false)
    },
  },
  {
    key: 'playwright',
    label: 'Playwright',
    icon: 'fas fa-globe',
    accent: 'var(--accent-green)',
    apply: (ctx) => {
      ctx.setName('playwright')
      ctx.setType('stdio')
      ctx.setCommand('npx')
      ctx.setArgs('@playwright/mcp@latest,--cdp-endpoint,http://localhost:9222')
      ctx.setEnv('')
      ctx.setUseOAuth(false)
    },
  },
  {
    key: 'pencil',
    label: 'Pencil',
    icon: 'fas fa-pen',
    accent: 'var(--accent-yellow)',
    apply: (ctx) => {
      // Pencil Desktop exposes a local HTTP MCP on 127.0.0.1:3100 while the
      // app is running — not an npm package. Requires Pencil.dev desktop open.
      ctx.setName('pencil')
      ctx.setType('http')
      ctx.setUrl('http://localhost:3100/mcp')
      ctx.setHeaders('')
      ctx.setUseOAuth(false)
    },
  },
  {
    key: 'chrome-devtools',
    label: 'Chrome DevTools',
    icon: 'fab fa-chrome',
    accent: 'var(--accent-orange)',
    apply: (ctx) => {
      // Google's official chrome-devtools MCP — drives a local Chrome via
      // CDP for performance tracing, console/network inspection, DOM ops.
      ctx.setName('chrome-devtools')
      ctx.setType('stdio')
      ctx.setCommand('npx')
      ctx.setArgs('-y,chrome-devtools-mcp@latest')
      ctx.setEnv('')
      ctx.setUseOAuth(false)
    },
  },
]
