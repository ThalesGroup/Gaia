# Build an Agent From Scratch With Gaia

This tutorial walks from zero to a working LLM agent using `gaia`.
It covers:

- a fast local mock setup
- a production-style backend launch
- minimal Python and JavaScript agent examples

## 1) What You Need

- a Linux/macOS machine or VM
- Docker installed
- NVIDIA drivers + NVIDIA Container Toolkit if using GPU backends
- Rust toolchain (to build `gaia` from source)
- optional: `HF_TOKEN` for gated Hugging Face models

## 2) Build Gaia

From repository root:

```bash
cargo build --release -p gaia-cli
```

Run:

```bash
./target/release/gaia --help
```

Optional install:

```bash
./install.sh
```

## 3) Validate Your Runtime

Run:

```bash
gaia doctor
```

This gives:

- machine capability summary
- backend availability
- model recommendations

## 4) Fastest Path: Mock API

Use this when you want to validate agent wiring before real inference costs.

Start mock API:

```bash
gaia serve --mock --detach --host 0.0.0.0 --port 8000
```

Open:

- API: `http://localhost:8000/v1`
- Health: `http://localhost:8000/health`

Stop mock:

```bash
gaia stop --mock
```

## 5) Real LLM Backend (Production-Like Settings)

Set secrets:

```bash
export GAIA_SECURITY_PROFILE=prod
export GAIA_API_KEY="replace-with-strong-random-key"
export HF_TOKEN="hf_xxx_if_model_is_gated"
```

Launch backend:

```bash
gaia serve \
  --security-profile prod \
  --backend vllm \
  --model Qwen/Qwen2.5-7B-Instruct \
  --model-revision 0123456789abcdef0123456789abcdef01234567 \
  --host 0.0.0.0 \
  --port 8000 \
  --api-key "$GAIA_API_KEY" \
  --detach
```

Check runtime:

```bash
gaia status
gaia logs --lines 200
```

## 6) Build a Minimal Agent (Python)

`gaia` exposes an OpenAI-style base URL. You can use the OpenAI SDK pattern directly.

Install:

```bash
pip install openai
```

Create `agent.py`:

```python
import os
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8000/v1",
    api_key=os.environ["GAIA_API_KEY"],
)

messages = [{"role": "system", "content": "You are a concise assistant."}]

print("Agent ready. Type 'exit' to quit.")
while True:
    user = input("you> ").strip()
    if user.lower() in {"exit", "quit"}:
        break
    messages.append({"role": "user", "content": user})
    response = client.chat.completions.create(
        model="Qwen/Qwen2.5-7B-Instruct",
        messages=messages,
        temperature=0.2,
    )
    answer = response.choices[0].message.content
    print(f"agent> {answer}")
    messages.append({"role": "assistant", "content": answer})
```

Run:

```bash
export GAIA_API_KEY="same-key-used-by-gaia-serve"
python3 agent.py
```

## 7) Build a Minimal Agent (JavaScript)

Install:

```bash
npm install openai
```

Create `agent.mjs`:

```javascript
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "http://localhost:8000/v1",
  apiKey: process.env.GAIA_API_KEY,
});

const messages = [{ role: "system", content: "You are a concise assistant." }];
const question = process.argv.slice(2).join(" ") || "Give me a one-line health check.";
messages.push({ role: "user", content: question });

const response = await client.chat.completions.create({
  model: "Qwen/Qwen2.5-7B-Instruct",
  messages,
  temperature: 0.2,
});

console.log(response.choices[0].message.content);
```

Run:

```bash
export GAIA_API_KEY="same-key-used-by-gaia-serve"
node agent.mjs "Summarize this deployment in one sentence."
```

## 8) Multi-VM Architecture (Recommended for Scale)

For production-like setups:

1. Run `gaia` on worker VMs to manage model runtimes.
2. Put a gateway/control plane in front (for example LiteLLM).
3. Point all apps and agents to one gateway URL.

This gives centralized routing, failover, and policy controls while keeping `gaia` focused on runtime orchestration.

## 9) Troubleshooting Checklist

- `gaia doctor` first
- if detached service is not answering: `gaia status` then `gaia logs --lines 200`
- if model is gated: verify `HF_TOKEN`
- if using `prod`: ensure explicit `GAIA_API_KEY`
- if benchmark/debug is needed: `gaia benchmark --requests 10`

## 10) Next Steps

- run `gaia generate-compose` or `gaia generate-k8s` for reproducible deployment artifacts (review before apply)
- refresh the catalog with `gaia catalog refresh` and `gaia catalog promote`
- add CI smoke tests for your agent against the Gaia endpoint
- for multi-VM setups, see `docs/deployment-patterns.md` (LiteLLM gateway pattern)
