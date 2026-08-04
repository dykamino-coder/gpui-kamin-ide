// Which session the CHAT IS ACTUALLY SHOWING, as opposed to which one is active.
//
// `activeTabId` flips the instant a session is clicked, but the transcript below
// does not: a big session has to replay first, and while the main thread is busy
// doing that the screen still holds the previous session's pixels. The composer
// read `activeTabId` and sent there, so a message typed against what was visibly
// the OLD conversation was delivered to the NEW one — the user was looking at one
// session and writing into another. That is a wrong-recipient bug, not a
// cosmetic lag, so the composer needs to know when the view and the target have
// actually converged.
import { signal, computed } from '@preact/signals'
import { activeTabId } from './tabs'

/** The tab whose transcript the viewer currently has on screen. `null` before
 *  the first bind. Written by JsonlViewer, which is the only thing that knows
 *  what it has painted. */
export const boundTabId = signal<string | null>(null)

/** True while the shown conversation is NOT the active session's — i.e. the
 *  window in which anything typed would land somewhere the user cannot see. */
export const sessionBindingPending = computed(
  () => activeTabId.value !== null && boundTabId.value !== activeTabId.value,
)
