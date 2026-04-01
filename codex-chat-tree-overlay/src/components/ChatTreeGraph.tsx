import type { ThreadChatTree } from "../types";
import { buildThreadChatTreeGraph } from "../utils/chatTreeGraph";

const DEPTH_GAP = 74;
const LANE_GAP = 58;
const GRAPH_PADDING_X = 26;
const GRAPH_PADDING_Y = 20;
const NODE_RADIUS = 7;
const CONNECTOR_CURVE_OFFSET = 24;

type ChatTreeGraphProps = {
  tree: ThreadChatTree | null;
  isSwitching: boolean;
  switchingNodeId: string | null;
  onSelectNode: (nodeId: string) => void | Promise<void>;
};

function shortText(value: string | null) {
  if (!value) {
    return "Untitled node";
  }
  return value.length > 48 ? `${value.slice(0, 48)}...` : value;
}

function laneX(lane: number) {
  return GRAPH_PADDING_X + lane * LANE_GAP;
}

function depthY(depth: number) {
  return GRAPH_PADDING_Y + depth * DEPTH_GAP;
}

export function ChatTreeGraph({
  tree,
  isSwitching,
  switchingNodeId,
  onSelectNode,
}: ChatTreeGraphProps) {
  const graph = buildThreadChatTreeGraph(tree);
  const graphNodeById = new Map(graph.nodes.map((entry) => [entry.node.nodeId, entry]));
  const graphWidth =
    graph.laneCount > 0
      ? GRAPH_PADDING_X * 2 + Math.max(0, graph.laneCount - 1) * LANE_GAP
      : GRAPH_PADDING_X * 2;
  const graphHeight =
    graph.depthCount > 0
      ? GRAPH_PADDING_Y * 2 + Math.max(0, graph.depthCount - 1) * DEPTH_GAP
      : GRAPH_PADDING_Y * 2;

  if (graph.nodes.length === 0) {
    return <div className="empty-state">No branch nodes yet.</div>;
  }

  return (
    <div
      className="graph-shell"
      style={{
        minWidth: `${graphWidth}px`,
        minHeight: `${graphHeight}px`,
      }}
    >
      <svg
        className="graph-svg"
        width={graphWidth}
        height={graphHeight}
        viewBox={`0 0 ${graphWidth} ${graphHeight}`}
        aria-hidden="true"
      >
        {graph.edges.map((edge) => {
          const fromNode = graphNodeById.get(edge.fromNodeId);
          const toNode = graphNodeById.get(edge.toNodeId);
          if (!fromNode || !toNode) {
            return null;
          }

          const fromX = laneX(fromNode.lane);
          const fromY = depthY(fromNode.depth);
          const toX = laneX(toNode.lane);
          const toY = depthY(toNode.depth);
          const verticalGap = toY - fromY;
          const splitY = fromY + Math.min(verticalGap * 0.5, CONNECTOR_CURVE_OFFSET);
          const path =
            Math.abs(fromX - toX) < 0.5
              ? `M ${fromX} ${fromY + NODE_RADIUS} L ${toX} ${toY - NODE_RADIUS}`
              : `M ${fromX} ${fromY + NODE_RADIUS} C ${fromX} ${splitY} ${toX} ${splitY} ${toX} ${toY - NODE_RADIUS}`;

          return (
            <path
              key={`${edge.fromNodeId}->${edge.toNodeId}`}
              className="graph-connector"
              d={path}
            />
          );
        })}
      </svg>
      {graph.nodes.map((entry) => {
        const busy = isSwitching && switchingNodeId !== entry.node.nodeId;
        return (
          <button
            key={entry.node.nodeId}
            type="button"
            className={`graph-node${entry.isCurrent ? " is-current" : ""}${
              switchingNodeId === entry.node.nodeId ? " is-switching" : ""
            }`}
            style={{
              left: `${laneX(entry.lane)}px`,
              top: `${depthY(entry.depth)}px`,
            }}
            disabled={busy}
            onDoubleClick={() => {
              void onSelectNode(entry.node.nodeId);
            }}
            title={`${shortText(entry.node.summary ?? entry.node.turnId ?? entry.node.nodeId)}${
              entry.isCurrent ? "\nCurrent branch." : "\nDouble-click to switch."
            }`}
            aria-label={shortText(entry.node.summary ?? entry.node.turnId ?? entry.node.nodeId)}
          >
            <span className="graph-node-dot" />
          </button>
        );
      })}
    </div>
  );
}
