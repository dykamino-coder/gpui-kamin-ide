import { useEffect, useRef } from 'preact/hooks'
import { signal } from '@preact/signals'
import { api } from '../../services/api-client'
import type { ProxySettings } from '../../services/api-client'
import { Card } from '../shared'
import { serverStatus } from '../../signals/server'
import type { ServerConfig } from '../../types'
import styles from './CompactConfig.module.css'

const config = signal<ServerConfig | null>(null)
const copiedUrl = signal<string | null>(null)

// Editable settings form state
const settingsForm = signal<ProxySettings | null>(null)
const savedSettings = signal<ProxySettings | null>(null)  // snapshot from server
const saving = signal(false)
const saved = signal(false)

function copyUrl(url: string) {
  const fallback = () => {
    const ta = document.createElement('textarea')
    ta.value = url
    ta.style.cssText = 'position:fixed;opacity:0;left:-9999px'
    document.body.appendChild(ta)
    ta.focus()
    ta.select()
    try { document.execCommand('copy') } catch {}
    document.body.removeChild(ta)
  }
  if (navigator.clipboard?.writeText) {
    navigator.clipboard.writeText(url).catch(fallback)
  } else {
    fallback()
  }
  copiedUrl.value = url
  setTimeout(() => { if (copiedUrl.value === url) copiedUrl.value = null }, 1500)
}

export function CompactConfig() {
  const certInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    api.getConfig().then((c) => { config.value = c }).catch(() => {})
    api.getProxySettings().then((ps) => { settingsForm.value = ps; savedSettings.value = { ...ps } }).catch(() => {})
  }, [])

  const c = config.value
  const form = settingsForm.value
  if (!c) return null

  const host = typeof window !== 'undefined' ? window.location.host : `${c.host}:${c.port}`
  const proto = typeof window !== 'undefined' ? window.location.protocol : 'http:'

  function updateForm(patch: Partial<ProxySettings>) {
    if (!settingsForm.value) return
    settingsForm.value = { ...settingsForm.value, ...patch }
  }

  function isDirty(key: keyof ProxySettings): boolean {
    if (!form || !savedSettings.value) return false
    return form[key] !== savedSettings.value[key]
  }

  function fieldClass(base: string, key: keyof ProxySettings): string {
    return isDirty(key) ? `${base} ${styles.dirty}` : base
  }

  async function handleReset() {
    saving.value = true
    try {
      const result = await api.resetProxySettings()
      settingsForm.value = result
      savedSettings.value = { ...result }
      api.getConfig().then((c) => { config.value = c }).catch(() => {})
      api.getStatus().then((s) => { serverStatus.value = s as any }).catch(() => {})
      saved.value = true
      setTimeout(() => { saved.value = false }, 2000)
    } catch {}
    saving.value = false
  }

  async function handleSave() {
    if (!settingsForm.value) return
    saving.value = true
    try {
      const result = await api.updateProxySettings(settingsForm.value)
      settingsForm.value = result
      savedSettings.value = { ...result }
      // Refresh config & status displays
      api.getConfig().then((c) => { config.value = c }).catch(() => {})
      api.getStatus().then((s) => { serverStatus.value = s as any }).catch(() => {})
      saved.value = true
      setTimeout(() => { saved.value = false }, 2000)
    } catch {}
    saving.value = false
  }

  return (
    <div class={styles.grid}>
      {/* Left column — Server + Base URLs */}
      <div class={styles.leftCol}>
        {/* Server Info — read-only */}
        <Card className={styles.section}>
          <h4 class={styles.sectionTitle}><i class="fa-solid fa-gear" /> Server</h4>
          <div class={styles.configGrid}>
            <div class={styles.configItem}>
              <span class={styles.configLabel}>Host</span>
              <code class={styles.configValue}>{host}</code>
            </div>
            <div class={styles.configItem}>
              <span class={styles.configLabel}>Mode</span>
              <code class={styles.configValue}>PTY</code>
            </div>
          </div>
        </Card>

      </div>

      {/* Right column — Editable Settings */}
      {form && (
        <Card className={styles.section}>
          <h4 class={styles.sectionTitle}><i class="fa-solid fa-sliders" /> Settings</h4>
          <div class={styles.settingsGrid}>
            <div class={styles.settingsField}>
              <label class={styles.settingsLabel}>Log Level</label>
              <select
                class={fieldClass(styles.settingsSelect, 'logLevel')}
                value={form.logLevel}
                onChange={(e) => updateForm({ logLevel: (e.target as HTMLSelectElement).value })}
              >
                <option value="error">error</option>
                <option value="warn">warn</option>
                <option value="info">info</option>
                <option value="debug">debug</option>
                <option value="verbose">verbose</option>
              </select>
            </div>
            <div class={styles.settingsField}>
              <label class={styles.settingsLabel}>DB Max Requests</label>
              <input class={fieldClass(styles.settingsInput, 'maxRequests')} type="number" min="100" step="100"
                value={form.maxRequests}
                onInput={(e) => updateForm({ maxRequests: parseInt((e.target as HTMLInputElement).value) || 1000 })}
              />
            </div>
          </div>

          {/* Network / Proxy */}
          <h4 class={`${styles.sectionTitle} ${styles.subTitle}`}><i class="fa-solid fa-shield-halved" /> Network / Proxy</h4>
          <div class={styles.proxyForm}>
            <div class={styles.proxyField}>
              <label class={styles.proxyLabel}>HTTP Proxy</label>
              <input class={fieldClass(styles.proxyInput, 'httpProxy')} type="text" placeholder="http://proxy:8080"
                value={form.httpProxy || ''}
                onInput={(e) => updateForm({ httpProxy: (e.target as HTMLInputElement).value || null })}
              />
            </div>
            <div class={styles.proxyField}>
              <label class={styles.proxyLabel}>HTTPS Proxy</label>
              <input class={fieldClass(styles.proxyInput, 'httpsProxy')} type="text" placeholder="http://proxy:8080"
                value={form.httpsProxy || ''}
                onInput={(e) => updateForm({ httpsProxy: (e.target as HTMLInputElement).value || null })}
              />
            </div>
            <div class={styles.proxyField}>
              <label class={styles.proxyLabel}>NO_PROXY</label>
              <input class={fieldClass(styles.proxyInput, 'noProxy')} type="text" placeholder="127.0.0.1,localhost,::1"
                value={form.noProxy || ''}
                onInput={(e) => updateForm({ noProxy: (e.target as HTMLInputElement).value || null })}
              />
            </div>
            <div class={styles.proxyField}>
              <label class={styles.proxyLabel}>CA Certificate</label>
              <div class={styles.certRow}>
                <code class={styles.certPath} title={form.caCert || ''}>{form.caCert || 'Not set'}</code>
                <button class={styles.certUploadBtn} onClick={() => certInputRef.current?.click()}>
                  <i class="fa-solid fa-upload" /> Upload
                </button>
                {form.caCert && (
                  <button class={styles.certClearBtn} onClick={() => updateForm({ caCert: null })}>
                    <i class="fa-solid fa-xmark" />
                  </button>
                )}
                <input ref={certInputRef} type="file" accept=".crt,.pem,.cer,.cert" style={{ display: 'none' }}
                  onChange={async (e) => {
                    const file = (e.target as HTMLInputElement).files?.[0]
                    if (!file) return
                    try {
                      const result = await api.uploadCaCert(file)
                      settingsForm.value = { ...settingsForm.value!, ...result }
                      savedSettings.value = { ...settingsForm.value! }
                      saved.value = true
                      setTimeout(() => { saved.value = false }, 2000)
                    } catch {}
                    (e.target as HTMLInputElement).value = ''
                  }}
                />
              </div>
            </div>
          </div>

          {/* Save button */}
          <div class={styles.proxyActions}>
            <button class={styles.proxySaveBtn} disabled={saving.value} onClick={handleSave}>
              {saving.value
                ? <><i class="fa-solid fa-spinner fa-spin" /> Saving...</>
                : saved.value
                  ? <><i class="fa-solid fa-check" /> Saved</>
                  : <><i class="fa-solid fa-floppy-disk" /> Save</>}
            </button>
            <span class={styles.proxyHint}>Changes apply to new requests immediately</span>
            <button class={styles.resetBtn} disabled={saving.value} onClick={handleReset}>
              <i class="fa-solid fa-rotate-left" /> Reset to Defaults
            </button>
          </div>
        </Card>
      )}
    </div>
  )
}
