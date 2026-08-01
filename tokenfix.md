# Fixing `Input tokens exceed the configured limit of 192000`

## 1. What the error means

Your request serialized to **194,652 input tokens**, but the endpoint enforces a **192,000-token input cap**. You're over by **2,652 tokens (~1.4%)** — a marginal overflow, not a fundamentally broken request.

Important: **192k is not the model's real context window.** Claude models advertise 200k. The gateway (OpenRouter or a proxy in front of the provider) has configured 192k, almost certainly to reserve headroom for the output (`max_tokens`) inside the model's true window. So this is a *configured* limit, and "input tokens" means **everything the model reads**: system prompt, tool/function schemas, the entire message history, tool results, pasted files, and images — not just your last user message.

This is an *input/context* problem, not an output/`max_tokens` generation problem. The request was rejected before any tokens were generated.

## 2. Quick fixes (no code)

| Fix | Action |
|---|---|
| **Drop the oldest messages** | You only need ~1.4%; removing the first 1–2 turns usually does it |
| **Truncate the biggest single item** | One pasted log, file dump, or huge `tool_result` is often 50%+ of the payload |
| **Remove attachments/images** | They're resent (and re-billed) on every turn |
| **Start a new thread** | Carry over a hand-written summary instead of full history |
| **Summarize earlier turns** | Collapse them into one compact "conversation so far" message |
| **Reduce `max_tokens`** | Some gateways validate `input + max_tokens ≤ limit` |
| **Switch models** | A 1M-context model (e.g. Gemini 2.5 Pro, or Claude's 1M-context beta) if your gateway exposes it |
| **Raise the gateway limit** | If it's your own proxy or an OpenRouter provider preference, 192k may be a setting — raisable only up to the model's true limit minus output budget |

## 3. Programmatic solutions

### 3a. Count tokens *before* sending

```python
# OpenAI / tiktoken
import tiktoken
enc = tiktoken.encoding_for_model("gpt-4o")
n = len(enc.encode(text))

# Anthropic — exact, server-side, free endpoint
import anthropic
client = anthropic.Anthropic()
resp = client.messages.count_tokens(
    model="claude-sonnet-4-5",
    system=system_prompt,
    tools=tools,            # include these — schemas count!
    messages=messages,
)
print(resp.input_tokens)

# Heuristic fallback: ~4 chars per token
def approx_tokens(text: str) -> int:
    return max(1, len(text) // 4)
```

The 4-chars heuristic is fine for English prose but **underestimates for code, JSON, and non-English text** (sometimes by 2×). Add a 10–20% margin if you rely on it. Local tokenizers will always disagree slightly with the server.

### 3b. Budget-aware sliding-window trimmer

The tricky part is **not splitting tool-call pairs**: an assistant `tool_use` must be followed by its `tool_result`, and a `tool_result` can never appear without its `tool_use`. Orphaned tool messages cause their own 400s. Treat each pair as one atomic unit.

```python
import json
from typing import Callable

def estimate_tokens(msg: dict) -> int:
    """Rough per-message estimate. Swap for tiktoken/count_tokens in prod."""
    content = msg.get("content", "")
    if not isinstance(content, str):
        content = json.dumps(content, default=str)
    return max(1, len(content) // 4) + 4  # +4 for role/formatting overhead

def _has_block(msg: dict, block_type: str) -> bool:
    c = msg.get("content")
    return isinstance(c, list) and any(
        isinstance(b, dict) and b.get("type") == block_type for b in c
    )

def _build_units(convo: list[dict]) -> list[list[dict]]:
    """Group into atomic units, newest-first. A user message containing
    tool_result blocks is bundled with the preceding assistant tool_use msg."""
    units = []
    i = len(convo) - 1
    while i >= 0:
        m = convo[i]
        if _has_block(m, "tool_result") and i > 0 and _has_block(convo[i - 1], "tool_use"):
            units.append([convo[i - 1], m])   # pair kept in original order
            i -= 2
        else:
            units.append([m])
            i -= 1
    return units  # newest first

def trim_messages(
    messages: list[dict],
    max_input_tokens: int,
    reserve_for_output: int = 0,   # set = your max_tokens if the gateway counts it
    counter: Callable[[dict], int] = estimate_tokens,
) -> list[dict]:
    system = [m for m in messages if m.get("role") == "system"]
    convo  = [m for m in messages if m.get("role") != "system"]

    budget = max_input_tokens - reserve_for_output - sum(counter(m) for m in system)
    if budget <= 0:
        raise ValueError("System prompt alone exceeds the input budget.")

    kept, used = [], 0
    for unit in _build_units(convo):          # newest first, pairs atomic
        cost = sum(counter(m) for m in unit)
        if kept and used + cost > budget:     # always keep at least the newest unit
            break
        kept.append(unit)
        used += cost

    kept.reverse()                            # reverse UNITS, not flattened msgs
    trimmed = system + [m for unit in kept for m in unit]

    if sum(counter(m) for m in trimmed) > max_input_tokens - reserve_for_output:
        raise ValueError("Newest message alone exceeds budget; truncate its content.")
    return trimmed
```

Retry pattern — the retry must send a **genuinely smaller** payload:

```python
try:
    response = client.messages.create(model=MODEL, messages=messages, ...)
except BadRequestError as e:
    if "exceed the configured limit" in str(e):
        messages = trim_messages(messages, max_input_tokens=192_000,
                                 reserve_for_output=8_192)
        response = client.messages.create(model=MODEL, messages=messages, ...)
    else:
        raise
```

For OpenAI-style APIs the same logic applies — match `{"role": "tool", "tool_call_id": ...}` messages back to the preceding assistant message's `tool_calls`.

### 3c. Longer-term architectural fixes

- **Rolling summary memory** — when history exceeds ~60–70% of budget, LLM-summarize the oldest half into a single message (`"Summary of earlier discussion: ..."`) and keep the last N turns verbatim. Cheapest route to effectively unlimited memory.
- **RAG instead of stuffing** — embed documents once, retrieve top-k relevant chunks per query, inject only those. Never paste whole files you don't need verbatim.
- **Chunking / map-reduce** — for one huge input (book, log, codebase), split it, process each chunk (summarize/extract), then combine results in a final call.
- **Truncate tool results at the source** — cap `tool_result` size when you *write* it into history (first/last N lines, or ~2,000 tokens), not just at trim time. Historical tool results are resent every turn, so their cost compounds.

### 3d. LangChain note

`langchain_core.messages.utils.trim_messages` does this for you:

```python
from langchain_core.messages.utils import trim_messages

trimmed = trim_messages(
    messages,
    max_tokens=190_000,
    token_counter=model,     # the model's own counter, or count_tokens_approximately
    strategy="last",         # keep the most recent messages
    include_system=True,     # never drop the system prompt
    start_on="human",        # trim boundary lands on a human turn, so tool
                             # tool_use/tool_result pairs stay intact
    allow_partial=False,
)
```

`start_on="human"` and `allow_partial=False` specifically prevent orphaned tool messages — a common source of follow-on 400s.

## 4. Gotchas that bite people

- **The limit is configured, not the model's.** 192k vs 200k is the gateway reserving ~8k for output. Raising it means changing gateway config *and* leaving room for `max_tokens`.
- **`max_tokens` may count at validation time.** 194k input + 8k output = 202k, over even the true 200k window. Test empirically by lowering `max_tokens` to see whether that's the binding constraint.
- **Everything counts:** system prompt, tool/JSON schemas (large schemas run 1–5k tokens each, sometimes 5–10k total), images (Claude bills roughly `width × height / 750` tokens — dimensions, not file size), files, and every historical tool result.
- **Streaming doesn't help.** It changes delivery, not input size.
- **Prompt caching reduces cost and latency, not the limit.** Cached tokens still count toward the context window.
- **Blind retries are futile.** A 400 here is deterministic — backoff won't save you. Shrink, then retry once.
- **Guard the degenerate cases:** system prompt alone over budget, or the newest message alone over budget. In the latter case, dropping turns can't help — you must truncate content.

## 5. What to do first — checklist

1. **Right now:** delete the oldest 2–3 messages (or the single biggest pasted blob) and resend. At 1.4% over, this almost certainly unblocks you.
2. **Verify the real constraint:** lower `max_tokens` and see if the error changes — that tells you whether input+output share the budget.
3. **Audit the payload:** count tokens with the provider's `count_tokens` endpoint including `system` and `tools`; check for stray images/attachments and bloated tool schemas.
4. **If it recurs:** wrap every call in `trim_messages` (yours or LangChain's) with a 5–10% margin under the cap.
5. **If conversations are long-lived:** add rolling summarization. **If documents are large:** move to RAG/chunking.
6. **Only then:** switch to a larger-context model or raise the gateway limit — don't fight the cap if trimming is enough.