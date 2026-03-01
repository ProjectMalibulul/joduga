import { create } from "zustand";
import {
  type Edge,
  type Node,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  applyNodeChanges,
  applyEdgeChanges,
  addEdge,
} from "@xyflow/react";
import { invoke } from "@tauri-apps/api/core";
import { NODE_CATALOG } from "./catalog";
import type { AudioNodeData, EngineEdgeInfo, EngineNodeInfo, ParamValue } from "./types";

/* ── helpers ───────────────────────────────────────────────── */

let nextId = 1;
const uid = () => String(nextId++);

/* ── zustand store ─────────────────────────────────────────── */

export interface AppState {
  nodes: Node<AudioNodeData>[];
  edges: Edge[];
  engineRunning: boolean;

  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;

  addNode: (catalogIdx: number, x: number, y: number) => void;
  removeSelected: () => void;
  setParam: (nodeId: string, paramIdx: number, value: number) => void;

  startEngine: () => Promise<void>;
  stopEngine: () => Promise<void>;
}

export const useStore = create<AppState>((set, get) => ({
  nodes: [],
  edges: [],
  engineRunning: false,

  /* React Flow callbacks */
  onNodesChange: (changes) =>
    set({ nodes: applyNodeChanges(changes, get().nodes) as Node<AudioNodeData>[] }),
  onEdgesChange: (changes) =>
    set({ edges: applyEdgeChanges(changes, get().edges) }),
  onConnect: (connection) =>
    set({ edges: addEdge(connection, get().edges) }),

  /* Add a node from the catalog */
  addNode: (catalogIdx, x, y) => {
    const tpl = NODE_CATALOG[catalogIdx];
    if (!tpl) return;
    const id = uid();
    const params: ParamValue[] = tpl.params.map((p) => ({ ...p, value: p.default }));
    const node: Node<AudioNodeData> = {
      id,
      type: "audio",
      position: { x, y },
      data: {
        label: tpl.name,
        icon: tpl.icon,
        color: tpl.color,
        category: tpl.category,
        templateIdx: catalogIdx,
        engineType: tpl.engineType,
        engineSubtype: tpl.engineSubtype,
        numInputs: tpl.numInputs,
        numOutputs: tpl.numOutputs,
        params,
      },
    };
    set({ nodes: [...get().nodes, node] });
  },

  removeSelected: () => {
    const { nodes, edges } = get();
    const selIds = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
    set({
      nodes: nodes.filter((n) => !n.selected),
      edges: edges.filter(
        (e) => !e.selected && !selIds.has(e.source) && !selIds.has(e.target)
      ),
    });
  },

  setParam: (nodeId, paramIdx, value) => {
    set({
      nodes: get().nodes.map((n) => {
        if (n.id !== nodeId) return n;
        const params = [...(n.data as AudioNodeData).params];
        params[paramIdx] = { ...params[paramIdx], value };
        return { ...n, data: { ...(n.data as AudioNodeData), params } };
      }),
    });
    // Send to engine if running
    const { engineRunning, nodes } = get();
    if (!engineRunning) return;
    const nd = nodes.find((n) => n.id === nodeId);
    if (!nd) return;
    const p = (nd.data as AudioNodeData).params[paramIdx];
    invoke("set_param", {
      nodeId: Number(nodeId),
      paramHash: p.hash,
      value,
    }).catch(console.error);
  },

  startEngine: async () => {
    const { nodes, edges } = get();
    const engineNodes: EngineNodeInfo[] = nodes.map((n) => {
      const d = n.data as AudioNodeData;
      return {
        id: Number(n.id),
        engine_type: d.engineType,
        num_inputs: d.numInputs,
        num_outputs: d.numOutputs,
        engine_subtype: d.engineSubtype,
        params: d.params.map((p) => ({ hash: p.hash, value: p.value })),
      };
    });
    const engineEdges: EngineEdgeInfo[] = edges.map((e) => ({
      from_node: Number(e.source),
      from_port: Number(e.sourceHandle?.replace("out-", "") ?? 0),
      to_node: Number(e.target),
      to_port: Number(e.targetHandle?.replace("in-", "") ?? 0),
    }));
    try {
      await invoke("start_engine", { nodes: engineNodes, edges: engineEdges });
      set({ engineRunning: true });
    } catch (e) {
      console.error("start_engine failed", e);
    }
  },

  stopEngine: async () => {
    try {
      await invoke("stop_engine");
      set({ engineRunning: false });
    } catch (e) {
      console.error("stop_engine failed", e);
    }
  },
}));
