import { useEffect, useRef, useState } from "react";
import { ChatTreeGraph } from "./components/ChatTreeGraph";
import { getOverlaySnapshot, refreshChatTree, setCurrentNode } from "./services/tauri";
import type { OverlaySnapshot } from "./types";

const POLL_INTERVAL_MS = 900;

const EMPTY_SNAPSHOT: OverlaySnapshot = {
  visible: false,
  sessionId: null,
  isCodexFocused: false,
  statusMessage: "Waiting for Codex Desktop...",
  codexWindowRect: null,
  overlayRect: null,
  chatTree: null,
  helperConnected: false,
  isSwitching: false,
  switchingNodeId: null,
  lastError: null,
};

export default function App() {
  const [snapshot, setSnapshot] = useState<OverlaySnapshot>(EMPTY_SNAPSHOT);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    let timer: number | null = null;

    async function tick() {
      try {
        const next = await getOverlaySnapshot();
        if (mountedRef.current) {
          setSnapshot(next);
        }
      } catch (error) {
        if (mountedRef.current) {
          setSnapshot((current) => ({
            ...current,
            lastError: error instanceof Error ? error.message : String(error),
          }));
        }
      } finally {
        if (mountedRef.current) {
          timer = window.setTimeout(tick, POLL_INTERVAL_MS);
        }
      }
    }

    void tick();
    return () => {
      mountedRef.current = false;
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
  }, []);

  async function handleSwitchNode(nodeId: string) {
    const previous = snapshot;
    setSnapshot((current) => ({
      ...current,
      isSwitching: true,
      switchingNodeId: nodeId,
      lastError: null,
    }));
    try {
      await setCurrentNode(nodeId);
      const next = await refreshChatTree();
      if (mountedRef.current) {
        setSnapshot(next);
      }
    } catch (error) {
      if (mountedRef.current) {
        setSnapshot({
          ...previous,
          isSwitching: false,
          switchingNodeId: null,
          lastError: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }

  const nodeCount = snapshot.chatTree?.nodes.length ?? 0;

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <div className="eyebrow">Codex Sidecar</div>
          <h1>Chat Tree</h1>
        </div>
        <button
          type="button"
          className="ghost-button"
          onClick={() => {
            void refreshChatTree().then(setSnapshot).catch((error) => {
              setSnapshot((current) => ({
                ...current,
                lastError: error instanceof Error ? error.message : String(error),
              }));
            });
          }}
        >
          Refresh
        </button>
      </header>

      <section className="status-card">
        <div className="status-row">
          <span className="status-label">Session</span>
          <span className="status-value mono">
            {snapshot.sessionId ? snapshot.sessionId.slice(0, 8) : "none"}
          </span>
        </div>
        <div className="status-row">
          <span className="status-label">Helper</span>
          <span className={`status-badge${snapshot.helperConnected ? " is-ok" : ""}`}>
            {snapshot.helperConnected ? "connected" : "offline"}
          </span>
        </div>
        <div className="status-row">
          <span className="status-label">Nodes</span>
          <span className="status-value">{nodeCount}</span>
        </div>
        <p className="status-message">{snapshot.statusMessage}</p>
        {snapshot.lastError ? <p className="error-message">{snapshot.lastError}</p> : null}
      </section>

      <section className="graph-card">
        <ChatTreeGraph
          tree={snapshot.chatTree}
          isSwitching={snapshot.isSwitching}
          switchingNodeId={snapshot.switchingNodeId}
          onSelectNode={handleSwitchNode}
        />
      </section>

      <footer className="app-footer">
        <span>Double-click a node to switch branch.</span>
        <span>The main Codex app refreshes after branch change.</span>
      </footer>
    </main>
  );
}
