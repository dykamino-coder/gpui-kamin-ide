import { createServer } from 'node:http'
import { afterEach, expect, it } from 'vitest'
import { transcribeAudio } from './transcription'

const close: Array<() => Promise<void>> = []
afterEach(async () => { for (const cleanup of close.splice(0)) await cleanup() })

it('posts actual recorded bytes, configured model and host credential as multipart', async () => {
  let requestBody = ''
  let authorization = ''
  const server = createServer(async (request, response) => {
    authorization = request.headers.authorization ?? ''
    for await (const chunk of request) requestBody += chunk.toString()
    response.setHeader('Content-Type', 'application/json')
    response.end(JSON.stringify({ text: 'recognized fixture speech' }))
  })
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
  close.push(() => new Promise(resolve => server.close(() => resolve())))
  const port = (server.address() as { port: number }).port
  const result = await transcribeAudio({ whisperUrl: `http://127.0.0.1:${port}/transcribe`, whisperModel: 'fixture-model', whisperToken: 'fixture-only' },
    Buffer.from('RIFF fixture recording bytes').toString('base64'), 'audio/wav')
  expect(result).toEqual({ text: 'recognized fixture speech' })
  expect(authorization).toBe('Bearer fixture-only')
  expect(requestBody).toContain('filename="recording.wav"')
  expect(requestBody).toContain('RIFF fixture recording bytes')
  expect(requestBody).toContain('fixture-model')
})

it('rejects unconfigured, invalid and oversized recordings without a network request', async () => {
  expect(await transcribeAudio({}, '', 'audio/wav')).toHaveProperty('error')
  const config = { whisperUrl: 'file:///private' }
  expect(await transcribeAudio(config, 'YQ==', 'audio/wav')).toHaveProperty('error')
  for (const [base64, mime] of [['!!!!', 'audio/wav'], ['YQ==', 'text/plain'], ['', 'audio/wav'], ['A'.repeat(11_184_816), 'audio/wav']]) {
    expect(await transcribeAudio({ whisperUrl: 'http://127.0.0.1:1' }, base64, mime)).toHaveProperty('error')
  }
})
