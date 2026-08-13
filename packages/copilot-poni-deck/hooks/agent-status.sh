#!/usr/bin/env bash
# GitHub Copilot CLI hook: agent-status.sh
# Reads hook JSON from stdin, resolves the copilot agent PID (the parent of
# this hook process), and writes a status file consumed by Poni Deck at
#   %APPDATA%\poni-deck\agents\{copilotPid}.json   (Windows / APPDATA set)
#   $XDG_CONFIG_HOME/poni-deck/agents/{copilotPid}.json  (otherwise)
# Mirrors hooks/agent-status.ps1. Returns no decision; pure side effect.
#
# Requires `jq`. Install it on macOS with `brew install jq` if needed; hooks
# silently no-op when jq is missing.

set -euo pipefail

command -v jq >/dev/null 2>&1 || exit 0

# --- ordering seq: epoch (nanoseconds on GNU date, seconds on BSD) ---
if date -d '0' >/dev/null 2>&1; then
    seq="$(date +%s%N)"
else
    seq="$(date +%s)"
fi

# --- read hook input from stdin ---
raw=""
if [ ! -t 0 ]; then
    raw="$(cat)"
fi
[ -n "$raw" ] || exit 0

hook_event="$(printf '%s' "$raw" | jq -r '.hook_event_name // empty' 2>/dev/null)"
cwd="$(printf '%s' "$raw" | jq -r '.cwd // empty' 2>/dev/null)"
tool_name="$(printf '%s' "$raw" | jq -r '.tool_name // empty' 2>/dev/null)"
notification_type="$(printf '%s' "$raw" | jq -r '.notification_type // empty' 2>/dev/null)"
[ -n "$cwd" ] || cwd="$(pwd)"

# --- map event -> status (PreToolUse/PostToolUse intentionally not registered) ---
status=""
case "$hook_event" in
    SessionStart) status="idle" ;;
    UserPromptSubmit) status="working" ;;
    PermissionRequest) status="waiting_confirmation" ;;
    Stop) status="idle" ;;
    ErrorOccurred) status="error" ;;
    Notification)
        case "$notification_type" in
            permission_prompt|elicitation_dialog) status="waiting_confirmation" ;;
            *) status="idle" ;;
        esac
        ;;
esac

# --- resolve the copilot agent PID by walking the /proc parent chain ---
agent_pid=""
cur="$PPID"
for _ in $(seq 1 10); do
    [ -n "$cur" ] && [ -d "/proc/$cur" ] || break
    cmdline="$(tr '\0' ' ' < "/proc/$cur/cmdline" 2>/dev/null || true)"
    base="$(basename "${cmdline%% *}" 2>/dev/null || true)"
    case "$base" in
        copilot*|node*)
            agent_pid="$cur"
            break
            ;;
    esac
    ppid="$(awk '/^PPid:/{print $2}' "/proc/$cur/status" 2>/dev/null || true)"
    [ -n "$ppid" ] && [ "$ppid" != "$cur" ] || break
    cur="$ppid"
done
[ -n "$agent_pid" ] || exit 0

# --- status file location ---
if [ -n "${APPDATA:-}" ]; then
    agents_dir="$APPDATA/poni-deck/agents"
else
    cfg="${XDG_CONFIG_HOME:-$HOME/.config}"
    agents_dir="$cfg/poni-deck/agents"
fi
target="$agents_dir/$agent_pid.json"

# --- SessionEnd: drop the status file (monitor also cleans stale files) ---
if [ "$hook_event" = "SessionEnd" ]; then
    rm -f -- "$target" 2>/dev/null || true
    exit 0
fi
[ -n "$status" ] || exit 0

# --- launcher: Copilot CLI is terminal-native; detect VS Code terminals ---
launcher="terminal"
if [ "${TERM_PROGRAM:-}" = "vscode" ]; then
    launcher="vscode"
fi

# --- ordering guard + skip if nothing changed ---
if [ -f "$target" ]; then
    old_seq="$(jq -r '.seq // empty' "$target" 2>/dev/null || echo '')"
    old_status="$(jq -r '.status // empty' "$target" 2>/dev/null || echo '')"
    old_cwd="$(jq -r '.cwd // empty' "$target" 2>/dev/null || echo '')"
    if [ -n "$old_seq" ] && [ "$old_seq" -gt "$seq" ]; then exit 0; fi
    if [ "$old_status" = "$status" ] && [ "$old_cwd" = "$cwd" ]; then exit 0; fi
fi

mkdir -p "$agents_dir" 2>/dev/null || true
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
payload="$(jq -nc \
    --arg status "$status" \
    --argjson pid "$agent_pid" \
    --arg cwd "$cwd" \
    --arg launcher "$launcher" \
    --argjson seq "$seq" \
    --arg ts "$ts" \
    '{status:$status,pid:$pid,cwd:$cwd,launcher:$launcher,tool:"copilot",seq:$seq,timestamp:$ts}')"

# --- atomic write: unique tmp -> target ---
tmp="$target.$$.tmp"
printf '%s' "$payload" > "$tmp" 2>/dev/null || true
mv -f -- "$tmp" "$target" 2>/dev/null || rm -f -- "$tmp"
exit 0
