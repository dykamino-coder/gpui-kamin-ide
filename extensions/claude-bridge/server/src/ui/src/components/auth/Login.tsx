// ============================================================================
// Login Screen — Sidebar Auth variant (V10)
// ============================================================================

import { useState } from 'preact/hooks'
import { login, loginError, loginLoading } from '../../signals/auth'
import styles from './Login.module.css'

function BridgeLogo() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2048 1380" width="48" height="32" style={{ display: 'block' }}>
      <path fill="currentColor" d="M353 13.3c35.4-1.5 79-.3 115-.3l214.5 0 653.5 0 280.1 0 67.9 0c12.5 0 37-.9 47.8 1.9 4.2 4.3 2.5 76.3 2.5 89.2l-.1 164.2c0 13-.9 58.2 1.6 66.8 4.8 3.2 73.1 1.9 83.2 1.9l152.4-.1c9.1 0 64.1-2.2 64.3 4l1.3 324.5c0 3.7-1.3 12.1-6 12.3-22 .8-44.2.5-66.3.5l-172.1 0c-9 0-49.4-.5-55.2.9-2 1.8-2.9 2.5-2.9 5.8-.3 20.1-.2 40.7-.2 60.8l0 121 0 380.4 0 75.5c0 12 .6 32.8-.8 43.8-7.9 8-130.5 1.6-148.5 3.8-5.4.7-40.6.3-45.8-1.5-6.6-10.1.8-312.2-3.1-357.3-10.1-1.9-50.5-1.1-62.7-1.1-30.8-.1-63.1-.2-93.9.3-1.1 0-7.4.8-7.5 1.1-2 3.6-1.9 14.7-1.9 18.5l0 226.2c0 34.4 0 69 0 103.5-.3 1.8 0 6.9-1.8 8-7.3 4.6-22.4 3-30.6 2.8l-90.7.1c-10.7 0-38.4.6-47.3-1.1-3.5-4.3-2.6-28.6-2.7-35.5-1.5-107.2 2.1-215.6-.3-322.8-10.4-1.9-46-.2-57.7-.2l-113.4 0-103.9 0c-17.1 0-35.6-.6-52.6 1l-5.3 10.6c-.3 29.6-.2 59.5-.2 89.1l0 166.4c0 12.7 2 84.5-2.6 90.8-17.6 4.1-72.2 1.2-93 2l-69-.4c-10.6-.3-9.6-8.5-9.5-17.1l.3-217.4 0-71.5c0-9.1 1.3-47.2-3.2-52.5-10.1-2.1-153.6-2.2-158.7-.2-3.3 1.3-3.8 5.7-4.1 8.8-1 9.7-.2 20.1-.2 29.9l.1 55.5 0 186 0 13.7c-3.3 1.4-6.6 1.5-10.1 1.6-15.4.6-147.2 1.3-154.6-1.1-5.4-1.8-3.9-13-3.9-17.8l.1-443.3 0-146.7c0-11.8 1.1-75.4-1.5-81-8.3-3.3-51.3-2.2-62.9-2.2l-189 0-55.8 0c-7.9 0-15.8.2-23.7 0-5.7-.1-5.5-8.5-5.5-12.4-.3-92.2-.1-184.5-.1-276.7.4-8.5-.8-41.9.8-47.6 1.7-6.3 85.4-4 92.6-4l153.7.1c23.4 0 64.8 1.6 88.7-2 3.8-.6 2.8-19.7 2.8-24.6l0-221.5c0-12.5-1.4-69.1 1.1-76z"/>
    </svg>
  )
}

export function Login() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  const handleSubmit = async (e: Event) => {
    e.preventDefault()
    if (!username.trim() || !password.trim()) return
    await login(username.trim(), password.trim())
  }

  return (
    <div class={styles.backdrop}>
      <div class={styles.card}>
        {/* Left info panel */}
        <div class={styles.info}>
          <div class={styles.logo}>
            <BridgeLogo />
          </div>
          <h2 class={styles.title}>Open Claude MAX Bridge</h2>
          <p class={styles.subtitle}>Bridge your Claude Max subscription to API client</p>
          <ul class={styles.features}>
            <li><i class="fa-solid fa-check" /> Anthropic & OpenAI API formats</li>
            <li><i class="fa-solid fa-check" /> Multi-user token management</li>
            <li><i class="fa-solid fa-check" /> Real-time usage dashboard</li>
            <li><i class="fa-solid fa-check" /> Session persistence & resumption</li>
          </ul>
        </div>

        {/* Right form panel */}
        <form class={styles.formSide} onSubmit={handleSubmit}>
          <h3 class={styles.formTitle}>Sign In</h3>

          <div class={styles.formGroup}>
            <label class={styles.label}>Username</label>
            <input
              class={styles.input}
              type="text"
              placeholder="admin"
              value={username}
              onInput={(e) => setUsername((e.target as HTMLInputElement).value)}
              autoFocus
            />
          </div>

          <div class={styles.formGroup}>
            <label class={styles.label}>Password</label>
            <input
              class={styles.input}
              type="password"
              placeholder="Enter password"
              value={password}
              onInput={(e) => setPassword((e.target as HTMLInputElement).value)}
            />
          </div>

          {loginError.value && (
            <div class={styles.error}>
              <i class="fa-solid fa-circle-exclamation" />
              {loginError.value}
            </div>
          )}

          <button
            class={styles.submitBtn}
            type="submit"
            disabled={loginLoading.value || !username.trim() || !password.trim()}
          >
            {loginLoading.value ? 'Signing in...' : 'Sign In'}
          </button>
        </form>
      </div>
    </div>
  )
}
