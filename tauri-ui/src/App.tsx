import { useState, useCallback, useMemo, useRef, type DragEvent } from "react";
import {
    ReactFlow,
    Background,
    Controls,
    MiniMap,
    Panel,
    type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { useStore } from "./store";
import { NODE_CATALOG, CATEGORIES } from "./catalog";
import AudioNode from "./AudioNode";
import "./styles.css";

const nodeTypes: NodeTypes = { audio: AudioNode };

export default function App() {
    const {
        nodes,
        edges,
        onNodesChange,
        onEdgesChange,
        onConnect,
        addNode,
        removeSelected,
        startEngine,
        stopEngine,
        engineRunning,
    } = useStore();

    const reactFlowWrapper = useRef<HTMLDivElement>(null);
    const [search, setSearch] = useState("");
    const [openCat, setOpenCat] = useState<string | null>(null);

    /* keyboard shortcuts */
    const onKeyDown = useCallback(
        (e: React.KeyboardEvent) => {
            if (e.key === "Delete" || e.key === "Backspace") removeSelected();
        },
        [removeSelected]
    );

    /* drag-and-drop from sidebar */
    const onDragOver = useCallback((e: DragEvent) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
    }, []);

    const onDrop = useCallback(
        (e: DragEvent) => {
            e.preventDefault();
            const idx = Number(e.dataTransfer.getData("application/joduga-node"));
            if (isNaN(idx)) return;
            const bounds = reactFlowWrapper.current?.getBoundingClientRect();
            if (!bounds) return;
            addNode(idx, e.clientX - bounds.left - 80, e.clientY - bounds.top - 20);
        },
        [addNode]
    );

    /* filtered catalog */
    const filtered = useMemo(() => {
        const q = search.toLowerCase();
        return NODE_CATALOG.map((t, i) => ({ ...t, idx: i })).filter(
            (t) =>
                (!q || t.name.toLowerCase().includes(q) || t.category.toLowerCase().includes(q)) &&
                (!openCat || t.category === openCat)
        );
    }, [search, openCat]);

    return (
        <div className="app" onKeyDown={onKeyDown} tabIndex={0}>
            {/* ── sidebar ────────────────────────────────────────── */}
            <aside className="sidebar">
                <h2 className="sidebar-title">Joduga</h2>

                {/* transport */}
                <div className="transport">
                    <button
                        className={`btn ${engineRunning ? "btn-stop" : "btn-play"}`}
                        onClick={engineRunning ? stopEngine : startEngine}
                    >
                        {engineRunning ? "Stop" : "Play"}
                    </button>
                </div>

                {/* search */}
                <input
                    className="search"
                    type="text"
                    placeholder="Search nodes..."
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                />

                {/* categories */}
                <div className="categories">
                    <button
                        className={`cat-btn ${openCat === null ? "active" : ""}`}
                        onClick={() => setOpenCat(null)}
                    >
                        All
                    </button>
                    {CATEGORIES.map((c) => (
                        <button
                            key={c}
                            className={`cat-btn ${openCat === c ? "active" : ""}`}
                            onClick={() => setOpenCat(openCat === c ? null : c)}
                        >
                            {c}
                        </button>
                    ))}
                </div>

                {/* node list */}
                <div className="node-list">
                    {filtered.map((t) => (
                        <div
                            key={t.idx}
                            className="catalog-item"
                            style={{ borderLeftColor: t.color }}
                            draggable
                            onDragStart={(e) =>
                                e.dataTransfer.setData("application/joduga-node", String(t.idx))
                            }
                            onDoubleClick={() => addNode(t.idx, 200 + Math.random() * 200, 100 + Math.random() * 200)}
                        >
                            <span className="catalog-icon">{t.icon}</span>
                            <span className="catalog-name">{t.name}</span>
                        </div>
                    ))}
                </div>
            </aside>

            {/* ── canvas ─────────────────────────────────────────── */}
            <div className="canvas" ref={reactFlowWrapper}>
                <ReactFlow
                    nodes={nodes}
                    edges={edges}
                    onNodesChange={onNodesChange}
                    onEdgesChange={onEdgesChange}
                    onConnect={onConnect}
                    onDrop={onDrop}
                    onDragOver={onDragOver}
                    nodeTypes={nodeTypes}
                    fitView
                    deleteKeyCode={null}
                    proOptions={{ hideAttribution: true }}
                >
                    <Background gap={20} />
                    <Controls />
                    <MiniMap
                        nodeColor={(n) => (n.data as any)?.color ?? "#777"}
                        maskColor="rgba(0,0,0,0.6)"
                    />
                    <Panel position="top-right">
                        <span className={`engine-badge ${engineRunning ? "on" : "off"}`}>
                            {engineRunning ? "Engine ON" : "Engine OFF"}
                        </span>
                    </Panel>
                </ReactFlow>
            </div>
        </div>
    );
}
