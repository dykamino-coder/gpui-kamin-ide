import { describe, expect, it, vi } from 'vitest'

import { handleServerMessage, type HandlerCtx } from './handle-server-message'

/** INC-2026-0053: горячая смена модели чистила консоль.
 *
 *  `session:model-changed` пересылался как `session-restarted`, а вебвью на
 *  любой `session-restarted` ставит `needsClear` и на следующем выводе PTY
 *  чистит буфер. PTY при горячей смене не перезапускался, поэтому живая
 *  прокрутка выбрасывалась ни за что. Признак `inPlace` несёт сервер; его
 *  отсутствие читается как настоящий перезапуск, то есть прежнее поведение. */
function ctxWithSpy(): { ctx: HandlerCtx; sent: { channel: string; args: unknown[] }[] } {
  const sent: { channel: string; args: unknown[] }[] = []
  const ctx = {
    tabId: 'tab-1',
    window: {
      webContents: {
        send: (channel: string, ...args: unknown[]) => {
          sent.push({ channel, args })
        },
      },
    },
    notifySessionInfo: vi.fn(),
  } as unknown as HandlerCtx
  return { ctx, sent }
}

function payloadOf(sent: { channel: string; args: unknown[] }[]): Record<string, unknown> {
  const event = sent.find((e) => e.channel === 'session-restarted')
  expect(event, 'событие о смене обязано уйти в вебвью').toBeTruthy()
  return event!.args[1] as Record<string, unknown>
}

describe('INC-2026-0053: горячая смена модели не выдаёт себя за перезапуск', () => {
  it('признак inPlace от сервера доезжает до вебвью', () => {
    const { ctx, sent } = ctxWithSpy()

    handleServerMessage(
      { type: 'session:model-changed', sessionId: 's-1', model: 'claude-fable-5-1', effort: 'high', inPlace: true },
      ctx,
    )

    expect(payloadOf(sent)).toMatchObject({ model: 'claude-fable-5-1', effort: 'high', inPlace: true })
  })

  it('сервер старого образца читается как настоящий перезапуск', () => {
    // Без признака консоль обязана чиститься по-прежнему: приложение и сервер
    // обновляются порознь, и молчание старого сервера за «не чистить» принять
    // нельзя.
    const { ctx, sent } = ctxWithSpy()

    handleServerMessage({ type: 'session:model-changed', sessionId: 's-1', model: 'claude-opus-5-5' }, ctx)

    expect(payloadOf(sent).inPlace).toBe(false)
  })

  it('настоящий перезапуск сессии признака не несёт', () => {
    const { ctx, sent } = ctxWithSpy()

    handleServerMessage(
      { type: 'session:model-changed', sessionId: 's-1', model: 'claude-opus-5-5', inPlace: 'да' as unknown as boolean },
      ctx,
    )

    // Только строгое `true` считается горячей сменой — иначе любой мусор в
    // поле отменял бы чистку после настоящего перезапуска.
    expect(payloadOf(sent).inPlace).toBe(false)
  })
})
