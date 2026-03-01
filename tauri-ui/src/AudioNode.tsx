import { memo, useCallback } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { AudioNodeData } from "./types";
import { useStore } from "./store";

/** Custom React Flow node: renders ports, label, and param sliders. */
const AudioNode = memo(({ id, data, selected }: NodeProps) => {
    const d = data as AudioNodeData;
    const setParam = useStore((s) => s.setParam);

    return (
        <div
            className={`audio-node ${selected ? "selected" : ""}`}
            style={{ borderColor: d.color, width: 200 }}
        >
            {/* Input handles */}
            {Array.from({ length: d.numInputs }, (_, i) => (
                <Handle
                    key={`in-${i}`}
                    id={`in-${i}`}
                    type="target"
                    position={Position.Left}
                    style={{ top: `${((i + 1) / (d.numInputs + 1)) * 100}%` }}
                />
            ))}

            {/* Header */}
            <div className="node-header" style={{ background: d.color }}>
                <span className="node-icon">{d.icon}</span>
                <span className="node-label">{d.label}</span>
            </div>

            {/* Params */}
            {d.params.length > 0 && (
                <div className="node-params">
                    {d.params.map((p, i) => (
                        <ParamSlider
                            key={p.hash}
                            nodeId={id}
                            idx={i}
                            name={p.name}
                            value={p.value}
                            min={p.min}
                            max={p.max}
                            suffix={p.suffix}
                            log={p.log}
                            setParam={setParam}
                        />
                    ))}
                </div>
            )}

            {/* Output handles */}
            {Array.from({ length: d.numOutputs }, (_, i) => (
                <Handle
                    key={`out-${i}`}
                    id={`out-${i}`}
                    type="source"
                    position={Position.Right}
                    style={{ top: `${((i + 1) / (d.numOutputs + 1)) * 100}%` }}
                />
            ))}
        </div>
    );
});

AudioNode.displayName = "AudioNode";
export default AudioNode;

/* -- Param slider (extracted to avoid full-node re-renders) -- */

interface ParamSliderProps {
    nodeId: string;
    idx: number;
    name: string;
    value: number;
    min: number;
    max: number;
    suffix: string;
    log: boolean;
    setParam: (nodeId: string, idx: number, value: number) => void;
}

const SLIDER_STEPS = 1000;

const ParamSlider = memo(
    ({ nodeId, idx, name, value, min, max, suffix, log, setParam }: ParamSliderProps) => {
        const onChange = useCallback(
            (e: React.ChangeEvent<HTMLInputElement>) => {
                const raw = Number(e.target.value);
                let v: number;
                if (log && min > 0) {
                    // slider [0..SLIDER_STEPS] -> log-scale value
                    const t = raw / SLIDER_STEPS;
                    v = min * Math.pow(max / min, t);
                } else {
                    // slider [0..SLIDER_STEPS] -> linear value
                    const t = raw / SLIDER_STEPS;
                    v = min + (max - min) * t;
                }
                setParam(nodeId, idx, v);
            },
            [nodeId, idx, min, max, log, setParam]
        );

        // Convert current value to slider position [0..SLIDER_STEPS]
        let sliderVal: number;
        if (log && min > 0) {
            const clamped = Math.max(min, Math.min(max, value));
            sliderVal = (Math.log(clamped / min) / Math.log(max / min)) * SLIDER_STEPS;
        } else {
            const range = max - min;
            sliderVal = range > 0 ? ((value - min) / range) * SLIDER_STEPS : 0;
        }
        sliderVal = Math.round(Math.max(0, Math.min(SLIDER_STEPS, sliderVal)));

        const display =
            Math.abs(value) >= 1000
                ? `${(value / 1000).toFixed(1)}k`
                : value.toFixed(Math.abs(value) < 1 ? 3 : 1);

        return (
            <div className="param-row">
                <label className="param-label">{name}</label>
                <input
                    type="range"
                    className="param-slider nodrag nopan"
                    min={0}
                    max={SLIDER_STEPS}
                    step={1}
                    value={sliderVal}
                    onChange={onChange}
                    onPointerDown={(e) => e.stopPropagation()}
                />
                <span className="param-value">
                    {display}
                    {suffix ? ` ${suffix}` : ""}
                </span>
            </div>
        );
    }
);

ParamSlider.displayName = "ParamSlider";
