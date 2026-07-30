# Planning: Multi-Agent Universal Dashboard — Agent Monitor

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Revised` |
| Versi    | `2.3.0` |
| Tanggal  | `2026-07-30` |
| Author   | Planner |
| Reviewer | `Reviewer` |

---

## 1. Tujuan

Membangun desktop monitor dashboard **universal** yang mampu menampilkan **semua instance coding AI agent dari berbagai tool** (opencode, Claude Code, Cursor, GitHub Copilot, dll) dalam satu tampilan terpusat. Setiap instance, dari tool apa pun, otomatis terdaftar di dashboard saat dibuka. Dashboard menampilkan per-instance: tool identity, folder path, PID, status koneksi, dan seluruh sub-agent beserta status real-time, log, dan session tree.

**Target v1:** Support 1 tool (opencode) dengan arsitektur yang sudah siap untuk multi-tool tanpa perubahan struktural.
**Target measurable:** Dashboard mampu menampilkan ≥ 3 instance opencode simultan dari folder berbeda, masing-masing menjalankan workflow multi-agent lengkap, dengan update status real-time dan latensi event ≤ 200ms dari instance ke dashboard.

---

## 2. Scope

### In Scope (v2.0.0 — Universal)

- [ ] **Protocol universal** — shared types package dengan generic naming, semua message punya field `tool: string` untuk identitas tool
- [ ] **Adapter pattern** — setiap tool punya adapter yang connect ke monitor via protocol yang sama
- [ ] **Adapter SDK** — BaseAdapter abstract class untuk memudahkan integrasi tool baru
- [ ] **Tool icon registry** — tool → icon mapping di settings/constants dashboard
- [ ] **Instance list dengan label + icon per tool** — setiap instance menampilkan icon tool (opencode, claude-code, dll)
- [ ] **Auto-registrasi instance** — setiap instance (via adapter) otomatis mendaftar ke monitor saat startup, tanpa konfigurasi manual
- [ ] **Multi-folder, multi-instance display** — dashboard menampilkan daftar semua instance dari folder mana pun
- [ ] **Instance list view** — melihat semua instance yang terhubung: tool, folder path, PID, hostname, status koneksi
- [ ] **Per-instance agent status** — setiap instance menampilkan sub-agent internal-nya dengan status real-time
- [ ] **Per-instance confirmation modal** — ketika agent dalam suatu instance minta approve/reject, muncul modal di dashboard
- [ ] **Per-instance session tree** — tampilan hierarki parent-child session dalam setiap instance
- [ ] **Per-instance live log stream** — scrolling log output per instance, bisa switch antar instance
- [ ] **Per-instance timeline history** — daftar event kronologis per agent dalam instance
- [ ] **Per-instance resource monitor** — token usage, elapsed time, estimated cost per instance dan kumulatif
- [ ] **Quick actions per-agent per-instance** — Cancel, Retry, Pause, Skip
- [ ] **Floating pill overlay** — selalu visible di sudut layar, menampilkan jumlah instance aktif total + agent aktif
- [ ] **Expandable detail card** — klik pill → expand ke panel detail penuh
- [ ] **Notifications** — native OS notification untuk event penting
- [ ] **Auto-hide / opacity settings** — preferensi user

### Out of Scope

- [ ] Implementasi adapter untuk tool selain opencode (v2+)
- [ ] Terminal replacement atau shell emulator
- [ ] Code editor atau file manager
- [ ] Git integration atau diff viewer
- [ ] Deployment ke production (cukup development build dan local packaging)
- [ ] Cloud sync atau remote monitoring lintas mesin
- [ ] Plugin system untuk third-party agent types
- [ ] Instance auto-discovery via network (mDNS/UPnP) — hanya local file-based discovery
- [ ] Remote control instance dari dashboard (hanya quick actions yang sudah listed)

---

## 3. Pendekatan

### Strategi Terpilih

**Arsitektur Universal: Protocol + Adapter Pattern**

Dashboard monitor menjadi **tool-agnostic**. Semua logika spesifik tool dipisahkan ke adapter, sementara core dashboard dan protocol bersifat universal.

```
┌─────────────────────────────────────────────────────────────┐
│                     MONITOR DASHBOARD                        │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  @agent-monitor/app (Tauri + Svelte)                 │   │
│  │  • WS Server (multi-client)                          │   │
│  │  • Instance Registry                                 │   │
│  │  • UI: Pill, InstanceList, DetailPanel               │   │
│  │  └── all types from @agent-monitor/protocol          │   │
│  └──────────────────────────────────────────────────────┘   │
│                       ▲                                      │
│                       │ WS (universal protocol)              │
│          ┌────────────┼────────────┬────────────┐            │
│          ▼            ▼            ▼            ▼            │
│  ┌──────────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐     │
│  │ adptr-opencode│ │adptr-claude│ │adptr-cursor│ │ adptr-... │
│  │              │ │          │ │        │ │          │     │
│  │ opencode     │ │ Claude   │ │ Cursor │ │ Tool X   │     │
│  │ specific     │ │ specific │ │ spec.  │ │ specific  │     │
│  └──────────────┘ └──────────┘ └────────┘ └──────────┘     │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  @agent-monitor/adapter-sdk                          │   │
│  │  • BaseAdapter trait                                 │   │
│  │  • connect, register, emit, listen, heartbeat        │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  @agent-monitor/protocol                             │   │
│  │  • Universal types (Instance, Agent, Event, Command) │   │
│  │  • Serde + Valico schema validation                  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Registration Mechanism — File-based Discovery (Universal):**

1. **Monitor** memulai WebSocket server di `127.0.0.1:{port}` (default: `19785`).
2. **Monitor** menulis koneksi info ke file bersama:
   - Lokasi: `{monitor_config_dir}/monitor.json`
     - Windows: `%APPDATA%/agent-monitor/monitor.json`
     - macOS/Linux: `~/.config/agent-monitor/monitor.json`
   - Isi: `{ port: number, pid: number, startedAt: ISO8601 }`
3. **Setiap instance** (via adapter) saat startup:
   - Membaca `monitor.json` → dapatkan WS URL `ws://127.0.0.1:{port}`
   - Jika file ditemukan → connect ke WS → kirim `register` message dengan payload universal (termasuk field `tool`)
   - Jika file tidak ditemukan → instance tetap jalan normal, tulis file pendaftaran sementara di `{monitor_config_dir}/instances/{pid}.json`
4. **Periodic retry loop:** Jika WS connect gagal, instance retry dengan exponential backoff (15s → 30s → 45s → max 60s), maksimal 5 menit.
5. **Monitor** menyimpan registry semua instance terhubung di memori.
6. **Instance deregister** saat shutdown.
7. **Heartbeat:** instance kirim `ping` setiap 30 detik. Monitor hapus instance setelah 3 missed pings.

**Event Schema (Universal):**

```typescript
// Generic instance — tidak spesifik ke tool apapun
interface Instance {
  id: string;           // unique: format "{tool}:{pid}@{hostname}" (contoh: "opencode:1234@myhost")
  tool: string;         // "opencode" | "claude-code" | "cursor" | etc
  pid: number;
  cwd: string;
  hostname: string;
  version: string;
  toolMetadata?: Record<string, unknown>; // tool-specific extra data
  agents: Agent[];
  status: 'connected' | 'disconnected' | 'stale';
}

interface Agent {
  name: string;
  tool: string;
  role?: string;        // untuk tool yg punya multi-role (contoh: opencode requester/planner/reviewer/implementator)
  status: 'running' | 'waiting_confirmation' | 'error' | 'done' | 'idle';
  sessionId?: string;
  parentSessionId?: string | null;
}

// Event — generic
interface AgentEvent {
  type: 'agent.status' | 'agent.log' | 'agent.confirm' | 'agent.error' | 'session.created' | 'session.closed';
  tool: string;
  instanceId: string;
  agentName: string;
  timestamp: string;
  payload: Record<string, unknown>;
}

// Command — generic, tool-aware
interface UniversalCommand {
  type: 'command';
  action: 'cancel' | 'retry' | 'pause' | 'skip' | 'deregister';
  tool: string;
  instanceId: string;
  agentName?: string;
  sessionId?: string;
}
```

**Struktur Direktori (Rust workspace + Tauri + Svelte Monorepo):**

```
agent-monitor/
├── Cargo.toml                    # Workspace root
├── agent-monitor.code-workspace  # VS Code workspace (optional)
├── .gitignore
├── packages/
│   ├── protocol/                   # agent-monitor-protocol (Rust crate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── types.rs          # Generic: Instance, Agent, Event, Command
│   │       └── validator.rs      # Valico schema definitions
│   │
│   ├── adapter-sdk/                # agent-monitor-adapter-sdk (Rust crate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # BaseAdapter trait definition
│   │       └── types.rs          # Adapter-specific types
│   │
│   └── adapter-opencode/           # agent-monitor-adapter-opencode (Rust crate)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # OpencodeAdapter implements BaseAdapter
│           ├── mapper.rs         # Mapping opencode events ↔ universal schema
│           └── config.rs         # Opencode-specific config paths
│
└── apps/
    └── monitor/                    # agent-monitor-app (Rust + Svelte)
        ├── Cargo.toml
        ├── tauri.conf.json         # Tauri configuration
        ├── build.rs                # Build script
        ├── src-tauri/              # Rust backend
        │   ├── src/
        │   │   ├── main.rs       # App entry, Tauri setup
        │   │   ├── ws_server.rs  # WebSocket server (tokio-tungstenite)
        │   │   ├── registry.rs   # Universal instance registry (HashMap)
        │   │   ├── discovery.rs  # File watcher (notify)
        │   │   └── commands.rs   # Tauri command definitions (IPC bridge)
        │   ├── Cargo.toml
        │   └── build.rs
        ├── src/                    # Svelte frontend
        │   ├── app.html
        │   ├── main.ts
        │   ├── App.svelte
        │   ├── stores/            # Svelte writable stores (all generic types)
        │   │   ├── instances.ts
        │   │   ├── agents.ts
        │   │   ├── sessions.ts
        │   │   ├── logs.ts
        │   │   ├── timeline.ts
        │   │   ├── resources.ts
        │   │   ├── ui.ts
        │   │   └── settings.ts
        │   ├── components/
        │   │   ├── Pill.svelte
        │   │   ├── DetailCard.svelte
        │   │   ├── InstanceList.svelte
        │   │   ├── InstanceRow.svelte       # Show tool icon + label
        │   │   ├── ToolIcon.svelte          # NEW: icon by tool name
        │   │   ├── AgentStatusBadge.svelte
        │   │   ├── ConfirmationModal.svelte
        │   │   ├── SessionTree.svelte
        │   │   ├── LogStream.svelte
        │   │   ├── Timeline.svelte
        │   │   ├── ResourceMonitor.svelte
        │   │   ├── QuickActions.svelte
        │   │   └── SettingsPanel.svelte
        │   ├── hooks/
        │   │   ├── useWsEvents.ts
        │   │   ├── useTauriCommands.ts
        │   │   └── useAutoHide.ts
        │   └── utils/
        │       ├── format.ts
        │       ├── constants.ts        # Tool icon registry (emoji-based static mapping)
        │       └── toolRegistry.ts     # Tool metadata maps (SVG components, labels, colors — extendable)
        ├── tailwind.config.js
        ├── svelte.config.js
        ├── tsconfig.json
        └── vite.config.ts
```

**Stack Technology (Rust + Tauri + Svelte):**

| Library | Versi (Estimasi) | Lisensi | Tujuan |
|---------|------------------|---------|--------|
| tauri | ^2.0 | MIT | Desktop app host (Rust backend + web frontend) |
| svelte | ^4.0 | MIT | UI framework (lightweight, compile-time) |
| sveltekit | ^2.0 | MIT | Full-stack framework for frontend |
| typescript | ^5.3 | MIT | Type safety |
| tailwindcss | ^3.4 | MIT | Utility-first CSS |
| tokio-tungstenite | ^0.23 | MIT | WebSocket server (monitor) & client (adapter) |
| serde | ^1.0 | MIT | Serialization/deserialization |
| serde_json | ^1.0 | MIT | JSON handling |
| valico | ^3.0 | MIT | Schema validation (Rust equivalent of Zod) |
| chrono | ^0.4 | MIT | Timestamp formatting |
| tokio | ^1.0 | MIT | Async runtime |
| anyhow | ^1.0 | MIT | Error handling |
| notify | ^6.0 | MIT | File watcher untuk pending registrations |
| cargo-tauri | ^2.0 | MIT | Build orchestration & packaging |

**Alur Data:**

```
Instance A (opencode adapter, frontend/)
  │  adapter-opencode: opencode events → universal schema
  │  WS connect → ws://127.0.0.1:19785
  │  register { tool: "opencode", pid, cwd, ... }
  │  agent.status { tool: "opencode", ... }
  │  recv: command { tool: "opencode", action: "cancel", ... }
  ▼
Monitor (Tauri main process — Rust)
  │  WS Server (tokio-tungstenite) → universal protocol
  │  Registry: HashMap<instanceId, { ws, tool, pid, cwd, agents, ... }>
  │  Tauri command → forward events to Svelte frontend
  ▼
Frontend (Svelte + Svelte writable stores)
  │  All stores use universal types (no opencode-specific)
  │  toolRegistry: display tool icon + name per instance
  ▼
  ┌──────────────────────────────────────────┐
  │  InstanceList (multi-tool instances)      │
  │  ├─ 🔧 opencode  — frontend/ (PID:1234)  │
  │  │   └─ AgentList: requester, planner...  │
  │  ├─ 🤖 claude-code — backend/ (PID:5678) │
  │  │   └─ AgentList: claude agent          │
  │  └─ Instance Detail Panel (same as v1)    │
  └──────────────────────────────────────────┘
```

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **Framework desktop JS berbasis Chromium** | Ukuran bundle besar (~200MB+), memory usage tinggi (setiap instance Chromium), tidak perlu untuk overlay monitoring sederhana. Tauri jauh lebih ringan (~10-15MB) dan performa lebih baik. |
| **PWA** | Tidak memiliki akses native system tray, always-on-top window, dan overlay desktop. Hanya cocok untuk browser-based monitoring, bukan desktop overlay. |
| **Go + Wails** | Ecosystem Rust lebih matang untuk proyek ini, komunitas Tauri lebih besar, shared language (Rust) antara frontend dan backend mengurangi friction. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Instance crash tanpa sempat deregister → stale entry di registry | Tinggi | Sedang | Heartbeat ping 30 detik. Auto-hapus setelah 3 missed pings (90 detik). Tampilkan status "disconnected" dengan timestamp. |
| R2: Dua instance dari tool berbeda di folder yang sama | Rendah | Rendah | Gunakan `tool + pid` sebagai unique key. InstanceRow menampilkan tool icon + PID untuk membedakan. |
| R3: Banyak instance (>10) terkoneksi simultan → performa render | Rendah | Sedang | Virtual scrolling untuk instance list & log stream. Throttle store update per 50ms. Batasi buffer log per instance (10.000 entries). |
| R4: Monitor crash/restart → semua instance kehilangan koneksi | Sedang | Tinggi | Instance auto-reconnect dengan exponential backoff (max 30 detik). Instance terus berjalan normal saat reconnect. Setelah reconnect, kirim ulang `register` + state terkini. |
| R5: Conflict port WebSocket (port 19785 sudah dipakai) | Rendah | Sedang | Auto fallback ke port acak. Update `monitor.json` dengan port baru. Jika port acak juga gagal, fallback ke file-based polling saja (tanpa WS). |
| R6: Renderer overload karena log flood dari banyak instance sekaligus | Sedang | Tinggi | Virtual scrolling (`svelte-virtual-list` atau library Svelte setara). Throttle per-instance (batch per 50ms). Buffer per-instance maksimal 10.000 entries. Prioritaskan instance yang sedang aktif dipilih. |
| R7: File `monitor.json` corrupted atau race condition saat write | Rendah | Sedang | Atomic write (write to `.tmp` → rename). File lock via `fs.open` dengan `wx` flag. Validasi JSON saat baca. |
| R8: Tool adapter versi tidak kompatibel dengan protocol | Rendah | Sedang | Semua tipe protocol di-version via `protocolVersion` field di `register` message. Monitor tolak koneksi jika versi mismatch, kirim error message. |

### Edge Case

- [ ] **EC1: Monitor belum jalan saat instance startup** — instance tetap jalan normal, tulis file pending registration. Instance masuk ke periodic retry loop (setiap 15 detik, exponential backoff max 60 detik). Jika setelah 5 menit monitor tetap tidak ada, instance hapus pending file dan lanjut tanpa monitor.
- [ ] **EC2: Monitor mati saat ada confirmation request terbuka** — modal tetap tampil dengan status "disconnected (reconnecting...)" dan tombol disabled. Setelah reconnect, modal re-enable.
- [ ] **EC3: Instance mati di tengah proses agent** — dashboard tampilkan status "crashed" dengan tool + PID + timestamp mati.
- [ ] **EC4: File `monitor.json` dihapus secara tidak sengaja** — monitor akan recreate saat restart. Instance yang sudah terdaftar via WS tetap terdaftar (file hanya untuk discovery).
- [ ] **EC5: Instance restart cepat (restart dalam < 5 detik)** — PID baru → daftar sebagai instance baru. Instance lama dengan PID lama akan dihapus setelah heartbeat timeout.
- [ ] **EC6: Banyak instance mengirim log flood di waktu bersamaan** — throttle store update per 50ms, prioritaskan instance visible.
- [ ] **EC7: User menghapus instance dari dashboard (manual force-deregister)** — monitor kirim `deregister` ke instance via WS. Instance akan reconnect jika masih aktif.
- [ ] **EC8: Pill di-drag ke sudut layar berbeda** — simpan posisi di localStorage, restore saat restart.
- [ ] **EC9: Multiple physical displays** — gunakan `screen.getPrimaryDisplay()` atau user pilih display di settings.
- [ ] **EC10: monitor.json exist tapi monitor sudah mati** — instance retry 5 kali, jika gagal hapus `monitor.json`, buat pending file, lanjut periodic retry.
- [ ] **EC11: Config directory belum ada** — instance buat directory `{config_dir}/instances/` secara rekursif.
- [ ] **EC12: Stale pending file orphan** — scan `instances/` dan hapus file dengan PID yang sudah tidak aktif.
- [ ] **EC13: Tool tidak dikenal di registry tool icons** — dashboard tampilkan icon default generic + tool name string. User bisa kustomisasi di settings.
- [ ] **EC14: Adapter mengirim field extra yang tidak dikenali protocol** — Valico/Serde validation menerima field extra tanpa menolak koneksi, log warning, forward tetap ke renderer dengan flag `has_extra_fields`. Jangan reject seluruh koneksi.

---

## 5. Dependency

### Library

| Library | Versi (Estimasi) | Lisensi | Tujuan |
|---------|------------------|---------|--------|
| tauri | ^2.0 | MIT | Desktop app host (Rust backend + web frontend) |
| svelte | ^4.0 | MIT | UI framework (lightweight, compile-time) |
| sveltekit | ^2.0 | MIT | Full-stack framework for frontend |
| typescript | ^5.3 | MIT | Type safety |
| tailwindcss | ^3.4 | MIT | Utility-first CSS |
| tokio-tungstenite | ^0.23 | MIT | WebSocket server (monitor) & client (adapter) |
| serde | ^1.0 | MIT | Serialization/deserialization |
| serde_json | ^1.0 | MIT | JSON handling |
| valico | ^3.0 | MIT | Schema validation (Rust equivalent of Zod) |
| notify | ^6.0 | MIT | File watcher untuk pending registrations |
| chrono | ^0.4 | MIT | Timestamp formatting |
| tokio | ^1.0 | MIT | Async runtime |
| anyhow | ^1.0 | MIT | Error handling |
| cargo-tauri | ^2.0 | MIT | Build orchestration & packaging |

### Service

| Service | Tujuan |
|---------|--------|
| N/A — semua berjalan lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| `@agent-monitor/protocol` (Rust crate, workspace local) | Universal Rust types + Valico schemas untuk semua WS message — digunakan oleh monitor & semua adapter |
| `@agent-monitor/adapter-sdk` (Rust crate, workspace local) | BaseAdapter trait — semua adapter tool implements trait ini |
| `@agent-monitor/adapter-opencode` (Rust crate, workspace local) | Implementasi adapter untuk opencode — mapping opencode events → universal protocol |
| File `{monitor_config_dir}/monitor.json` | Discovery mechanism — port & PID monitor |
| Tauri IPC protocol | Contract antara Rust backend ↔ Svelte frontend |

---

## 6. Asumsi

Asumsi-asumsi berikut berlaku untuk seluruh rencana ini:

- **A1: Registrasi via file discovery** — mekanisme auto-registrasi default: monitor menulis `monitor.json` → adapter baca file → connect via WS.
- **A2: Semua instance di localhost** — monitor dan semua instance berada di mesin yang sama (127.0.0.1).
- **A3: Instance bisa jalan tanpa monitor** — semua tool tetap berfungsi normal meskipun monitor tidak ada.
- **A4: monitor.json di direktori konfigurasi bersama** — semua adapter membaca dari lokasi yang sama (`~/.config/agent-monitor/monitor.json` atau `%APPDATA%/agent-monitor/monitor.json`).
- **A5: Satu monitor per mesin** — hanya satu instance monitor yang boleh berjalan dalam satu mesin.
- **A6: Port WebSocket tidak diblokir firewall lokal** — koneksi ke 127.0.0.1 tidak dihalangi.
- **A7: User mengizinkan always-on-top** — OS support always-on-top window.
- **A8: Satu user, satu mesin** — monitor hanya untuk local single-user session.
- **A9: Tauri Notification API tersedia** — fitur notifikasi native bergantung pada OS support.
- **A10: Adapter melakukan periodic retry** — jika koneksi WS gagal, adapter terus mencoba secara periodik.
- **A11: Filesystem writable** — user memiliki akses write ke `{config_dir}/`.
- **A12: `localhost`/`127.0.0.1` reliable** — resolusi `127.0.0.1` konsisten di semua platform.
- **A13: Protocol versioning** — semua adapter dan monitor menggunakan protocol version yang sama. Mismatch dideteksi saat register dan ditolak.
- **A14: Tool identity immutable** — sekali instance register dengan `tool: "opencode"`, tool identity tidak berubah selama sesi.
- **A15: Tauri v2 always-on-top support** — Tauri v2 mendukung always-on-top window untuk floating pill overlay.
- **A16: Tauri v2 system tray support** — Tauri v2 dengan plugin mendukung system tray untuk minimize-to-tray behavior.
- **A17: Tauri security model** — `tauri.conf.json` permissions dikonfigurasi untuk allow localhost WS connection (127.0.0.1), file system access ke `{config_dir}`, dan notification API.
- **A18: Tauri draggable window** — Svelte header region dikonfigurasi sebagai drag area untuk floating pill.
- **A19: Rust toolchain available** — Tim memiliki Rust toolchain (rustup, cargo) terinstal dan terkonfigurasi.
- **A20: Tauri v2 ecosystem maturity** — Tauri v2 dianggap stable enough untuk development build dan local packaging, meskipun ekosistemnya lebih muda dari Electron.

---

## 7. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

### Package: `agent-monitor-protocol`

- [ ] **T0: Init Rust workspace + protocol crate** — setup cargo workspace, Cargo.toml untuk `agent-monitor-protocol`. Definisikan generic types: `Instance`, `Agent`, `AgentEvent`, `UniversalCommand` — semua dengan field `tool`. Buat Serde + Valico schemas untuk runtime validation. [M]
- [ ] **T0b: Rust toolchain setup + CI pipeline** — setup rustup, rustfmt, clippy config, GitHub Actions CI untuk `cargo build` + `cargo test` + linting. [S]

### Package: `agent-monitor-adapter-sdk`

- [ ] **T1: BaseAdapter trait** — definisikan trait Rust dengan method (semua parameter merujuk ke tipe dari `agent-monitor-protocol`): `connect(ws_url: &str)`, `register(instance: &Instance)`, `emit(event: &AgentEvent)`, `on_command(callback: impl Fn(UniversalCommand))`, `heartbeat(interval_ms: u64)`, `disconnect()`. Sediakan default implementation untuk heartbeat & reconnect dengan exponential backoff. [M]
- [ ] **T1b: MockAdapter untuk testing** — buat implementasi `MockAdapter: BaseAdapter` di dalam `adapter-sdk/src/testing/`. MockAdapter menerima konfigurasi untuk mensimulasikan berbagai tool (opencode, claude-code, dll) tanpa tool aktual. Digunakan untuk integration test komponen dashboard. [M]

### Package: `agent-monitor-adapter-opencode`

- [ ] **T2: Implementasi adapter untuk opencode** — buat `OpencodeAdapter: BaseAdapter`. Mapping opencode events ke universal schema:
  - opencode startup → `register` dengan `tool: "opencode"`
  - opencode agents → `Agent[]` dengan `tool: "opencode"`, `role` dari agent name
  - opencode confirm → `agent.confirm` dengan tool field
  - opencode session events → `session.created` / `session.closed`
  - opencode log → `agent.log`
  [L]
- [ ] **T3: File-based discovery di adapter** — implementasi `resolve_monitor_url()`: baca `monitor.json` dari path standar, parse port, return WS URL. Buat pending registration file jika monitor tidak ditemukan. [M]
- [ ] **T4: Periodic retry loop + heartbeat** — implementasi `retry_loop()`: exponential backoff 15s→30s→45s→60s, max 5 menit. Heartbeat ping setiap 30 detik. [M]
- [ ] **T5: Command mapping dari monitor → aksi opencode** — mapping command universal ke action opencode: `cancel`, `retry`, `pause`, `skip`, `deregister`. [M]
- [ ] **T6: Graceful shutdown** — kirim `deregister` via WS (jika terkoneksi), cleanup pending file, tutup WS connection, hentikan retry loop. [S]

### App: `agent-monitor-app` (Monitor Dashboard — Tauri + Svelte)

- [ ] **T7: Setup project scaffolding** — substeps: (a) setup Rust toolchain + cargo workspace root, (b) inisialisasi Tauri 2.0 project scaffold (Svelte standalone, bukan SvelteKit pure web; konfigurasi `svelte.config.js` dengan `adapter-tauri` dan `tauri.conf.json` dengan `build devPath: "http://localhost:1420"` dan `distDir`), (c) setup `Cargo.toml` workspace dependency ke `agent-monitor-protocol` crate, (d) konfigurasi `tauri.conf.json` permissions untuk WS localhost, file system, dan notification API. [M]
- [ ] **T8: Implementasi WebSocket server** — `tokio-tungstenite` server bind ke port, handle multiple client connections, message parsing using Valico validation dari protocol crate. [L]
- [ ] **T9: Universal instance registry** — HashMap menggunakan schema universal (Instance dari protocol crate). Handle register/deregister/heartbeat timeout (hapus stale setelah 3 missed pings). [M]
- [ ] **T10: File discovery + notify watch + stale cleanup** — tulis `monitor.json` saat start, watch pending registrations dengan `notify`, atomic file write. [M]
- [ ] **T11: Implementasi Tauri commands** — Tauri command definitions (`get_instances`, `send_command`, `subscribe_events`, `get_settings`, `update_settings`) untuk komunikasi Rust ↔ Svelte frontend dengan universal types. [M]
- [ ] **T12: Svelte stores (tool-agnostic)** — buat semua store dengan generic types dari protocol: instances, agents, sessions, logs, timeline, resources, ui, settings. Tidak ada tipe spesifik opencode. Gunakan Svelte writable stores. [M]
- [ ] **T13: Tool icon registry + ToolIcon component** — buat mapping `tool:string → { icon: string, label: string, color: string }` di `constants.ts`. Representasi icon: emoji string (misal `"🔧"` untuk opencode). Jika emoji tidak cukup, gunakan SVG component di `toolRegistry.ts`. Component ToolIcon mencoba emoji dulu, fallback ke SVG, lalu fallback generic. [S]
- [ ] **T14: Floating Pill komponen** — tampilkan jumlah instance aktif total + agents. Drag behavior, always-on-top, auto-hide. Implementasi di Svelte + CSS. [L]
- [ ] **T15: Instance List + Row komponen** — daftar semua instance dengan tool icon, tool name, folder path, PID, status koneksi, jumlah agent. Virtual scrolling menggunakan `@tanstack/vue-virtual` atau `svelte-virtual-list`. [L]
- [ ] **T16a: AgentStatus component** — tampilkan daftar sub-agent per instance dengan status badge dan role. [L]
- [ ] **T16b: ConfirmationModal component** — modal approve/reject untuk agent yang minta konfirmasi. Tampilkan label instance (tool + folder path). [L]
- [ ] **T16c: SessionTree component** — tampilan hierarki parent-child session dalam setiap instance. [L]
- [ ] **T16d: LogStream component** — scrolling log output per instance, bisa switch antar instance. Virtual scrolling. [L]
- [ ] **T16e: Timeline component** — daftar event kronologis per agent dalam instance. [L]
- [ ] **T16f: ResourceMonitor component** — token usage, elapsed time, estimated cost per instance dan kumulatif. [L]
- [ ] **T17: Quick Actions** — tombol Cancel, Retry, Pause, Skip dengan target instance + agent tertentu. Debounce 500ms. Panggil Tauri command untuk forward ke instance. [M]
- [ ] **T18: Notifications** — native OS notification via Tauri Notification API untuk event penting dari instance mana pun. [M]
- [ ] **T19: Settings Panel** — auto-hide config, opacity slider, always-on-top toggle, port config, tool icon customization. Persist di Svelte store atau Tauri store. [M]
- [ ] **T20: Testing & Bug Fixing** — unit test (`cargo test`) untuk protocol types (Valico validation), registry, discovery. Unit test frontend Svelte menggunakan **Vitest** untuk stores dan components. Integration test mechanism: MockAdapter Rust menjalankan mock WebSocket server di localhost, frontend Svelte menghubungkan ke mock WS server seperti instance nyata — validasi protocol types dan event flow. [L]
- [ ] **T21: Packaging** — Tauri build config untuk Windows `.msi/.exe` + macOS `.dmg`. Gunakan `cargo tauri build`. [M]

**Dependency antar-task:**

```
# Protocol & SDK
T0 ──→ digunakan oleh T1, T7, T9 (types digunakan di mana-mana)
T0b ──→ paralel dengan T0 (toolchain sebelum coding)

# Adapter Opencode
T1 ──→ T2 ──→ T4 ──→ T6
  │              │
  ├──→ T3 ──────┘
  │
  └──→ T1b (MockAdapter, independen — paralel dengan T2-T6)
T5 bisa paralel dengan T3/T4

# Monitor App
T7 ──→ T8 ──→ T11 ──→ T14, T15, T16a–T16f (paralel)
  │      │              │
  │      └──→ T9 ────→  │
  │            │           │
  │            └──→ T10 ─┘
  │                    │
T12 ──────────────→ T16a–T16f (parallel dengan T14, T15)
T13 independen, bisa paralel dengan T12
  │                         │
             T17, T18 (setelah T16a–T16f)
T19 independen, bisa paralel dengan T12–T16f
T20 → setelah T14–T19
T21 → setelah T20
```

## 8. Open Questions

- [ ] **Q1: Di mana path default monitor config?** — **Keputusan: `~/.config/agent-monitor/` (Linux/macOS), `%APPDATA%/agent-monitor/` (Windows).** Berbeda dari lokasi opencode karena sifat universal.
- [ ] **Q2: Protocol versioning scheme?** — **Keputusan: Semver.** Field `protocolVersion` di register message. Monitor tolak jika major version mismatch, kirim `version_mismatch` error. Minor/patch compatible.
- [ ] **Q3: Bagaimana handle tool yang tidak punya konsep sub-agent?** — **Keputusan: Agent list bisa kosong atau berisi 1 agent (`main`).** Protocol mendukung agent array 0..N. Tool seperti Claude Code cukup register dengan 1 agent bernama "claude".
- [ ] **Q4: Valico schema — strict atau passthrough?** — **Keputusan: Passthrough dengan warning log.** Field extra dari adapter tidak ditolak, hanya dicatat sebagai warning. Forward tetap ke renderer dengan flag `has_extra_fields`. (Gunakan `serde_json::Value` untuk mengakomodasi extra fields.)
- [ ] **Q5: Bagaimana tool icon untuk tool baru?** — **Keputusan: Fallback icon generic + label nama tool.** User bisa kustomisasi di settings panel.

---

## 9. Acceptance Criteria

> Semua kriteria harus measurable & bisa diverifikasi secara objektif.

- [ ] **AC1:** Dua instance dari tool yang sama (opencode, v1) dari folder berbeda muncul di dashboard dalam < 5 detik setelah startup.
- [ ] **AC2:** Dashboard menampilkan daftar instance dengan tool icon, tool name, folder path, PID, dan status koneksi.
- [ ] **AC3:** Setiap instance menampilkan sub-agent dalam hierarki yang benar sesuai workflow.
- [ ] **AC4:** Klik instance → detail panel menampilkan agent status, session tree, log, timeline, dan resource monitor.
- [ ] **AC5:** Confirmation modal dari instance manapun muncul dengan label instance (tool + folder path) dan tombol approve/reject berfungsi.
- [ ] **AC6:** Log stream per-instance menampilkan teks dengan auto-scroll. Switch antar instance mengubah log yang ditampilkan.
- [ ] **AC7:** Instance crash → dashboard deteksi dalam < 120 detik (3 missed heartbeats) → tampilkan status "crashed".
- [ ] **AC8:** Instance baru muncul di dashboard tanpa restart monitor (auto-registration via WS).
- [ ] **AC9:** Monitor restart → semua instance reconnect dalam < 30 detik → dashboard pulih tanpa data loss > 5 detik.
- [ ] **AC10:** Quick Actions (Cancel, Retry, Pause, Skip) dari dashboard dieksekusi di instance yang benar.
- [ ] **AC11:** Floating pill menampilkan jumlah instance aktif (misal: "3 instances, 8 agents") yang update real-time.
- [ ] **AC12:** Protocol types work dengan mock adapter dari tool berbeda (validasi via test).
- [ ] **AC13:** Serde + Valico schema memvalidasi message yang masuk; message invalid di-log tanpa crash dan diteruskan dengan flag `has_extra_fields`.
- [ ] **AC14:** Tool icon registry menampilkan icon yang sesuai untuk opencode, dan fallback generic untuk tool tak dikenal.
- [ ] **AC15:** Monitor bisa di-packaging menjadi executable yang bisa diinstal di Windows dan macOS.

---

## 10. Referensi

- [Tauri Documentation](https://v2.tauri.app/)
- [Tauri Stack](https://v2.tauri.app/learn/stacks/)
- [Tokio Documentation](https://tokio.rs/)
- [tokio-tungstenite Documentation](https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/)
- [Svelte Documentation](https://svelte.dev/docs)
- [SvelteKit Documentation](https://kit.svelte.dev/docs)
- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
- [Valico Documentation](https://docs.rs/valico/latest/valico/)
- [Serde Documentation](https://serde.rs/)
- [notify (file watcher)](https://docs.rs/notify/latest/notify/)
- [chrono Documentation](https://docs.rs/chrono/latest/chrono/)
- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Tauri CLI Commands](https://v2.tauri.app/start/commands/)
- [Tauri Build & Packaging](https://v2.tauri.app/distribution/)
- [Tauri Plugin System](https://v2.tauri.app/api/plugins/)

---

## 11. Arsitektur Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        MESIN (LOCALHOST)                              │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐   │
│  │                    MONITOR DASHBOARD (Tauri App)               │   │
│  │                                                                     │
│  │  ┌─────────────────────────────────────────────────────────┐  │   │
│  │  │  src-tauri/ (Rust Backend)                                    │  │   │
│  │  │  ┌─────────────────┐  ┌────────────────┐  ┌───────────────┐  │  │   │
│  │  │  │ WS Server        │  │ Universal      │  │ File Watcher   │  │  │   │
│  │  │  │ (tokio-tungsten- │  │ Registry       │  │ (notify crate) │  │  │   │
│  │  │  │  socket)         │  │ HashMap<id,    │  │                │  │  │   │
│  │  │  │  port 19785      │  │  Instance>     │  │ pending file   │  │  │   │
│  │  │  └────────┬────────┘  └───────┬────────┘  │ directory      │  │  │   │
│  │  │           │                     │               │              │  │   │
│  │  │           └─────────────────────┼───────────────┘              │  │   │
│  │  │                                   │                            │  │   │
│  │  │  ┌────────────────────────────────▼──────────────────────┐  │  │   │
│  │  │  │  commands.rs (Tauri Command Definitions)                   │  │  │   │
│  │  │  │  • get_instances() → Vec<Instance>                     │  │  │   │
│  │  │  │  • send_command(instance_id, command) → Result       │  │  │   │
│  │  │  │  • subscribe_events(tx) → EventStream                │  │  │   │
│  │  │  │  • get_settings() → Settings                          │  │  │   │
│  │  │  │  • update_settings(settings) → Result                │  │  │   │
│  │  │  └──────────────────────────────────────────────────────┘  │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                                   │                             │
│  │  ┌────────────────────────────────▼─────────────────────┐  │   │
│  │  │  src/ (Svelte Frontend)                                 │  │   │
│  │  │  ┌────────────────────────────────────────────────┐  │  │   │
│  │  │  │  App.svelte (tool-agnostic)                        │  │  │   │
│  │  │  │  ┌──────────────┐      ┌──────────────────┐   │  │  │   │
│  │  │  │  │   Pill       │      │   DetailCard        │   │  │  │   │
│  │  │  │  │  (minimized) │─────│  (expanded)          │   │  │  │   │
│  │  │  │  └──────────────┘      │                      │   │  │  │   │
│  │  │  │                           │  ┌─────────────────┐ │  │  │   │
│  │  │  │                           │  │ InstanceList     │ │  │  │   │
│  │  │  │                           │  │ ├─ 🔧 opencode  │ │  │  │   │
│  │  │  │                           │  │ │   frontend\    │ │  │  │   │
│  │  │  │                           │  │ │   PID: 1234    │ │  │  │   │
│  │  │  │                           │  │ ├─ 🤖 claude-code│ │  │  │   │
│  │  │  │                           │  │ │   backend\     │ │  │  │   │
│  │  │  │                           │  │ │   PID: 5678    │ │  │  │   │
│  │  │  │                           │  └────────┬──────────┘ │  │  │   │
│  │  │  │                           │           │ (click)     │  │  │   │
│  │  │  │                           │  ┌────────▼──────────┐  │  │  │   │
│  │  │  │                           │  │ DetailPanel         │  │  │  │   │
│  │  │  │                           │  │ (per-instance)     │  │  │  │   │
│  │  │  │                           │  │ Tool: opencode      │  │  │  │   │
│  │  │  │                           │  │ ├─AgentStatus        │  │  │  │   │
│  │  │  │                           │  │ ├─ConfirmModal       │  │  │  │   │
│  │  │  │                           │  │ ├─SessionTree        │  │  │  │   │
│  │  │  │                           │  │ ├─LogStream          │  │  │  │   │
│  │  │  │                           │  │ ├─Timeline           │  │  │  │   │
│  │  │  │                           │  │ └─ResourceMonitor     │  │  │  │   │
│  │  │  │                           │  └───────────────────┘  │  │  │   │
│  │  │  │                           └─────────────────────────┘  │  │   │
│  │  │  └───────────────────────────────────────────────────────┘  │   │
│  │  └─────────────────────────────────────────────────────────────┘   │
│  │                                                                      │
│  │  ┌──────────────────────────────────────────────────────────────┐  │
│  │  │  SHARED DISCOVERY FILE (~/.config/agent-monitor/)            │  │
│  │  │  ┌──────────────────────┐  ┌────────────────────────────┐   │  │
│  │  │  │  monitor.json        │  │  instances/                │   │  │
│  │  │  │  { port, pid }       │  │  ├── 1234.json (pending)   │   │  │
│  │  │  └──────────────────────┘  │  └── 5678.json (pending)   │   │  │
│  │  │                              └────────────────────────────┘   │  │
│  │  └──────────────────────────────────────────────────────────────┘  │
│  │                                                                      │
│  │  ┌──────────────────────────────────────────────────────────────┐  │
│  │  │  agent-monitor-protocol (Rust crate)                     │  │
│  │  │  ┌──────────────────┐  ┌─────────────────┐                  │  │
│  │  │  │ types.rs          │  │ validator.rs     │                  │  │
│  │  │  │ Instance, Agent,  │  │ Valico schemas   │                  │  │
│  │  │  │ Event, Command    │  │ Serde derives    │                  │  │
│  │  │  └──────────────────┘  └─────────────────┘                  │  │
│  │  └──────────────────────────────────────────────────────────────┘  │
│  │                                                                      │
│  │  ┌──────────────────────────────────────────────────────────────┐  │
│  │  │  agent-monitor-adapter-sdk (Rust crate)                     │  │
│  │  │  ┌──────────────────────────────┐                            │  │
│  │  │  │  BaseAdapter trait            │                            │  │
│  │  │  │  ─ connect, register, emit   │                            │  │
│  │  │  │  ─ on_command, heartbeat      │                            │  │
│  │  │  │  ─ disconnect, retry_loop     │                            │  │
│  │  │  └──────────────────────────────┘                            │  │
│  │  └──────────────────────────────────────────────────────────────┘  │
│  │                                                                      │
│  │  ┌────────────────────────┐  ┌────────────────────────┐           │
│  │  │  OCODE INSTANCE A      │  │  OCODE INSTANCE B      │           │
│  │  │  adapter-opencode       │  │  adapter-opencode       │           │
│  │  │  (D:\project\frontend) │  │  (D:\project\backend)  │           │
│  │  │  ┌──────────────────┐  │  │  ┌──────────────────┐  │           │
│  │  │  │  OpencodeAdapter  │  │  │  │  OpencodeAdapter  │  │           │
│  │  │  │  (implements      │  │  │  │  (implements      │  │           │
│  │  │  │   BaseAdapter)    │  │  │  │   BaseAdapter)    │  │           │
│  │  │  │  ─ register       │  │  │  │  ─ register       │  │           │
│  │  │  │  ─ emit events    │  │  │  │  ─ emit events    │  │           │
│  │  │  │  ─ recv commands  │  │  │  │  ─ recv commands  │  │           │
│  │  │  └──────────────────┘  │  │  │  └──────────────────┘  │           │
│  │  │  ┌──────────────────┐  │  │  │  ┌──────────────────┐  │           │
│  │  │  │  Agents:          │  │  │  │  │  Agents:          │  │           │
│  │  │  │  ├─ Requester     │  │  │  │  │  ├─ Requester     │  │           │
│  │  │  │  ├─ Planner       │  │  │  │  │  ├─ Planner       │  │           │
│  │  │  │  ├─ Reviewer      │  │  │  │  │  ├─ Reviewer      │  │           │
│  │  │  │  └─ Implementator │  │  │  │  │  └─ Implementator │  │           │
│  │  │  └──────────────────┘  │  │  │  └──────────────────┘  │           │
│  │  └────────────────────────┘  └────────────────────────┘           │
│  │  ┌────────────────────────┐                                       │
│  │  │  CLAUDE CODE INSTANCE  │  (v2+, NOT in v1 scope)               │
│  │  │  adapter-claude-code   │                                       │
│  │  │  ┌──────────────────┐  │                                       │
│  │  │  │  ClaudeAdapter    │  │                                       │
│  │  │  │  (implements      │  │                                       │
│  │  │  │   BaseAdapter)    │  │                                       │
│  │  │  └──────────────────┘  │                                       │
│  │  └────────────────────────┘                                       │
│  └─────────────────────────────────────────────────────────────────────┘
```

---

## Revisi History

| Versi   | Tanggal     | Author   | Perubahan |
|---------|-------------|----------|-----------|
| `1.0.0` | `2026-07-26` | Planner  | Initial draft |
| `1.1.0` | `2026-07-26` | Planner  | Fix kontradiksi jumlah sub-fitur (9→12); tambah §6 Asumsi; resolve Q1–Q4 di §8; spesifikasi testing framework; AC6 pakai threshold objektif |
| `1.2.0` | `2026-07-26` | Planner  | **Koreksi fundamental arsitektur:** Monitor bukan viewer 1 sesi, tapi dashboard multi-instance. Monitor jadi WS server, opencode jadi WS client. |
| `1.3.0` | `2026-07-26` | Planner  | **Perbaikan gap reviewer:** Tambah periodic retry loop, EC1 perbaikan, EC10–EC12, T1a, A10–A12 |
| `2.0.0` | `2026-07-26` | Planner  | **Re-arsitektur universal (tool-agnostic):** Ubah dari opencode-specific ke universal protocol + adapter pattern. Tambah packages: `@agent-monitor/protocol` (types + zod), `@agent-monitor/adapter-sdk` (BaseAdapter), `@agent-monitor/adapter-opencode` (implementasi). Semua tipe universal dengan field `tool`. Restruktur monorepo (packages/ + apps/). Monitor app jadi tool-agnostic, semua opencode-specific logic di adapter-opencode. Versi naik ke 2.0.0 — breaking change arsitektur. |
| `2.2.0` | `2026-07-30` | Planner  | **Migrasi tech stack:** Mengganti framework desktop lama + npm ecosystem dengan Rust + Tauri. Mengubah tumpukan dari npm/web ke Rust workspace (Cargo). Menggunakan Svelte menggantikan framework UI lama sebagai frontend, tokio-tungstenite mengganti library WS lama, Valico mengganti library schema validation lama, notify mengganti file watcher lama, Serde mengganti TypeScript JSON parse. Tauri Command mengganti IPC bridge/contextBridge lama. Semua adapter diprogram dalam Rust crate. Dokumentasi referensi diperbarui. Perubahan breaking pada implementasi namun arsitektur universal (protocol + adapter) tetap dipertahankan. |
| `2.3.0` | `2026-07-30` | Planner  | **Finalisasi migrasi Rust + Tauri (revisi post-review):** (1) Rewrite total §3 Struktur Direktori ke Rust workspace + Tauri + Svelte monorepo. (2) Rewrite §5 dependency table — hapus semua npm packages, ganti dengan crate Rust + Lisensi kolom. (3) Reverse §3 alternatives table (Tauri dipilih, Electron ditolak). (4) Rewrite §11 arsitektur diagram ke model Tauri (src-tauri/Rust backend + src/Svelte frontend + Tauri commands). (5) Tambah A15-A20 asumsi Tauri-specific (security model, always-on-top, system tray, draggable window, toolchain, ecosystem maturity). (6) Fix §8 Q4 & §9 AC13 — Zod → Valico/Serde. (7) Rewrite §10 referensi — hapus Electron/npm, tambah Rust/Tauri/Svelte/Valico/Serde/notify/chrono/cargo docs. (8) Split T16 [XL] → T16a-T16f (masing-masing [L]). (9) Detail T7 substeps (a-d) dan T20 (MockAdapter WS server integration test mechanism). (10) Tambah T0b (Rust toolchain + CI). (11) Update dependency graph untuk T16 split dan T11 removal. |
