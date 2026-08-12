import type { KaminBridgeApi } from "../../shared/types";

type SessionSubmitBridge = Pick<KaminBridgeApi, "submitText">;

/** Slash commands triggered outside the composer must use the same serialized
 * submit path as normal chat messages. */
export function submitSessionRename(
  bridge: SessionSubmitBridge | null | undefined,
  tabId: string,
): void {
  bridge?.submitText(tabId, "/rename");
}
