# Claude Code hook: agent-status.ps1
# Reads hook JSON from stdin, resolves the claude process PID (the parent of
# this hook process), and writes a status file consumed by Poni Deck at
#   %APPDATA%\poni-deck\agents\{claudePid}.json
# Mirrors .opencode/plugins/agent-status.ts. Returns no decision; pure side effect.

$ErrorActionPreference = 'SilentlyContinue'

# --- ordering seq: process creation time ≈ hook event emission order ---
# Hooks are async; each event spawns its own powershell.exe, so completion
# order can differ from event order (a stale write may land after a newer
# one). seq lets us ignore a write from an older event (e.g. a late
# PermissionRequest clobbering the Stop of a denied tool call).
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

# --- DEBUG: log every hook event to inspect the deny flow ---
try {
    $debugLog = Join-Path $env:APPDATA 'poni-deck\hooks-debug.log'
    $dbgLine = "{0}  event={1}  tool={2}  pid={3}  tool_input={4}" -f `
        (Get-Date).ToString('HH:mm:ss.fff'), $hookEvent, $toolName, $PID, `
        ([string]($evt.tool_input | ConvertTo-Json -Compress -ErrorAction SilentlyContinue))
    Add-Content -LiteralPath $debugLog -Value $dbgLine -ErrorAction SilentlyContinue
} catch { }

if ([string]::IsNullOrWhiteSpace($cwd)) { $cwd = $env:CLAUDE_PROJECT_DIR }
if ([string]::IsNullOrWhiteSpace($cwd)) { $cwd = (Get-Location).Path }

# --- map event -> status ---
$statusMap = @{
    'SessionStart'      = 'idle'
    'UserPromptSubmit'  = 'working'
    'PreToolUse'        = 'working'
    'PostToolUse'       = 'working'
    'PermissionRequest' = 'waiting_confirmation'
    'Stop'              = 'idle'
    'Notification'      = 'idle'
    'StopFailure'       = 'error'
}
$status = $statusMap[$hookEvent]
if ($hookEvent -eq 'PreToolUse' -and $toolName -in @('AskUserQuestion', 'Question')) {
    $status = 'waiting_confirmation'
}

# --- resolve the claude agent PID by walking the parent chain ---
$agentPid = $null
$curPid = $PID
for ($i = 0; $i -lt 5; $i++) {
    $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$curPid" -ErrorAction SilentlyContinue
    if ($null -eq $proc) { break }
    if ([string]$proc.Name -like 'claude*') { $agentPid = [int]$proc.ProcessId; break }
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

# --- launcher: prefer the claude session file's entrypoint ---
$launcher = 'terminal'
$sessionPath = Join-Path (Join-Path $env:USERPROFILE '.claude\sessions') "$agentPid.json"
if (Test-Path -LiteralPath $sessionPath) {
    try {
        $sess = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
        if ([string]$sess.entrypoint -like '*vscode*') { $launcher = 'vscode' }
    } catch { }
}
if ($launcher -eq 'terminal' -and $env:TERM_PROGRAM -eq 'vscode') { $launcher = 'vscode' }

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
    tool      = 'claude'
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
