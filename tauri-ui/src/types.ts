/** Shared TypeScript types for Joduga */

export interface ParamDef {
    name: string;
    hash: number;
    min: number;
    max: number;
    default: number;
    log: boolean;
    suffix: string;
}

export interface NodeTemplate {
    name: string;
    category: string;
    icon: string;
    color: string;
    numInputs: number;
    numOutputs: number;
    engineType: "Oscillator" | "Filter" | "Gain" | "Output";
    /** Sub-type index sent to C++ (waveform type or filter mode) */
    engineSubtype: number;
    params: ParamDef[];
}

/** Data stored on each React Flow node */
export interface AudioNodeData {
    [key: string]: unknown;
    label: string;
    icon: string;
    color: string;
    category: string;
    templateIdx: number;
    engineType: string;
    engineSubtype: number;
    numInputs: number;
    numOutputs: number;
    params: ParamValue[];
}

export interface ParamValue extends ParamDef {
    value: number;
}

/* Types sent to the Tauri backend */
export interface EngineNodeInfo {
    id: number;
    engine_type: string;
    num_inputs: number;
    num_outputs: number;
    params: { hash: number; value: number }[];
    engine_subtype: number;
}

export interface EngineEdgeInfo {
    from_node: number;
    from_port: number;
    to_node: number;
    to_port: number;
}
