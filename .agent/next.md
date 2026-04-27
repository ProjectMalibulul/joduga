# Loop 13 candidate: audit unwrap_or(0) / unwrap_or_else node-id fallbacks

Loops 7-8 found that silently falling back to node id 0 / "the last
node in the list" masked compile failures of the user's graph. Sweep
the workspace for other unwrap_or / unwrap_or_else / .or(Some(0))
patterns on Result/Option returns of node lookups, file lookups, or
parser dispatches that should be hard errors. List them; pick the
worst one to fix next loop.
