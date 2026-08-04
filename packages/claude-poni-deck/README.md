# claude-poni-deck

Claude Code plugin that reports session status to [Poni Deck](https://github.com/anomalyco/opencode).

It ships a PowerShell hook that runs on Claude Code lifecycle events
(`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
`PermissionRequest`, `Stop`, `Notification`, `StopFailure`, `SessionEnd`) and
writes a small JSON status file to `%APPDATA%\poni-deck\agents\{claudePid}.json`.
Poni Deck watches that directory and displays the live status of every running
agent.

The hook is a pure side effect: it returns no decision, so it never blocks or
allows/denies tool calls.

## Requirements

- Windows (PowerShell 5.1+)
- Claude Code with plugin support (v2.1.x or newer)

## Install from npm

The plugin can be distributed as an npm package through a Claude Code plugin
marketplace. Host a marketplace catalog (e.g. a git repo or a file) that
references this package:

```json
{
  "name": "poni-deck-plugins",
  "owner": {
    "name": "Poni Deck Contributors"
  },
  "plugins": [
    {
      "name": "poni-deck",
      "source": {
        "source": "npm",
        "package": "claude-poni-deck",
        "version": "0.1.0"
      },
      "description": "Reports Claude Code session status to Poni Deck"
    }
  ]
}
```

Then install it in Claude Code:

```
/plugin marketplace add https://github.com/<owner>/<marketplace-repo>.git
/plugin install poni-deck@poni-deck-plugins
/reload-plugins
```

## Install from git (this repo)

This package also ships a `.claude-plugin/marketplace.json`, so it can be used
as a marketplace directly. From inside the repo root:

```
/plugin marketplace add ./packages/claude-poni-deck
/plugin install poni-deck@poni-deck-plugins
/reload-plugins
```

## Status file contract

| Field       | Type     | Description                                        |
| ----------- | -------- | -------------------------------------------------- |
| `status`    | `string` | `working`, `idle`, `waiting_confirmation`, `error` |
| `pid`       | `number` | The claude process id                             |
| `cwd`       | `string` | Working directory of the session                  |
| `launcher`  | `string` | `vscode` or `terminal`                             |
| `tool`      | `string` | Always `claude`                                    |
| `seq`       | `number` | Monotonic ordering guard against stale writes     |
| `timestamp` | `string` | ISO 8601 timestamp of the last write              |

## Development

The canonical hook lives at `hooks/agent-status.ps1`. It mirrors the behavior
of the opencode plugin in
[`opencode-poni-deck`](https://www.npmjs.com/package/opencode-poni-deck).

## License

MIT
