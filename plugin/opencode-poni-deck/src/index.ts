import type { Hooks, Plugin } from "@opencode-ai/plugin"
import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"

const DEBOUNCE_MS = 250
const HEARTBEAT_MS = 10_000
const RETRY_MS = 500

type Status = "working" | "idle" | "waiting_confirmation" | "error"

function detectLauncher(): string {
  if (process.env.TERM_PROGRAM === "vscode") return "vscode"
  if (Object.keys(process.env).some((k) => k.startsWith("VSCODE_"))) return "vscode"
  return "terminal"
}

let dir: string | null = null
let pid = process.pid
let cwd = ""
let current: Status = "idle"
let lastWritten: Status | null = null
let debounceTimer: ReturnType<typeof setTimeout> | null = null
let heartbeatTimer: ReturnType<typeof setInterval> | null = null
let launcher = "terminal"
let client: any = null

function log(level: "debug" | "info" | "warn" | "error", message: string, extra: Record<string, unknown> = {}) {
  client?.app
    ?.log({
      body: {
        service: "agent-status",
        level,
        message,
        extra: { pid, ...extra },
      },
    })
    .catch(() => {})
}

function writeStatus(status: Status) {
  current = status
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(flush, DEBOUNCE_MS)
}

function flush() {
  debounceTimer = null
  if (!dir) return

  const target = join(dir, `${pid}.json`)
  const tmp = `${target}.tmp`
  const payload = JSON.stringify({
    status: current,
    pid,
    cwd,
    launcher,
    timestamp: new Date().toISOString(),
  })

  try {
    mkdirSync(dir, { recursive: true })
    writeFileSync(tmp, payload)
    rmSync(target, { force: true })
    renameSync(tmp, target)
    if (lastWritten !== current) {
      lastWritten = current
      client?.app
        ?.log({
          body: {
            service: "agent-status",
            level: "info",
            message: `status ${current}`,
            extra: { pid },
          },
        })
        .catch(() => {})
    }
  } catch (err) {
    client?.app
      ?.log({
        body: {
          service: "agent-status",
          level: "error",
          message: `failed to write ${target}: ${String(err)}`,
        },
      })
      .catch(() => {})
    setTimeout(flush, RETRY_MS)
  }
}

function cleanup() {
  if (debounceTimer) clearTimeout(debounceTimer)
  if (heartbeatTimer) clearInterval(heartbeatTimer)
  if (dir) {
    try {
      rmSync(join(dir, `${pid}.json`), { force: true })
    } catch {
      // ignore
    }
  }
}

function mapEventToStatus(type: string, props: any): Status | null {
  switch (type) {
    case "session.idle":
      return "idle"
    case "session.status": {
      const s = props?.status?.type
      if (s === "idle") return "idle"
      if (s === "busy" || s === "retry") return "working"
      return null
    }
    case "session.error":
      return "error"
    case "message.updated":
      return props?.info?.role === "assistant" ? "working" : null
    case "permission.asked":
      return "waiting_confirmation"
    case "permission.v2.asked":
      return "waiting_confirmation"
    case "permission.replied":
      return "working"
    case "permission.v2.replied":
      return "working"
    default:
      return null
  }
}

export const agentStatus: Plugin = async (ctx) => {
  dir = process.env.APPDATA
    ? join(process.env.APPDATA, "poni-deck", "agents")
    : join(process.env.USERPROFILE ?? ".", ".config", "poni-deck", "agents")
  cwd = ctx.directory ?? process.cwd()
  pid = process.pid
  launcher = detectLauncher()
  client = (ctx as any).client ?? null

  mkdirSync(dir, { recursive: true })
  flush()

  heartbeatTimer = setInterval(flush, HEARTBEAT_MS)
  process.on("exit", cleanup)

  return {
    "tool.execute.before": async (input: { tool: string }) => {
      if (input.tool === "question") {
        log("debug", "question tool asked", { tool: input.tool })
        writeStatus("waiting_confirmation")
      } else {
        writeStatus("working")
      }
    },
    "tool.execute.after": async () => writeStatus("working"),

    async event({ event }) {
      const type: string = event.type
      if (type === "permission.asked" || type === "permission.v2.asked" || type === "permission.replied" || type === "permission.v2.replied") {
        log("debug", `permission event ${type}`)
      }
      const status = mapEventToStatus(type, event.properties)
      if (status) writeStatus(status)
    },
  } satisfies Hooks
}

export default agentStatus
