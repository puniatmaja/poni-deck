# GitHub Copilot CLI hook: agent-status.ps1
# Reads hook JSON from stdin, resolves the copilot agent PID (the parent of
# this hook process), and writes a status file consumed by Poni Deck at
#   %APPDATA%\poni-deck\agents\{copilotPid}.json
# Mirrors packages/claude-poni-deck/hooks/agent-status.ps1 and
# packages/opencode-poni-deck/src/index.ts. Returns no decision; pure side effect.

$ErrorActionPreference = 'SilentlyContinue'

# --- ordering seq: process creation time ~= hook event emission order ---
# Copilot CLI hooks are synchronous; every event spawns its own pwsh.exe, so
# completion order can differ from event order. seq lets Poni Deck ignore a
# write from an older event (e.g. a late PermissionRequest clobbering the Stop
# of a denied tool call).
$seq = [System.Diagnostics.Process]::GetCurrentProcess().StartTime.ToUniversalTime().ToFileTimeUtc()

# --- read hook input from stdin ---
$raw = ''
try { $raw = [Console]::In.ReadToEnd() } catch { }
if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }

$evt = $null
try { $evt = $raw | ConvertFrom-Json } catch { }
if ($null -eq $evt) { exit 0 }

$hookEvent = [string]$evt.hook_event_name
$cwd = [string]$evt.cwd
$toolName = [string]$evt.tool_name
$notifType = [string]$evt.notification_type

if ([string]::IsNullOrWhiteSpace($cwd)) { $cwd = (Get-Location).Path }

# --- map event -> status ---
# PreToolUse/PostToolUse are intentionally NOT registered (see hooks/hooks.json
# and README): Copilot CLI runs command hooks synchronously and preToolUse is
# fail-closed, so firing on every tool call would add latency and could deny
# tool calls if the hook ever errored. UserPromptSubmit -> Stop already tracks
# working/idle per turn.
$statusMap = @{
    'SessionStart'      = 'idle'
    'UserPromptSubmit'  = 'working'
    'PermissionRequest' = 'waiting_confirmation'
    'Stop'              = 'idle'
    'ErrorOccurred'     = 'error'
}
$status = $statusMap[$hookEvent]
if ($hookEvent -eq 'Notification') {
    # notification hooks are fire-and-forget (never block the session)
    if ($notifType -in @('permission_prompt', 'elicitation_dialog')) {
        $status = 'waiting_confirmation'
    } else {
        $status = 'idle'
    }
}

# --- resolve the copilot agent PID by walking the parent chain ---
# The CLI is a Node.js app; the process may be `copilot`, `copilot.exe`, or
# `node.exe` whose command line contains "copilot".
$agentPid = $null
$curPid = $PID
for ($i = 0; $i -lt 10; $i++) {
    $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$curPid" -ErrorAction SilentlyContinue
    if ($null -eq $proc) { break }
    $name = [string]$proc.Name
    if ($name -like 'copilot*' -or ($name -like 'node*' -and ([string]$proc.CommandLine) -match 'copilot')) {
        $agentPid = [int]$proc.ProcessId
        break
    }
    $parentPid = [int]$proc.ParentProcessId
    if ($parentPid -le 0 -or $parentPid -eq $curPid) { break }
    $curPid = $parentPid
}
if ($null -eq $agentPid) { exit 0 }

$agentsDir = Join-Path $env:APPDATA 'poni-deck\agents'
$target = Join-Path $agentsDir "$agentPid.json"

# --- SessionEnd: drop the status file (monitor also cleans stale files) ---
if ($hookEvent -eq 'SessionEnd') {
    try { Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue } catch { }
    exit 0
}

if ($null -eq $status) { exit 0 }

# --- launcher: Copilot CLI is terminal-native; detect VS Code terminals ---
$launcher = 'terminal'
if ($env:TERM_PROGRAM -eq 'vscode') { $launcher = 'vscode' }

# --- ordering guard + skip if nothing changed ---
try {
    if (Test-Path -LiteralPath $target) {
        $old = Get-Content -LiteralPath $target -Raw | ConvertFrom-Json
        $oldSeq = $null
        if ($null -ne $old.seq) { $oldSeq = [long]$old.seq }
        # A newer event already wrote its status; drop this stale write.
        if ($null -ne $oldSeq -and $oldSeq -gt $seq) { exit 0 }
        if ([string]$old.status -eq $status -and [string]$old.cwd -eq $cwd) { exit 0 }
    }
} catch { }

$payload = @{
    status    = $status
    pid       = $agentPid
    cwd       = $cwd
    launcher  = $launcher
    tool      = 'copilot'
    seq       = $seq
    timestamp = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json -Compress

# --- atomic write: unique tmp -> target ---
try {
    New-Item -ItemType Directory -Force -Path $agentsDir | Out-Null
    $tmp = "$target.$PID.tmp"
    [System.IO.File]::WriteAllText($tmp, $payload)
    [System.IO.File]::Delete($target)
    [System.IO.File]::Move($tmp, $target)
} catch { }
