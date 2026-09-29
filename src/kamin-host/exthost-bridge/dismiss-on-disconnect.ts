import { isPeerDisconnected } from "../rpc.js"

/** Завершить закрываемый диалог штатно, если оболочка отсоединилась.
 *
 *  Диалог живёт в окне оболочки: когда соединение рвётся, окна уже нет, и
 *  показывать пользователю нечего. Контракт каждого такого API знает такой
 *  исход — `undefined`/`null` означает «закрыт пользователем», — поэтому
 *  ожидаемая отмена завершает вызов значением, а не отказом.
 *
 *  Любая другая ошибка проходит насквозь: сдерживание падений в ребёнке
 *  обязано по-прежнему видеть настоящие сбои расширений (BR-19). */
export function dismissOnPeerDisconnect<T>(call: Promise<T>, dismissed: T): Promise<T> {
  return call.catch((err: unknown) => {
    if (isPeerDisconnected(err)) return dismissed
    throw err
  })
}
