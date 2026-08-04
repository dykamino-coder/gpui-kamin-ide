import type { JSX } from 'preact'
import { useRef, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import { SettingsSection } from './SettingsSection'
import { SettingsInput } from './SettingsInput'
import styles from './WhisperSettings.module.css'

interface Props {
  url: string
  model: string
  token: string
  onUrlChange: (value: string) => void
  onModelChange: (value: string) => void
  onTokenChange: (value: string) => void
}

export function WhisperSettings({ url, model, token, onUrlChange, onModelChange, onTokenChange }: Props): JSX.Element {
  const bridge = useBridge()
  const [saveLabel, setSaveLabel] = useState('Save')

  async function handleSave(): Promise<void> {
    const cfg = await bridge.getConfig()
    bridge.setConfig({
      ...cfg,
      whisperUrl: url || undefined,
      whisperModel: model || undefined,
      whisperToken: token || undefined,
    })
    setSaveLabel('Saved!')
    setTimeout(() => setSaveLabel('Save'), 1500)
  }

  const [micStatus, setMicStatus] = useState<'idle' | 'waiting' | 'transcribing'>('idle')
  const [audioDataUri, setAudioDataUri] = useState<string | null>(null)
  const [micBars, setMicBars] = useState<number[]>([])
  const [transcription, setTranscription] = useState<string | null>(null)
  const recRef = useRef<MediaRecorder | null>(null)
  const rafRef = useRef<number>(0)

  function transcribe(base64: string, mimeType: string): void {
    bridge.getConfig().then((cfg: any) => {
      if (cfg.whisperUrl) {
        setMicStatus('transcribing')
        bridge.whisperTranscribe(base64, mimeType).then((result: any) => {
          if (result.text) {
            setTranscription(result.text)
            showToast({ type: 'success', title: 'Transcription', message: result.text.slice(0, 80) })
          } else {
            showToast({ type: 'error', title: 'Whisper', message: result.error || 'No text' })
          }
          setMicStatus('idle')
        }).catch((err: unknown) => {
          setMicStatus('idle')
          showToast({ type: 'error', title: 'Error', message: err instanceof Error ? err.message : 'Transcription failed' })
        })
      } else {
        setMicStatus('idle')
        showToast({ type: 'success', title: 'Recording complete', message: `${Math.round(base64.length / 1024)}KB` })
      }
    }).catch(() => {
      setMicStatus('idle')
    })
  }

  // Test Mic живёт на НАТИВНОМ getUserMedia + MediaRecorder — тем же путём,
  // что и голосовой ввод в чате. Chrome-PWA mic-мост Electron-эпохи вырезан
  // (аудит #70 A2: mic-* каналы в KaminIDE никем не слушались). Бары уровня —
  // WebAudio AnalyserNode вместо onMicLevels.
  async function handleTestMic(): Promise<void> {
    if (micStatus === 'waiting') {
      recRef.current?.stop()
      return
    }
    let stream: MediaStream
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    } catch (err) {
      showToast({ type: 'error', title: 'Mic', message: err instanceof Error ? err.message : 'Microphone unavailable' })
      return
    }
    setMicStatus('waiting')
    setAudioDataUri(null)
    setTranscription(null)
    const ac = new AudioContext()
    const analyser = ac.createAnalyser()
    analyser.fftSize = 64
    ac.createMediaStreamSource(stream).connect(analyser)
    const freq = new Uint8Array(analyser.frequencyBinCount)
    const tick = (): void => {
      analyser.getByteFrequencyData(freq)
      setMicBars(Array.from(freq, (v) => 3 + (v / 255) * 17))
      rafRef.current = requestAnimationFrame(tick)
    }
    rafRef.current = requestAnimationFrame(tick)
    const chunks: Blob[] = []
    const mr = new MediaRecorder(stream)
    recRef.current = mr
    mr.ondataavailable = (e) => chunks.push(e.data)
    mr.onstop = () => {
      cancelAnimationFrame(rafRef.current)
      void ac.close().catch(() => {})
      stream.getTracks().forEach((t) => t.stop())
      setMicBars([])
      const blob = new Blob(chunks, { type: mr.mimeType || 'audio/webm' })
      const reader = new FileReader()
      reader.onloadend = () => {
        const dataUri = String(reader.result ?? '')
        setAudioDataUri(dataUri)
        transcribe(dataUri.split(',')[1] ?? '', blob.type)
      }
      reader.readAsDataURL(blob)
    }
    mr.start()
  }

  return (
    <SettingsSection title="Voice Input (Whisper)">
      <SettingsInput label="Whisper API URL" type="text" placeholder="https://api.openai.com/v1/audio/transcriptions" value={url} onChange={onUrlChange} />
      <SettingsInput label="Model" type="text" placeholder="whisper-1" value={model} onChange={onModelChange} />
      <SettingsInput label="Whisper API Token" type="password" placeholder="Bearer token or API key" value={token} onChange={onTokenChange} />
      <div style="display:flex;gap:8px;margin-top:4px">
        <button class={styles.saveBtn} onClick={handleSave} style="flex:1">{saveLabel}</button>
        <button
          onClick={handleTestMic}
          style={`flex:1;padding:8px;border-radius:var(--radius-sm);cursor:pointer;font-size:12px;border:none;color:var(--bg-primary);font-weight:500;background:${micStatus === 'waiting' ? 'var(--accent-red)' : 'var(--accent-purple)'}`}
        >
          <i class={`fas ${micStatus === 'waiting' ? 'fa-stop' : 'fa-microphone'}`} style="margin-right:6px" />
          {micStatus === 'waiting' ? 'Stop' : 'Test Mic'}
        </button>
      </div>
      {micStatus === 'waiting' && (
        <div style="margin-top:6px;padding:8px 10px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);display:flex;align-items:center;gap:10px">
          <div style="width:8px;height:8px;border-radius:50%;background:var(--accent-red);animation:pulse-dot 1s infinite;flex-shrink:0" />
          <div style="flex:1;display:flex;align-items:center;gap:1px;height:20px">
            {(micBars.length > 0 ? micBars : Array(32).fill(3)).map((h, i) => (
              <div key={i} style={`width:3px;border-radius:999px;background:var(--accent-red);min-height:3px;height:${h}px`} />
            ))}
          </div>
          <span style="font-size:11px;color:var(--accent-red);flex-shrink:0">REC</span>
        </div>
      )}
      {micStatus === 'transcribing' && (
        <div style="margin-top:6px;padding:8px 10px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);display:flex;align-items:center;gap:8px">
          <i class="fas fa-spinner fa-spin" style="color:var(--accent-yellow);font-size:12px" />
          <span style="font-size:12px;color:var(--accent-yellow)">Transcribing...</span>
        </div>
      )}
      {audioDataUri && micStatus === 'idle' && (
        <div style="margin-top:6px;padding:6px 10px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm)">
          <div style="font-size:11px;color:var(--text-muted);margin-bottom:4px">Recorded audio:</div>
          <audio controls src={audioDataUri} style="width:100%;height:32px" />
          {transcription && (
            <div style="margin-top:6px;padding:6px 8px;border-left:2px solid var(--accent-green);font-size:12px;color:var(--accent-green)">
              {transcription}
            </div>
          )}
        </div>
      )}
      <p class={styles.hint}>Records via the system microphone (getUserMedia).</p>
    </SettingsSection>
  )
}
