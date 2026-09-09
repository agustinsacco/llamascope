# Protocol fixtures

These examples contain synthetic identifiers and paths, no real prompts or credentials.

| File | Contract |
| --- | --- |
| `slots-current.json` | Reduced form of the object-shaped `next_token` and signed `n_remain: -1` in the [upstream `/slots` reference](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md#get-slots-returns-the-current-slots-processing-state). Prompt counters intentionally omitted: current examples do not promise them. |
| `slots-legacy.json` | Regression coverage for the array-shaped `next_token` expected by the original LlamaScope adapter. This is a compatibility fixture, not a claim that current upstream emits arrays. |
| `props.json` | Reduced upstream `/props` shape: total slots, model path, build, and optional metadata. No queue size is present; missing queue is unknown, not zero. |

Loopback HTTP tests compose success, auth, malformed, loading, partial-failure, timeout, oversized-body, and reconnect sequences from these fixtures. Inline rate fixtures vary task identity and token counts independently to detect rates accidentally crossing request boundaries.

When adding a fixture, cite its protocol source and tested server version if available. These fixtures were checked against the upstream documentation, **not captured from a live llama-server during this change**. Keep that distinction in validation reports.
