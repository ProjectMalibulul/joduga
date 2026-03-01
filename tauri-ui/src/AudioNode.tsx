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
      style={{ borderColor: d.color }}
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

/* ── Param slider (extracted to avoid full-node re-renders) ── */

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

const ParamSlider = memo(
  ({ nodeId, idx, name, value, min, max, suffix, log, setParam }: ParamSliderProps) => {
    const onChange = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        let v = Number(e.target.value);
        if (log) {
          // slider position [0,1] → log-scale value
          const t = v / 1000;
          v = min * Math.pow(max / min, t);
        }
        setParam(nodeId, idx, v);
      },
      [nodeId, idx, min, max, log, setParam]
    );

    // Convert current value to slider position
    let sliderVal: number;
    if (log && min > 0) {
      sliderVal = (Math.log(value / min) / Math.log(max / min)) * 1000;
    } else {
      sliderVal = log ? 500 : value;
    }

    const display = value >= 1000 ? `${(value / 1000).toFixed(1)}k` : value.toFixed(value < 1 ? 3 : 1);

    return (
      <div className="param-row">
        <label className="param-label">{name}</label>
        <input
          type="range"
          className="param-slider"
          min={log ? 0 : min}
          max={log ? 1000 : max}
          step={log ? 1 : (max - min) / 200}
          value={sliderVal}
          onChange={onChange}
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
