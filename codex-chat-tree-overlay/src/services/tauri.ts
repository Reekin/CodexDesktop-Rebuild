import { invoke } from "@tauri-apps/api/core";
import type { OverlaySnapshot, SetCurrentNodeResult } from "../types";

const BROWSER_PREVIEW_SNAPSHOT: OverlaySnapshot = {
  visible: true,
  sessionId: null,
  isCodexFocused: false,
  statusMessage: "Browser preview mode. Launch the Tauri app to attach to Codex Desktop.",
  codexWindowRect: null,
  overlayRect: null,
  chatTree: null,
  helperConnected: false,
  isSwitching: false,
  switchingNodeId: null,
  lastError: null,
};

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function getOverlaySnapshot(): Promise<OverlaySnapshot> {
  if (!isTauriRuntime()) {
    return BROWSER_PREVIEW_SNAPSHOT;
  }
  return invoke<OverlaySnapshot>("get_overlay_snapshot");
}

export async function setCurrentNode(nodeId: string): Promise<SetCurrentNodeResult> {
  if (!isTauriRuntime()) {
    throw new Error("Tauri runtime is not available in browser preview mode.");
  }
  return invoke<SetCurrentNodeResult>("set_current_node", { nodeId });
}

export async function refreshChatTree(): Promise<OverlaySnapshot> {
  if (!isTauriRuntime()) {
    return BROWSER_PREVIEW_SNAPSHOT;
  }
  return invoke<OverlaySnapshot>("refresh_chat_tree");
}
