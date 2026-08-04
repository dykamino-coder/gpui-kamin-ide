import { useState, useCallback, useRef, useEffect } from 'preact/hooks'
import { useBridge } from './useBridge'

type VoiceState = 'idle' | 'countdown' | 'recording' | 'processing'

export interface VoiceInputState {
  state: VoiceState
  countdownDigit: number
  visible: boolean
}

export interface VoiceInputHandlers {
  toggle: () => void
  setVisible: (v: boolean) => void
}

const MAX_RECORDING_MINUTES = 5

function getSupportedMimeType(): string {
  const types = ['audio/webm;codecs=opus', 'audio/webm', 'audio/mp4']
  return types.find(t => MediaRecorder.isTypeSupported(t)) || ''
}

function sanitize(text: string): string {
  const placeholders = [
    'субтитры сделал dimatorzok', 'субтитры создавал dimatorzok',
    'продолжение следует...', 'продолжение следует',
    'звук не обнаружен', 'речь не распознана', 'ничего не распознано',
    'no speech detected', 'speech not recognized',
  ]
  let cleaned = text
  for (const phrase of placeholders) {
    cleaned = cleaned.replace(new RegExp(phrase.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi'), '')
  }
  return cleaned.trim()
}

export function useVoiceInput(
  onTextReady: (text: string) => void,
): [VoiceInputState, VoiceInputHandlers] {
  const bridge = useBridge()
  const [voiceState, setVoiceState] = useState<VoiceState>('idle')
  const [countdownDigit, setCountdownDigit] = useState(3)
  const [visible, setVisible] = useState(false)

  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const audioChunksRef = useRef<BlobPart[]>([])
  const recordingModeRef = useRef<'chromium' | null>(null)
  const timerIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const maxDurationRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const countdownTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const stateRef = useRef<VoiceState>('idle')

  function updateState(s: VoiceState): void {
    stateRef.current = s
    setVoiceState(s)
  }

  function stopTimer(): void {
    if (timerIntervalRef.current) { clearInterval(timerIntervalRef.current); timerIntervalRef.current = null }
    if (maxDurationRef.current) { clearInterval(maxDurationRef.current); maxDurationRef.current = null }
  }

  async function sendToWhisper(base64: string, mimeType: string): Promise<void> {
    const result = await bridge.whisperTranscribe(base64, mimeType)
    if (result.error) { console.warn('[voice] Whisper error:', result.error); return }
    const text = (result.text || '').trim()
    if (text) onTextReady(sanitize(text))
  }

  async function processChromiumRecording(): Promise<void> {
    try {
      const mr = mediaRecorderRef.current
      const audioBlob = new Blob(audioChunksRef.current, { type: mr?.mimeType || 'audio/webm' })
      const arrayBuffer = await audioBlob.arrayBuffer()
      const ctx = new AudioContext()
      const audioBuffer = await ctx.decodeAudioData(arrayBuffer)
      ctx.close()
      // Resample to 16kHz mono
      const TARGET_RATE = 16000
      const targetLength = Math.round(audioBuffer.duration * TARGET_RATE)
      const offlineCtx = new OfflineAudioContext(1, targetLength, TARGET_RATE)
      const source = offlineCtx.createBufferSource()
      source.buffer = audioBuffer
      source.connect(offlineCtx.destination)
      source.start(0)
      const resampled = await offlineCtx.startRendering()
      // Encode to WAV
      const length = resampled.length
      const sampleRate = resampled.sampleRate
      const bytesPerSample = 2
      const blockAlign = bytesPerSample
      const byteRate = sampleRate * blockAlign
      const dataSize = length * blockAlign
      const ab = new ArrayBuffer(44 + dataSize)
      const v = new DataView(ab)
      const w = (off: number, s: string) => { for (let i = 0; i < s.length; i++) v.setUint8(off + i, s.charCodeAt(i)) }
      w(0, 'RIFF'); v.setUint32(4, ab.byteLength - 8, true); w(8, 'WAVE')
      w(12, 'fmt '); v.setUint32(16, 16, true); v.setUint16(20, 1, true)
      v.setUint16(22, 1, true); v.setUint32(24, sampleRate, true)
      v.setUint32(28, byteRate, true); v.setUint16(32, blockAlign, true)
      v.setUint16(34, 16, true); w(36, 'data'); v.setUint32(40, dataSize, true)
      let off = 44
      const ch = resampled.getChannelData(0)
      for (let i = 0; i < length; i++) {
        const val = Math.max(-1, Math.min(1, ch[i]))
        v.setInt16(off, val < 0 ? val * 0x8000 : val * 0x7FFF, true)
        off += 2
      }
      // Chunked base64 encoding — avoids call stack overflow and main thread freeze on large audio
      const bytes = new Uint8Array(ab)
      let wavBase64 = ''
      for (let i = 0; i < bytes.length; i += 8192) {
        wavBase64 += String.fromCharCode(...bytes.subarray(i, i + 8192))
      }
      wavBase64 = btoa(wavBase64)
      await sendToWhisper(wavBase64, 'audio/wav')
    } catch (err) {
      console.error('[voice] Chromium processing error:', err)
    } finally {
      updateState('idle')
    }
  }

  function stopRecording(): void {
    const s = stateRef.current
    if (s !== 'recording' && s !== 'countdown') return
    stopTimer()
    if (countdownTimeoutRef.current) { clearTimeout(countdownTimeoutRef.current); countdownTimeoutRef.current = null }
    updateState('processing')

    if (recordingModeRef.current === 'chromium' && mediaRecorderRef.current) {
      mediaRecorderRef.current.stop()
      mediaRecorderRef.current.stream.getTracks().forEach(t => t.stop())
    }
  }

  function startCountdown(onFinish: () => void): void {
    let count = 3
    setCountdownDigit(count)
    updateState('countdown')

    countdownTimeoutRef.current = setTimeout(function tick() {
      count--
      if (stateRef.current !== 'countdown') return
      if (count > 0) {
        setCountdownDigit(count)
        countdownTimeoutRef.current = setTimeout(tick, 1000)
      } else {
        onFinish()
      }
    }, 1000)
  }

  async function startRecording(): Promise<void> {
    if (stateRef.current !== 'idle') return

    // Chrome-PWA mic-мост Electron-эпохи вырезан (аудит #70 A2): в KaminIDE
    // mic-* каналы никем не слушались, micConnected всегда был falsy —
    // остался только нативный getUserMedia.
    let stream: MediaStream | null = null
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    } catch {}

    if (stream) {
      recordingModeRef.current = 'chromium'
      const mimeType = getSupportedMimeType()
      const mr = new MediaRecorder(stream, mimeType ? { mimeType, audioBitsPerSecond: 16000 } : {})
      audioChunksRef.current = []
      mr.ondataavailable = (e) => audioChunksRef.current.push(e.data)
      mr.onstop = () => void processChromiumRecording()
      mediaRecorderRef.current = mr

      updateState('recording')
      mr.start()
      const startTime = Date.now()
      maxDurationRef.current = setInterval(() => {
        if (stateRef.current !== 'recording') { stopTimer(); return }
        if ((Date.now() - startTime) / 1000 >= MAX_RECORDING_MINUTES * 60) {
          stopRecording()
        }
      }, 1000)
    } else {
      console.warn('[voice] No mic available — neither PWA bridge nor native getUserMedia')
      updateState('idle')
    }
  }

  // Cleanup on unmount: stop timers, media streams
  useEffect(() => {
    return () => {
      stopTimer()
      if (countdownTimeoutRef.current) {
        clearTimeout(countdownTimeoutRef.current)
        countdownTimeoutRef.current = null
      }
      if (mediaRecorderRef.current?.stream) {
        mediaRecorderRef.current.stream.getTracks().forEach(t => t.stop())
      }
    }
  }, [])

  const toggle = useCallback(() => {
    const s = stateRef.current
    if (s === 'processing') return
    if (s === 'recording' || s === 'countdown') {
      stopRecording()
    } else {
      void startRecording()
    }
  }, [])

  return [
    { state: voiceState, countdownDigit, visible },
    { toggle, setVisible },
  ]
}
