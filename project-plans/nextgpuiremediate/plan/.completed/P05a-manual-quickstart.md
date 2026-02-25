# P05a Manual Quickstart (2 minutes)

Use this exactly to produce human-interactive evidence and unblock P05a.

## 1) Start app

```bash
cd /Users/acoliver/projects/personal-agent/gpuui
pkill -f personal_agent_gpui || true
cargo build --bin personal_agent_gpui
nohup cargo run --bin personal_agent_gpui >/tmp/personal_agent_gpui.log &
```

## 2) Perform 4 required checks in the tray popup

1. **Settings -> Model Selector -> results load**
2. **Profile Editor save path**
3. **MCP add/configure path**
4. **Chat send + stream updates**

## 3) Fill checklist

Open and complete:

```bash
${EDITOR:-vi} /Users/acoliver/projects/personal-agent/gpuui/project-plans/nextgpuiremediate/plan/.completed/P05a-human-checklist.md
```

For each section:
- replace `Actual:` with what you saw
- set `Result: PASS` or `Result: FAIL`
- mark matching checkbox (`- [x] PASS` or `- [x] FAIL`)

## 4) Run gates

```bash
/Users/acoliver/projects/personal-agent/gpuui/project-plans/nextgpuiremediate/plan/.completed/P05a-checklist-lint.sh
/Users/acoliver/projects/personal-agent/gpuui/project-plans/nextgpuiremediate/plan/.completed/P05a-unlock-gate.sh
/Users/acoliver/projects/personal-agent/gpuui/project-plans/nextgpuiremediate/plan/.completed/P05a-status.sh
```

## 5) Success condition

You are unblocked only when:
- checklist lint passes
- unlock gate exits 0
- status reports 4 / 4 PASS sections

Then P05a may be upgraded to PASS and P06 can begin.
