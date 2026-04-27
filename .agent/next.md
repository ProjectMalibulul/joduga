# Next loop seed

**Loop 4 candidates:**

1. **`tauri-ui/src-tauri/src/main.rs::parse_engine_type`** silently maps
   unknown strings to `NodeType::Gain`. A frontend bug becomes a silent
   wrong-engine-type bug in the running graph. Fix: return
   `Result<NodeType, String>` and propagate the error through the IPC
   command so the UI surfaces it.
2. ui_main.rs `output_node_id = nodes.len() + 1` (egui-ui only). UX-coupled.
3. C++ multi-output bug (latent).
4. Broader FFI ABI tests (NodeDesc / NodeConnection / CompiledGraph).

Pick (1) — clearest correctness fix in the same priority tier; touches
only the Tauri binary and a single helper; testable without the GUI.
