export type WindowRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

export type ThreadChatTreeNode = {
  nodeId: string;
  parentNodeId: string | null;
  summary: string | null;
  turnId: string | null;
  order: number;
};

export type ThreadChatTree = {
  currentNodeId: string | null;
  nodes: ThreadChatTreeNode[];
};

export type OverlaySnapshot = {
  visible: boolean;
  sessionId: string | null;
  isCodexFocused: boolean;
  statusMessage: string;
  codexWindowRect: WindowRect | null;
  overlayRect: WindowRect | null;
  chatTree: ThreadChatTree | null;
  helperConnected: boolean;
  isSwitching: boolean;
  switchingNodeId: string | null;
  lastError: string | null;
};

export type SetCurrentNodeResult = {
  currentNodeId: string;
  refreshTriggered: boolean;
};
