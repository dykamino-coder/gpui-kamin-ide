// Обратное давление для выгрузки расшифровки (INC-2026-0019).
//
// Цикл выгрузки слал начало, ВСЕ куски и конец подряд через сырой `ws.send`:
// без ожидания опустошения буфера, без проверки `bufferedAmount` и без отмены
// при разрыве. Куски маленькие, но соединение уже могло стоять выше
// предполагаемого предела исходящего буфера — и тогда медленный клиент копил
// работу выгрузки поверх обычного трафика сессии.
//
// Обычный защищённый отправитель (`sendToClient`) здесь НЕ годится: за
// пределом он БРОСАЕТ кадр. Для потока и журнала это самолечится повторной
// доставкой, а у выгрузки повторной доставки нет — брошенный кусок означает
// молча испорченный файл у пользователя. Поэтому давление здесь без потерь:
// ждём, пока буфер опустеет, а если он не опустеет — выгрузка честно
// прекращается с ошибкой.

import fsp from 'fs/promises'
import { StringDecoder } from 'string_decoder'

export interface DrainableSocket {
  readonly readyState: number
  readonly bufferedAmount: number
}

export type DrainOutcome = 'ready' | 'closed' | 'timeout'

export interface DrainOptions {
  /** Порог, выше которого новый кусок не отправляется. */
  limit: number
  /** Пауза между проверками. */
  pollMs: number
  /** Сколько всего ждать опустошения, прежде чем признать соединение мёртвым. */
  timeoutMs: number
  /** Подменяемы в тестах: иначе проверка ждала бы настоящие секунды. */
  now?: () => number
  sleep?: (ms: number) => Promise<void>
}

const OPEN = 1

/**
 * Дождаться места в исходящем буфере.
 *
 * Возвращает `closed`, если соединение закрылось, и `timeout`, если буфер так
 * и не опустел. Оба исхода означают «дальше слать нельзя» — и вызывающий
 * обязан прекратить выгрузку, а не досылать куски в никуда.
 */
export async function waitForDrain(ws: DrainableSocket, opts: DrainOptions): Promise<DrainOutcome> {
  const now = opts.now ?? (() => Date.now())
  const sleep = opts.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)))
  const deadline = now() + opts.timeoutMs
  // Уступить циклу событий ПЕРЕД первой же проверкой. Без этого при свободном
  // буфере выгрузка снова шла бы подряд, и буферу неоткуда было бы опустеть:
  // доказательство карточки — все отправки происходили до микрозадачи,
  // запланированной первой из них.
  await sleep(0)
  for (;;) {
    if (ws.readyState !== OPEN) return 'closed'
    if (ws.bufferedAmount <= opts.limit) return 'ready'
    if (now() >= deadline) return 'timeout'
    await sleep(opts.pollMs)
  }
}

export interface ChunkSocket extends DrainableSocket {
  send(data: string): void
}

/**
 * Stream `[0, total)` of a file to the socket as `jsonl:download-chunk` frames
 * of at most `chunkBytes` source bytes, waiting for drain before each. Only one
 * chunk is in memory; a UTF-8 character split by a chunk edge is carried over
 * by the decoder. Returns null when every chunk was sent, else why it stopped.
 */
export async function streamFileChunks(
  ws: ChunkSocket,
  filePath: string,
  total: number,
  chunkBytes: number,
  drain: DrainOptions,
): Promise<DrainOutcome | null> {
  const decoder = new StringDecoder('utf8')
  const fh = await fsp.open(filePath, 'r')
  try {
    const buf = Buffer.allocUnsafe(chunkBytes)
    for (let sent = 0; sent < total;) {
      const gate = await waitForDrain(ws, drain)
      if (gate !== 'ready') return gate
      const { bytesRead } = await fh.read(buf, 0, Math.min(chunkBytes, total - sent), sent)
      if (bytesRead === 0) break
      sent += bytesRead
      let chunk = decoder.write(buf.subarray(0, bytesRead))
      if (sent >= total) chunk += decoder.end()
      ws.send(JSON.stringify({ type: 'jsonl:download-chunk', chunk, sent, total }))
    }
    return null
  } finally {
    await fh.close()
  }
}
