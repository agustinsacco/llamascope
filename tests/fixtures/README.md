# Protocol fixtures

These examples contain synthetic identifiers and paths, no real prompts or credentials.

| File | Contract |
| --- | --- |
| `slots-live.json` | Shape of `/slots` from llama-server build `b0-f477f795` (upstream `server_slot::to_json`). Slot 0 is captured mid-prefill; slots 1 and 2 are derived from it to cover a prompt-cache hit while generating and an idle slot. `next_token` is an array, and `n_prompt_tokens` counts every token held in the slot (cache + ingested prompt + generated), so it grows during prefill **and** generation. Processed tokens exclude `n_prompt_tokens_cache`. |
| `slots-object.json` | Compatibility: object-shaped `next_token`, signed `n_remain: -1`, and no prompt counters, as shown in some upstream README examples. Missing counters must stay unknown, never zero. |
| `props.json` | Reduced upstream `/props` shape. Only `model_path` and `build_info` are read; everything else is optional. No queue size exists upstream. |

Loopback HTTP tests compose success, auth, malformed, loading, partial-failure, timeout, oversized-body, and reconnect sequences from these fixtures. Inline rate fixtures vary task identity and token counts independently to detect rates accidentally crossing request boundaries.

When adding a fixture, cite its protocol source and tested server build, and say whether it was captured from a live server or derived from documentation.
