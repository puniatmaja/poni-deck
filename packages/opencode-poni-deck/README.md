# opencode-poni-deck

Opencode plugin that reports agent status to [Poni Deck](https://github.com/anomalyco/opencode).

It writes a small JSON status file for the current opencode process to
`%APPDATA%\poni-deck\agents\{pid}.json` (or `~/.config/poni-deck/agents/` when
`APPDATA` is not set). Poni Deck watches that directory and displays the live
status of every running agent.

## Installation

Add the package to the `plugin` array in your `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": ["opencode-poni-deck"]
}
```

opencode installs npm plugins automatically with Bun on startup. Restart
opencode after editing the config.

## Local / development install

Load the package straight from this repo (no npm publish needed):

```json
{
  "plugin": ["./packages/opencode-poni-deck"]
}
```

## Status file contract

| Field       | Type     | Description                                        |
| ----------- | -------- | -------------------------------------------------- |
| `status`    | `string` | `working`, `idle`, `waiting_confirmation`, `error` |
| `pid`       | `number` | The opencode process id                           |
| `cwd`       | `string` | Working directory of the session                  |
| `launcher`  | `string` | `vscode` or `terminal`                             |
| `timestamp` | `string` | ISO 8601 timestamp of the last write              |

The plugin refreshes the file on a 10s heartbeat and on every status change
(debounced 250ms), so Poni Deck can remove stale entries when a process dies.

## License

MIT
