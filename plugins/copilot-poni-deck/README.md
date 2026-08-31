# copilot-poni-deck

GitHub Copilot CLI plugin that reports session status to [Poni Deck](https://github.com/anomalyco/opencode).

It ships hooks that run on Copilot CLI lifecycle events
(`SessionStart`, `UserPromptSubmit`, `PermissionRequest`, `Notification`,
`Stop`, `ErrorOccurred`, `SessionEnd`) and write a small JSON status file to
`%APPDATA%\poni-deck\agents\{copilotPid}.json`
(`$XDG_CONFIG_HOME/poni-deck/agents/` on Linux/macOS). Poni Deck watches that
directory and displays the live status of every running agent.

The hooks are a pure side effect: they return no decision, so they never
approve/deny tool calls or block the session.

## Requirements

- Copilot CLI (v1.0.74+; hooks are loaded at session start)
- Windows: PowerShell 7+ (`pwsh`) — required by Copilot CLI on Windows
- Linux/macOS: `bash` + [`jq`](https://stedolan.github.io/jq/) (install with
  `brew install jq` / `apt install jq`; hooks no-op silently without it)

## Install from the Poni Deck plugin marketplace

This repo ships a Copilot CLI plugin marketplace at `.github/plugin/marketplace.json`.
Register the marketplace, then install the plugin:

```bash
# from inside the repo, or use your GitHub `owner/repo` once pushed
copilot plugin marketplace add ./            # or: copilot plugin marketplace add anomalyco/opencode
copilot plugin install copilot-poni-deck@poni-deck-plugins
```

or from inside an interactive session:

```
/plugin marketplace add .
/plugin install copilot-poni-deck@poni-deck-plugins
```

## Install as a Copilot CLI plugin (direct)

GitHub Copilot CLI installs plugins from a git repo, directory, or URL:

```bash
# from inside the repo (development install)
copilot plugins install ./plugins/copilot-poni-deck
```

```bash
# from a GitHub repository
copilot plugins install anomalyco/opencode:plugins/copilot-poni-deck
```

Restart Copilot CLI after installing. Verify with `copilot plugin list` (or
`/plugins list` inside a session).

The plugin declares its hooks in `hooks/hooks.json` (see
[About hooks for GitHub Copilot](https://docs.github.com/en/copilot/concepts/agents/hooks)).
If your Copilot CLI version resolves plugin hook paths relative to the plugin
root instead of the `hooks/` directory, adjust the `bash`/`powershell` values
in `hooks/hooks.json` to `./hooks/agent-status.sh` / `./hooks/agent-status.ps1`.

## Install as user-level hooks (alternative)

User-level hooks apply to every Copilot CLI session for your account. Copy the
three files into your user hooks directory:

- Windows: `%USERPROFILE%\.copilot\hooks\`
- macOS/Linux: `~/.copilot/hooks/` (or `$COPILOT_HOME/hooks/`)

```bash
mkdir -p ~/.copilot/hooks
cp plugins/copilot-poni-deck/hooks/agent-status.sh ~/.copilot/hooks/
cp plugins/copilot-poni-deck/hooks/agent-status.ps1 ~/.copilot/hooks/
cp plugins/copilot-poni-deck/hooks/hooks.json ~/.copilot/hooks/poni-deck-hooks.json
```

Start (or restart) Copilot CLI. Hook configuration is loaded at startup.

## Event → status mapping

| Event                | Status                  |
| -------------------- | ----------------------- |
| `SessionStart`       | `idle`                  |
| `UserPromptSubmit`   | `working`               |
| `PermissionRequest`  | `waiting_confirmation`  |
| `Notification` (permission/elicitation) | `waiting_confirmation` |
| `Stop` (agentStop)   | `idle`                  |
| `ErrorOccurred`      | `error`                 |
| `SessionEnd`         | status file removed     |

> **Why no `PreToolUse`/`PostToolUse`?** Copilot CLI runs command hooks
> **synchronously** and blocks agent execution, and `preToolUse` command hooks
> are **fail-closed** (an error denies the tool call). Firing on every tool
> call would add per-tool latency and risk denying tools if the hook ever
> errored. `UserPromptSubmit` → `Stop` already tracks working/idle per turn.

## Status file contract

| Field       | Type     | Description                                        |
| ----------- | -------- | -------------------------------------------------- |
| `status`    | `string` | `working`, `idle`, `waiting_confirmation`, `error` |
| `pid`       | `number` | The copilot process id                            |
| `cwd`       | `string` | Working directory of the session                  |
| `launcher`  | `string` | `vscode` or `terminal`                             |
| `tool`      | `string` | Always `copilot`                                   |
| `seq`       | `number` | Monotonic ordering guard against stale writes     |
| `timestamp` | `string` | ISO 8601 timestamp of the last write              |

## Development

The canonical hook lives at `hooks/agent-status.ps1` (Windows) and
`hooks/agent-status.sh` (Linux/macOS). It mirrors the behavior of the Claude
Code plugin in `claude-poni-deck` and the opencode plugin in
[`opencode-poni-deck`](https://www.npmjs.com/package/opencode-poni-deck).

## License

MIT
