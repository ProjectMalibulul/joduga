# Loop 14 candidate: ShadowGraph DFS fallback (color.get(&next).copied().unwrap_or(0))

shadow_graph.rs:168 has `match color.get(&next).copied().unwrap_or(0)`
inside the cycle-detection DFS. If `next` somehow isn't in the color
map, defaulting to 0 (WHITE) would silently re-traverse it. Audit
whether this is reachable, and either:
  - prove unreachable and replace with .expect(), or
  - prove reachable and define correct behaviour.

Backup: refactor catalog() to be enum-keyed so demo-graph lookups can't
silently miss a renamed template (deeper fix mentioned in loop 13).
