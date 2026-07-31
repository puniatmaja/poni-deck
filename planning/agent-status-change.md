# Planning: Pergantian Status Agent saat Bekerja

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Draft` |
| Versi    | `1.5.0` |
| Tanggal  | `2026-07-31` |
| Author   | Planner |
| Reviewer | `(belum di-review)` |

---

## 1. Tujuan

Monitor (Dynamic Island overlay + system tray) menampilkan **status kerja agent secara real-time** dengan pergantian status: `working`, `idle`, `waiting_confirmation`, `error`, plus fallback `running`. Tidak lagi hardcode status `"running"` untuk semua proses `opencode.exe`.

Status diambil dari **plugin opencode** (`.opencode/plugins/agent-status.ts`, project-level) yang menulis file status per-PID di `%APPDATA%/agent-monitor/agents/{pid}.json`. Plugin menulis ulang file via **heartbeat periodik 10 detik** sehingga file tetap fresh (di bawah TTL) selama proses opencode hidup. Monitor membaca file tersebut per polling dan menampilkannya di overlay (warna dot, teks compact bar, badge per agent) serta tray tooltip.

**Measurable:**
- Status agent di overlay berubah dari `idle` → `working` → `waiting_confirmation` → `idle` sesuai event opencode, tanpa restart aplikasi.
- Perubahan status muncul di overlay dalam ≤ 1 siklus polling (default 2 detik) setelah file status ditulis.
- Tray tooltip menampilkan ringkasan status aggregate, contoh: `"Agent Monitor — 2 agents · 1 working"`.
- Tanpa plugin terpasang, seluruh behavior monitor identik dengan sebelum fitur (status `"running"`, dot hijau statis); format tooltip kini disatukan dengan kasus ber-plugin (PQ10).

---

## 2. Scope

### In Scope

- [ ] **Plugin opencode** — `.opencode/plugins/agent-status.ts`: subscribe event opencode, map event → status, tulis file status per-PID ke `%APPDATA%/agent-monitor/agents/{pid}.json`.
- [ ] **Rust reader modul** — `agent-monitor/src-tauri/src/status_reader.rs`: baca & parse file status per PID (serde), TTL 30 detik via file mtime.
- [ ] **Enrich status di process scanner** — isi `AgentInfo.state` dari file status, fallback `"running"`.
- [ ] **Cleanup file status saat process stopped** — hapus `agents/{pid}.json` di `lib.rs` saat PID hilang dari scan.
- [ ] **Orphan cleanup** — saat monitor startup hapus SEMUA file di `agents/`; scan berkala 60 detik membandingkan file dengan PIDs hasil scan → hapus orphan.
- [ ] **Aggregate status untuk tray** — `lib.rs` hitung jumlah per status.
- [ ] **Tray tooltip ringkasan** — contoh `"Agent Monitor — 2 agents · 1 working"`.
- [ ] **Overlay Dynamic Island** — warna dot per status + pulse animation, teks compact bar (idle / `N agents · working`), badge status per agent di expanded panel, animasi transisi saat status berubah.

### Out of Scope

- [ ] Notifikasi OS untuk perubahan status (cukup overlay + tray).
- [ ] Auto-refresh file watcher real-time (`notify` crate) — cukup polling 2 detik + TTL.
- [ ] Multi-tool support (Claude Code, Cursor, dll) — hanya opencode.
- [ ] Status per sub-agent internal (requester/planner/reviewer/implementator) — plan ini memonitor 1 status per proses opencode.
- [ ] Quick actions (cancel, retry, pause) dari monitor.
- [ ] Perubahan pada event opencode sendiri — hanya consume event yang sudah ada.

---

## 3. Pendekatan

### Strategi Terpilih

**Sumber data: Plugin opencode → status file per-PID** (keputusan user, final).

Plugin TS/JS di `.opencode/plugins/agent-status.ts` (project-level, root repo `D:\dev\experiment-poni-agent`) dijalankan oleh opencode di dalam proses yang sama dengan session agent. Karena plugin berjalan di proses opencode, PID plugin identik dengan PID proses `opencode.exe` yang discan oleh monitor (`process.pid`). Monitor mencocokkan berdasarkan PID ini.

**Model satu proses opencode = satu instance = satu file status:** repo ini memakai multi-agent workflow (requester/planner/reviewer/implementator) yang berjalan sebagai sub-session DALAM SATU proses opencode. `scan_agents()` menemukan `opencode.exe` per proses — ekspektasinya satu per terminal/proyek yang menjalankan opencode TUI. Saat sub-session mana pun me-eksekusi tool, `tool.execute.before` menandai instance yang sama sebagai `working` — dan itu benar karena instance memang sedang bekerja. Plugin TIDAK perlu filter by sessionID; status mencerminkan aktivitas proses secara keseluruhan.

**Model hook plugin (diverifikasi dari source opencode branch `dev`):** tipe `Hooks` (`packages/plugin/src/index.ts`) TIDAK memiliki named hooks `session.idle`, `session.status`, `session.error`, `message.updated`, `permission.replied`, `permission.asked`. Named hooks yang relevan untuk fitur ini: `event`, `tool.execute.before`, `tool.execute.after`. Hanya ada DUA jalur event masuk ke plugin:
1. **Named hooks `tool.execute.before` / `tool.execute.after` (JALAN)** — core memanggil `plugin.trigger("tool.execute.before"/"tool.execute.after", input, output)` (masing-masing 5 call-site untuk `before` dan 5 untuk `after` — total 10 — di `packages/opencode/src/session/tools.ts`).
2. **Generic hook `event` (SATU-SATUNYA jalur untuk event bus)** — SEMUA event lain (`session.status`, `session.idle`, `session.error`, `message.updated`, `permission.asked`, `permission.replied`) dipublish via `events.publish(Event.X, ...)` dan sampai ke plugin HANYA lewat `hook["event"]({ event: { id, type, properties: event.data } })` (`packages/opencode/src/plugin/index.ts` → `events.listen`). Tipe SDK `Event` (types.gen.ts) memang memuat sebagian nama ini (mis. `permission.replied`, `session.status`, `session.idle`), namun keberadaan nama di tipe TIDAK berarti named hook — dari sisi plugin, semuanya tetap bus-only. **Loader juga memfilter event sebelum memanggil hook `event` (PQ20):** `if (event.location?.directory !== ctx.directory) return Effect.void` — hanya event yang `location.directory`-nya sama dengan direktori proyek plugin yang diteruskan. Konsekuensi: payload event di hook adalah `{ event: { id, type, properties } }` dengan `properties` = data asli event (untuk `session.status`: `{ sessionID, status: { type } }`).

> Catatan: named hook `permission.ask` ADA di tipe `Hooks` tapi DEAD (0 call-site di repo) — JANGAN dipakai plugin ini; permission di-handle via generic `event` hook (`permission.asked` / `permission.replied`).

**Alur data:**

```
opencode session events
  │
  ├─ event bus ──▶ generic `event` hook ──▶ switch event.type
  │    (loader filter: hanya event dgn location.directory === ctx.directory — PQ20)
  │    session.idle → idle
  │    session.status (status.type: busy|retry → working, idle → idle)
  │    session.error → error
  │    message.updated (assistant streaming) → working
  │    permission.asked → waiting_confirmation
  │    permission.replied → working
  │
  ├─ plugin.trigger("tool.execute.before" / "tool.execute.after")  (named hooks)
  │    tool.execute.before → working
  │    tool.execute.after  → working  (bukan idle; idle HANYA via event bus)
  ▼
Plugin agent-status.ts ──debounce 250ms──▶ write atomic
  │  (heartbeat tiap 10 dtk: tulis ulang file   │
  │   agar mtime tetap fresh selama proses hidup)│
  │              %APPDATA%/agent-monitor/agents/{pid}.json
  ▼                                              │
  ┌──────────────────────────────┐               │
  │ polling_loop (setiap 2 detik)│ ◀─────────────┘
  │  scan_agents()               │
  │    └─ status_reader::read(pid)  (TTL 30 dtk via mtime, fallback "running")
  │  diff → started/stopped
  │    └─ stopped → hapus agents/{pid}.json
  │  startup: hapus semua file agents/ (orphan)
  │  scan berkala 60 dtk → bandingkan file dengan PID scan → hapus orphan
  │  emit "agent-update"
  │  update tray tooltip (aggregate)
  └──────────────────────────────┘
```

**Event → status mapping (plugin):**

| Event opencode | Mekanisme hook | Status yang ditulis |
|----------------|----------------|---------------------|
| `tool.execute.before` | named hook `tool.execute.before` | `working` |
| `tool.execute.after` | named hook `tool.execute.after` | `working` |
| `message.updated` (assistant streaming) | generic `event` + switch `event.type` | `working` |
| `session.status` → `status.type === "busy"` | generic `event` + switch `event.type` | `working` |
| `session.status` → `status.type === "idle"` | generic `event` + switch `event.type` | `idle` |
| `session.status` → `status.type === "retry"` | generic `event` + switch `event.type` | `working` |
| `session.idle` | generic `event` + switch `event.type` | `idle` |
| `permission.asked` | generic `event` + switch `event.type` | `waiting_confirmation` |
| `permission.replied` | generic `event` + switch `event.type` | `working` |
| `session.error` | generic `event` + switch `event.type` | `error` |

> **SEMUA baris dengan mekanisme "generic `event`" ditangani oleh SATU hook `event` yang sama, lalu di-switch via `event.type` (PQ14/PQ15).** Tidak ada named hooks untuk `session.*`, `permission.*`, maupun `message.updated`.

> Catatan:
> - **Semua event non-tool masuk via SATU hook `event` generik + switch `event.type` (PQ14/PQ15).** Tipe `Hooks` (`packages/plugin/src/index.ts`) tidak punya named hooks `session.idle`, `session.status`, `session.error`, `message.updated`, `permission.asked`, `permission.replied`. Semua event tersebut dipublish via event bus (`events.publish(Event.X, ...)`) dan sampai ke plugin hanya lewat `hook["event"]({ event: { id, type, properties } })` (`packages/opencode/src/plugin/index.ts`). Tipe SDK `Event` (types.gen.ts) memang memuat sebagian nama (mis. `permission.replied`, `session.status`, `session.idle`, `session.error`, `message.updated`), sedangkan `permission.asked` TIDAK ada di tipe (opencode meng-emit-nya via Bus, lihat issue anomalyco/opencode#9229) — namun dari sisi plugin TIDAK ada perbedaan: SEMUAnya tetap bus-only dan hanya sampai lewat hook `event`. Plugin memfilter `event.type` di dalam hook `event`: `permission.asked` → `waiting_confirmation`, `permission.replied` → `working` (agent melanjutkan); event berikutnya tetap menimpa bila perlu. Named hook `permission.ask` ADA di tipe tapi DEAD (0 call-site di repo) — jangan dipakai.
> - **`tool.execute.after` → `working`, BUKAN `idle` (PQ17).** Transisi ke `idle` HANYA via `session.idle` dan `session.status` dengan `type === "idle"`. Alasan: setelah tool selesai, agent biasanya masih memproses hasilnya (menulis message / memanggil tool berikutnya dalam chain). Pemetaan `tool.execute.after` → `idle` akan menulis flash `idle` di tengah tool-chain; debounce 250 ms meredamnya hanya jika event berikutnya datang dalam jendela debounce, dan flash tetap bisa tampil bila `session.status` busy datang setelah jendela itu. Dengan `tool.execute.after` → `working`, rantai sebelum/after tetap `working` terus-menerus sampai benar-benar ada sinyal idle dari event bus — flash `idle` di tengah tool-chain TIDAK pernah ditulis.
> - **`session.status` tidak pernah berstatus `"running"` (PQ8).** Tipe SDK `SessionStatus` = `{ type: "idle" } | { type: "retry"; attempt; message; next } | { type: "busy" }`. Mapping memakai `status.type`: `busy` → `working`, `idle` → `idle`, `retry` → `working`. Keputusan `retry` → `working`: session masih dalam proses (retry otomatis, belum selesai/error), jadi lebih dekat ke `working` daripada `idle`; sampai `session.error`/`session.idle` datang, status tetap `working`. Tidak ada nilai `"running"` pada `session.status` di seluruh plan.
> - **`session.updated` TIDAK dipakai sebagai sumber status (PQ9).** Payload `session.updated` hanya membawa `info: Session` yang tidak memiliki field `status` (`id`, `projectID`, `directory`, `title`, `version`, `time`, `summary`, `parentID`) — frasa "evaluasi status" sebelumnya ambigu dan dihapus. Aktivitas session sudah terwakili oleh `message.updated`, `tool.execute.*`, dan `session.status`, sehingga event ini dikeluarkan dari daftar handler.

**Struktur plugin (pseudo-code — acuan implementator, PQ14):**

```ts
// .opencode/plugins/agent-status.ts — pola: 1 hook `event` generik + named hooks tool
import type { Hooks, Plugin } from "@opencode-ai/plugin"

// EKSPOR WAJIB FUNGSI async, BUKAN objek Hooks langsung (Q21):
// loader memanggil ekspor sbg fungsi (menerima `ctx`, return objek hooks).
// `export const agentStatus: Hooks = {...}` ditolak loader
// (TypeError "Plugin export is not a function", di-swallow) → plugin tidak pernah dimuat.
export const agentStatus: Plugin = async (ctx) => ({
  // (A) Named hooks — JALAN (core memanggil plugin.trigger(...) di session/tools.ts)
  "tool.execute.before": async () => writeStatus("working"),
  "tool.execute.after":  async () => writeStatus("working"), // BUKAN idle (PQ17)

  // (B) Generic `event` hook — SATU-SATUNYA jalur untuk event bus.
  //     Dipanggil sbg hook["event"]({ event: { id, type, properties } }).
  //     payload `event` hanya { id, type, properties } — TIDAK punya `location` (Q22);
  //     logging pakai `event.type` + `event.properties.sessionID`.
  async event({ event }) {
    const status = mapEventToStatus(event.type, event.properties)
    if (status) writeStatus(status)
  },
})

function mapEventToStatus(type: string, props: any): Status | null {
  switch (type) {
    case "session.idle":      return "idle"
    case "session.status":    // payload: { sessionID, status: { type: SessionStatus } }
                              //   contoh: { sessionID: "...", status: { type: "busy" } } (PQ18)
                              const s = props?.status?.type // BUKAN props.type (PQ18)
                              return s === "idle" ? "idle"
                                   : (s === "busy" || s === "retry") ? "working"
                                   : null // status tak dikenal → abaikan
    case "session.error":     return "error"
    case "message.updated":   return props?.info?.role === "assistant" ? "working" : null
    case "permission.asked":  return "waiting_confirmation"
    case "permission.replied": return "working"
    default:                  return null
  }
}
```

`writeStatus(status)` = debounce 250 ms + atomic write Windows/Bun (tmp → `fs.rmSync(target, {force:true})` → rename) + heartbeat 10 detik, dengan target `{APPDATA}/agent-monitor/agents/{process.pid}.json` (lihat detail di bawah).

**File status schema (`agents/{pid}.json`) — minimal:**

```json
{
  "status": "working",
  "pid": 1234,
  "cwd": "D:\\dev\\experiment-poni-agent",
  "timestamp": "2026-07-31T10:00:00.000Z"
}
```

- **Atomic write (Windows/Bun):** tulis ke `{pid}.json.tmp` → hapus target lama dengan `fs.rmSync(target, { force: true })` → `rename` tmp → target. Rename/overwrite di runtime Bun/Node di Windows tidak dijamin atomic (beda dengan Rust `std::fs::rename`), sehingga target lama dihapus eksplisit. Jika write kedua gagal / file tidak ter-update → retry setelah 500 ms + log ke console; file lama tetap valid sampai TTL.
- **Definisi "atomic write" dalam plan ini (PQ11):** *tidak ada pembacaan file status yang partial/corrupt.* Monitor hanya membaca path final `{pid}.json` (tidak pernah `*.tmp`), sehingga pola tmp → rmSync → rename menjamin monitor tidak pernah membaca file setengah jadi — kriteria ini terpenuhi. Pola ini **bukan** atomic tanpa-gap: ada jendela milidetik saat target absen (setelah `rmSync`, sebelum `rename`); selama jendela tersebut `read(pid)` return `None` → fallback `running` pada polling itu (bukan error), dan polling berikutnya (≤ 2 detik) membaca file yang sudah utuh.
- **Heartbeat 10 detik** — plugin menulis ulang file status setiap 10 detik (`setInterval`) dengan status & timestamp terbaru. Menjamin file tetap fresh (mtime di bawah TTL) selama proses hidup, termasuk saat agent idle >30 detik tanpa event berikutnya → file idle tidak menjadi stale.
- **Debounce 250 ms** — status yang berubah cepat (misal rantai `tool.execute.before/after`) dibatalkan penulisannya sampai 250 ms tanpa perubahan terakhir.
- `timestamp` = waktu write terakhir (ISO 8601), **hanya info/debug** — TIDAK dipakai untuk TTL.
- **TTL dihitung dari file mtime** (`std::fs::metadata(path).modified()`), bukan parse ISO 8601: umur file > 30 detik → stale → fallback `running`.

**Status set yang ditampilkan:** `working` / `idle` / `waiting_confirmation` / `error` + fallback `running` jika file status tidak ada / stale (TTL 30 detik) / tidak valid. Set ini **berbeda** dari `Agent.status` di `planning/agent-monitor.md:145` (`'running' | 'waiting_confirmation' | 'error' | 'done' | 'idle'`) (PQ13): v2 plan memakai status life-cycle agent (termasuk `done` = agent selesai), sedangkan plan ini menambahkan `working` sebagai status aktivitas real-time dan tidak memakai `done` — agent yang selesai kembali ke `idle`. Status `idle`, `waiting_confirmation`, `error`, `running` konsisten di kedua plan.

**Lokasi tampil:**
- **Overlay Dynamic Island** — warna dot + teks compact bar + badge per agent.
- **Tray tooltip** — SATU format final untuk semua kasus (dengan & tanpa plugin) (PQ10):
  `"Agent Monitor — {N} agents · {summary}"`
  - `N` = jumlah total agent.
  - `summary`:
    - 0 agent → `idle` → `"Agent Monitor — 0 agents · idle"`.
    - Semua agent berstatus sama → nama status (misal `"Agent Monitor — 2 agents · working"`; tanpa plugin semua `running` → `"Agent Monitor — N agents · running"`).
    - Campuran → `{jumlah} {status prioritas tertinggi}` (contoh `"Agent Monitor — 2 agents · 1 working"`).
- Tidak ada notifikasi OS.

**Rust side:**

- `status_reader.rs` (modul baru) — `read(pid) -> Option<String>`: baca `{config_dir}/agents/{pid}.json`, parse serde, return status valid. **TTL 30 detik via file mtime** (`std::fs::metadata(path).modified()`): umur file > 30 detik → anggap tidak ada. Field `timestamp` hanya info/debug, tidak dipakai untuk TTL. Gunakan `config_dir()` dari `config.rs` (`%APPDATA%/agent-monitor/`) — visibilitas `config_dir()` diubah menjadi `pub(crate)` agar bisa dipanggil dari modul ini. Tanpa dependency baru.
- `process_scanner.rs` — setelah membangun `AgentInfo`, set `state` dari `status_reader::read(pid).unwrap_or_else(|| "running".to_string())`.
- `lib.rs` — saat process stopped terdeteksi (PID hilang dari scan), hapus `agents/{pid}.json`. **Orphan cleanup:** saat startup hapus SEMUA file di `agents/` (aman karena plugin menulis ulang via heartbeat 10 detik saat proses masih hidup), plus scan berkala 60 detik yang membandingkan file di `agents/` dengan PIDs hasil scan → hapus orphan. Hitung aggregate (total agents + jumlah per status) untuk `update_tray_tooltip`.
- `tray.rs` — tooltip diubah menerima ringkasan status, bukan hanya count.

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **Parse output stdout/stderr opencode via proses scan** | Fragile: output bisa interleaved, tidak ada struktur, error-prone. Plugin memberi event yang sudah terstruktur. |
| **File status tunggal (bukan per-PID)** | Tidak mendukung multiple agents simultan (file per PID dibutuhkan untuk aggregate & matching per instance). |
| **WebSocket/network dari plugin ke monitor** | Menambah dependency network, port binding, dan kompleksitas; file-based cukup untuk data status sederhana. |
| **Hardcode status "running" seperti sekarang** | Tidak memenuhi tujuan fitur — user butuh status real-time yang akurat. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Plugin tidak terpasang (user menjalankan opencode tanpa plugin) | Sedang | Rendah | Fallback `running`. Tidak ada regresi behavior lama (dot hijau statis, tooltip count biasa). |
| R2: Crash tanpa cleanup → stale file status | Sedang | Rendah | TTL 30 detik via file mtime: umur file > 30 detik → fallback `running`. Heartbeat 10 detik memastikan file hanya stale jika proses sudah mati atau write gagal berkepanjangan. |
| R3: Race condition saat write file status | Sedang | Rendah | Pola tmp → rmSync → rename. Definisi "atomic" plan ini: tidak ada pembacaan file yang partial/corrupt (monitor hanya baca `{pid}.json`, tidak pernah `*.tmp`). Gap singkat saat target absen ditangani fallback `running` pada polling tersebut. (resolve PQ11) |
| R4: PID reuse oleh proses lain | Rendah | Sedang | Plugin selalu overwrite file fresh saat init (tulis status `idle` saat plugin start). File per-PID di-refresh oleh proses baru. |
| R5: Status berubah sangat cepat (tool chain) | Sedang | Rendah | Debounce write 250 ms — mencegah I/O spam dan flapping status. |
| R6: Multiple agents simultan | Sedang | Rendah | File per PID; aggregate di tray; overlay menampilkan badge per agent. |
| R7: Plugin gagal baca env `APPDATA` | Rendah | Sedang | Fallback ke lokasi config default (`USERPROFILE/.config/agent-monitor/agents/`), sama dengan logika `config_dir()` di `config.rs`. |
| R8: File status corrupt / JSON tidak valid | Rendah | Rendah | `status_reader` gagal parse → anggap tidak ada → fallback `running`. Jangan crash polling. |
| R9: Status value tidak dikenal | Rendah | Rendah | Whitelist status (`working`, `idle`, `waiting_confirmation`, `error`); selain itu → fallback `running`. |
| R10: Heartbeat gagal / file tidak ter-update (write error) | Rendah | Sedang | Retry write setelah 500 ms + log ke console. Selama belum berhasil, file lama tetap valid sampai TTL (30 detik) → fallback `running`. |
| R11: Orphan file saat monitor restart (PID tidak pernah di `previous_pids`) | Sedang | Rendah | Cleanup saat startup (hapus semua file di `agents/`, aman berkat heartbeat) + scan berkala 60 detik membandingkan file dengan PIDs hasil scan → hapus orphan. |
| R12: 2+ proses `opencode.exe` dengan working_dir sama (TUI + server process terpisah) | Sedang | Sedang | Verifikasi asumsi A1 saat implementasi; defensif: prefer proses yang punya file status `agents/{pid}.json` (yang menulis = sumber status); jika beberapa proses punya file → pakai yang mtime terbaru; sisanya fallback `running`. (resolve PQ16) |
| R13: Event tidak sampai ke hook `event` karena loader memfilter `event.location?.directory !== ctx.directory` (mis. `permission.asked`/`permission.replied`) | Sedang | Tinggi | **WAJIB diverifikasi saat implementasi (PQ20, lihat Q22):** log `event.type` + `event.properties.sessionID` di T1 untuk memastikan event sampai (payload `event` di hook hanya `{ id, type, properties }`, tanpa `location`). Fallback: `permission.replied` selalu terjadi SETELAH `permission.asked` — kalau `asked` tidak sampai, `replied` juga tidak; verifikasi pasangan ini bersama, dokumentasikan temuan, sesuaikan mapping (atau cari source event alternatif). |

### Edge Case

- [ ] **EC1: Plugin tidak terpasang** — scan menemukan `opencode.exe` tapi tidak ada file status → status `running`, dot hijau statis, tooltip `"Agent Monitor — N agents · running"` (format unifikasi PQ10). Identik dengan behavior sebelum fitur (tanpa regresi status).
- [ ] **EC2: File status ada tapi umur file (mtime) > 30 detik** — dianggap stale → fallback `running`. Dengan heartbeat 10 detik, kondisi ini hanya muncul jika proses mati atau write gagal berkepanjangan.
- [ ] **EC3: File status ada tapi JSON tidak valid** — parse gagal → fallback `running`, polling tetap lanjut.
- [ ] **EC4: Proses opencode mati tanpa cleanup** — `agents/{pid}.json` tersisa; pada siklus berikutnya `lib.rs` deteksi PID hilang → hapus file. Sebelum terhapus, TTL menahan tampilan status lama maksimal 30 detik.
- [ ] **EC5: PID reuse cepat (restart < 30 detik, PID sama)** — plugin baru overwrite file saat init → status segar untuk PID yang sama. Jika file belum ditulis, TTL fallback `running`.
- [ ] **EC6: Rantai tool execute cepat** — `tool.execute.before` → `after` dalam < 250 ms → debounce menggabungkan, status akhir tercatat sekali. Tidak ada flapping visual.
- [ ] **EC7: Plugin menulis status lebih lambat dari polling** — polling sementara membaca file lama/stale → fallback `running` atau status lama; akan ter-update di polling berikutnya (≤ 2 detik).
- [ ] **EC8: Banyak agent menulis file bersamaan** — write per-PID terpisah (tidak saling timpa), debounce per-instance (per PID), pola tmp → rmSync → rename (definisi "atomic": tidak ada partial read).
- [ ] **EC9: Working dir plugin vs yang discan berbeda** — monitor hanya mencocokkan via PID; `cwd` di file status tidak dijadikan kunci matching.
- [ ] **EC10: Dir `agents/` belum ada saat monitor baca** — `read` return `None` → fallback `running` (bukan error).
- [ ] **EC11: Dua instance monitor (single-instance mutex sudah ada di `lib.rs:75`)** — mutex mencegah double-read/hapus file status.
- [ ] **EC12: `permission.asked` lalu user batal/tolak** — `permission.replied` (via hook `event`, `event.type === "permission.replied"`) → `working`; event selanjutnya menimpa status → kembali `working`/`idle`.
- [ ] **EC13: Agent idle terus-menerus (>30 detik tanpa event berikutnya)** — heartbeat 10 detik menulis ulang file → mtime selalu fresh → status tetap `idle` (abu-abu), tidak jatuh ke fallback `running`. (resolve PQ1)
- [ ] **EC14: Write atomic gagal di Windows/Bun** — write kedua gagal / file tidak ter-update → plugin retry setelah 500 ms + log ke console; file lama tetap valid sampai TTL, lalu fallback `running`. (resolve PQ4)
- [ ] **EC15: Orphan files saat monitor restart** — agent mati saat monitor mati → file `{pid}.json` tersisa tanpa pernah dihapus (PID tidak ada di `previous_pids`). Cleanup startup + scan berkala 60 detik menghapusnya. (resolve PQ7)
- [ ] **EC16: Aktivitas sub-session (planner/reviewer/implementator) dalam satu proses opencode** — tool execution dari sub-session mana pun menandai instance yang sama sebagai `working` (benar). Tidak ada filter by sessionID; status mencerminkan aktivitas proses secara keseluruhan. (resolve PQ2)
- [ ] **EC17: Event permission & session TIDAK tersedia sebagai named hooks (bus-only)** — `permission.asked` memang tidak ada di tipe SDK `Event`, tetapi perbedaan ini TIDAK relevan karena dari sisi plugin SEMUA event bus (`permission.asked`, `permission.replied`, `session.status`, `session.idle`, `session.error`, `message.updated` — termasuk yang ada di tipe SDK) tetap HANYA sampai lewat generic `event` hook + filter `event.type`. `waiting_confirmation` & semua status lain tetap terdeteksi. (resolve PQ12/PQ15)
- [ ] **EC18: `session.status` bernilai `retry`** — dipetakan ke `working` (session masih dalam proses, retry otomatis); tetap `working` sampai `session.error`/`session.idle` datang. (resolve PQ8)
- [ ] **EC19: 2+ proses `opencode.exe` untuk proyek yang sama (PQ16)** — jika ditemukan beberapa proses ber-working_dir sama, monitor memilih proses yang punya file status `agents/{pid}.json` sebagai sumber status (yang menulis = sumber); jika beberapa proses punya file → pakai yang mtime terbaru; proses lain tanpa file → fallback `running`. Tanpa dependency baru. (resolve PQ16)
- [ ] **EC20: Event ditolak loader karena `location.directory` tidak cocok (PQ20)** — loader plugin hanya meneruskan event yang `event.location?.directory === ctx.directory` ke hook `event` (`packages/opencode/src/plugin/index.ts`). Jika `permission.asked`/`permission.replied` (atau event lain) tidak membawa `location.directory` yang cocok, event tersebut TIDAK sampai → `waiting_confirmation` tidak pernah terdeteksi. Verifikasi saat implementasi via log `event.type` + `event.properties.sessionID` di T1 (payload `event` di hook tidak punya `location` — Q22). Fallback: `permission.replied` selalu datang SETELAH `permission.asked` — kalau `asked` tidak sampai, `replied` juga tidak; pasangan ini diverifikasi bersama; dokumentasikan temuan & sesuaikan mapping. (resolve PQ20)
- [ ] **EC21: Event `permission.v2.asked` / `permission.v2.replied` ada di event bus** — TIDAK di-mapping di plugin ini karena path permission ask yang aktif saat ini = v1 (`permission.asked` / `permission.replied`); dicatat sebagai observasi (EC). Jika path v2 diaktifkan di masa depan, tambahkan mapping `permission.v2.*` di switch `event.type`. (Di luar scope saat ini.)

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| serde | 1 (sudah ada di `Cargo.toml`) | Deserialize file status JSON di `status_reader.rs` |
| serde_json | 1 (sudah ada) | Parsing & serialisasi file status |
| tokio | 1 (sudah ada, feature `full`) | Interval polling loop |
| anyhow | 1 (sudah ada) | Error handling status_reader |
| windows | 0.58 (sudah ada) | Process enumeration (tidak berubah) |

> **Tidak ada dependency baru.** Baik Rust maupun plugin (opencode menyediakan plugin API tanpa dependency eksternal).

### Service

| Service | Tujuan |
|---------|--------|
| N/A | Semuanya lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| `config_dir()` di `config.rs` (`%APPDATA%/agent-monitor/`) — visibilitas diubah `fn` → `pub(crate)` | Basis path `agents/` folder — plugin & Rust wajib konsisten memakai path ini; dipanggil oleh `status_reader.rs` |
| `process_scanner.rs::scan_agents()` | Memberikan daftar PID opencode.exe yang akan di-enrich |
| `AgentInfo.state` di `state.rs` | Field yang diisi oleh status_reader |
| `polling_loop` di `lib.rs` | Orchestrasi scan → read status → diff → emit → tray |
| Event `agent-update` (Tauri emit) | Membawa `AgentInfo` berisi state ke frontend Svelte |
| Plugin opencode (project-level `.opencode/plugins/`) | Sumber status — diaktifkan otomatis oleh opencode bila file ada di root repo |

---

## 6. Task Breakdown

### Asumsi

| # | Asumsi | Catatan |
|---|--------|---------|
| A1 | Satu proses opencode = satu instance = satu file status — **PERLU DIVERIFIKASI saat implementasi (PQ16)** | `process.pid` plugin == PID proses `opencode.exe` yang discan. **Verifikasi wajib saat implementasi:** arsitektur TUI opencode dapat memisahkan server process dari UI process (keduanya `opencode.exe`), sehingga `scan_agents()` bisa menemukan 2+ proses untuk satu proyek dan PID file status (ditulis plugin yang jalan di proses server) bisa tak cocok dengan entry TUI yang discan. **Strategi defensif (jika 2+ `opencode.exe` ber-working_dir sama ditemukan):** prefer proses yang punya file status `agents/{pid}.json` — yang menulis = yang jadi sumber status; jika beberapa proses punya file → pakai yang mtime-nya terbaru; proses tanpa file → fallback `running`. `working_dir` sudah tersedia di `AgentInfo` (`process_scanner.rs:155-160`, parse dari `--cwd`/`--dir`/`--work-dir` di command line). Aktivitas sub-session (requester/planner/reviewer/implementator) tercermin sebagai `working` pada instance yang sama; plugin TIDAK perlu filter by sessionID. Jika opencode memindahkan plugin ke proses terpisah di masa depan, matching PID perlu direvisi. |
| A2 | Plugin TS/JS project-level dimuat otomatis oleh opencode ketika file berada di `.opencode/plugins/` root repo | Berdasarkan dokumentasi opencode plugin system. |
| A3 | `%APPDATA%` tersedia di sesi Windows; jika tidak, fallback `USERPROFILE/.config` (sama dengan `config_dir()`) | Konsisten dengan `config.rs:7-16`. |
| A4 | Status yang dikirim plugin terbatas pada set yang dipetakan (tidak ada status di luar `working`/`idle`/`waiting_confirmation`/`error`) | Whitelist di `status_reader` menangani value tak dikenal → fallback `running`. |
| A5 | Filesystem tempat `%APPDATA%` berada writable oleh proses opencode | Plugin gagal tulis → tidak ada file → fallback `running`, tanpa crash. |
| A6 | TTL 30 detik cukup untuk membedakan "stale" vs "agent diam/idle" | Heartbeat 10 detik menulis ulang file secara periodik, sehingga file tetap fresh (mtime < TTL) selama proses hidup — termasuk saat agent idle tanpa event. File hanya menjadi stale jika proses mati atau write gagal berkepanjangan. |

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [ ] **T1: Plugin opencode `agent-status.ts`** — buat `.opencode/plugins/agent-status.ts` (project-level root repo) mengikuti pola "Struktur plugin (pseudo-code)" di §3 dengan **pola ekspor yang BENAR (Q21):** `export const agentStatus: Plugin = async (ctx) => ({ ... })` — fungsi async yang menerima `ctx` dan return objek hooks ber-tipe `Hooks` (contoh lengkap di §3); **BUKAN** `export const agentStatus: Hooks = { ... }` — loader opencode menolak ekspor non-fungsi (`TypeError("Plugin export is not a function")`, di-swallow) → plugin tidak pernah dimuat → seluruh fitur mati. **SATU hook `event` generik + `switch (event.type)`** untuk semua event bus, plus named hooks `tool.execute.before`/`tool.execute.after`. Detail implementasi:
  - **Init:** buat dir `{APPDATA}/agent-monitor/agents/` (fallback `USERPROFILE/.config/...` jika `APPDATA` tidak ada), tulis file status awal `idle` untuk PID sendiri (`process.pid`).
  - **Hook `event` (generic) — SATU-SATUNYA jalur event bus (PQ14/PQ15):** `event.type === "session.idle"` → `idle`; `"session.status"` → baca `event.properties.status.type` (payload `{ sessionID, status: { type } }`, contoh `{ sessionID: "...", status: { type: "busy" } }` — BUKAN `event.properties.type`, PQ18/PQ19): `busy` → `working`, `idle` → `idle`, `retry` → `working`, nilai tak dikenal → null (abaikan); `"session.error"` → `error`; `"message.updated"` (filter `event.properties.info.role === "assistant"`) → `working`; `"permission.asked"` → `waiting_confirmation`; `"permission.replied"` → `working`. TIDAK register named hooks untuk `session.*` / `permission.*` / `message.updated` — tipe `Hooks` memang tidak memilikinya; register apa pun dengan nama itu akan diabaikan oleh core.
  - **Named hooks:** `tool.execute.before` → `working`; `tool.execute.after` → `working` (BUKAN `idle` — transisi `idle` HANYA via `session.idle` / `session.status` type `idle`). JANGAN pakai named hook `permission.ask` (DEAD di core, 0 call-site).
  - TIDAK subscribe `session.updated` sebagai sumber status (payload `Session` tidak punya field status; aktivitas sudah terwakili `message.updated`/`tool.execute.*`/`session.status`).
  - File berisi `{ status, pid, cwd, timestamp }` (tanpa `agentName`). Debounce write 250 ms. **Atomic write (Windows/Bun):** tulis `{pid}.json.tmp` → `fs.rmSync(target, {force:true})` → `rename` tmp → target; jika gagal → retry 500 ms + log ke console. **Heartbeat 10 detik:** `setInterval` menulis ulang file dengan status & timestamp terbaru agar mtime selalu fresh selama proses hidup. Cleanup di shutdown/unload jika memungkinkan.
  - **Verifikasi runtime (WAJIB, PQ20/Q22):** saat implementasi, log SETIAP `event.type` + `event.properties.sessionID` ke console untuk memastikan event sampai ke hook `event`. **Catatan (Q22):** payload `event` di dalam hook plugin HANYA `{ id, type, properties }` — TIDAK ada field `location` (log `event.location?.directory` selalu `undefined`); identifikasi event via `event.type`, korelasi via `event.properties.sessionID`. **Filter `location.directory` loader:** `packages/opencode/src/plugin/index.ts` hanya meneruskan event yang `event.location?.directory === ctx.directory` — karena payload `event` di hook tidak membawa `location`, verifikasi dilakukan dengan mengecek event mana yang SAMPAI ke hook (event yang tidak sampai = ditolak filter); jika `permission.asked`/`permission.replied` tidak sampai, `waiting_confirmation` tidak akan pernah terdeteksi. **Rencana fallback:** `permission.replied` selalu terjadi SETELAH `permission.asked` — kalau `asked` tidak sampai ke hook, `replied` juga tidak akan sampai; verifikasi pasangan event ini secara bersamaan (kalau dua-duanya tidak muncul → indikasi filter/blokir). Dokumentasikan temuan di plan/issue dan sesuaikan mapping sesuai perilaku nyata loader. Pastikan juga file benar-benar ter-update saat event terjadi (log `event.type` ke console saat debug). [M]
- [ ] **T2: Rust `status_reader.rs`** — modul baru di `agent-monitor/src-tauri/src/`. Baca `{config_dir()}/agents/{pid}.json`, parse JSON via serde, return status valid. **TTL 30 detik via file mtime** (`std::fs::metadata(path).modified()`, umur > 30 detik → anggap tidak ada) — TIDAK parse ISO8601 dari `timestamp`; field `timestamp` hanya info/debug. Whitelist status (`working`, `idle`, `waiting_confirmation`, `error`). Ubah `config.rs:7` `fn config_dir()` → `pub(crate) fn config_dir()` agar bisa dipanggil dari modul ini. Tanpa dependency baru. Register `mod status_reader` di `lib.rs`. [S]
- [ ] **T3: Rust `process_scanner.rs`** — setelah membangun `AgentInfo` (baris ~155), set `state` dari `status_reader::read(pid)`; fallback `"running"` jika file tidak ada/stale/tidak valid. Hapus hardcode `state: "running"`. [S]
- [ ] **T4: Rust `lib.rs`** — saat process stopped terdeteksi (PID di `previous_pids` tapi tidak di `current_pids`), hapus file `{config_dir()}/agents/{pid}.json`. Hitung aggregate status dari `agents` map (total + jumlah `working`/`idle`/`waiting_confirmation`/`error`/`running`) dan pass ke `update_tray_tooltip`. [S]
- [ ] **T5: Rust `tray.rs`** — ubah `update_tray_tooltip` menerima ringkasan status dan merender format final tunggal (PQ10): `"Agent Monitor — {N} agents · {summary}"` — 0 agent → `"Agent Monitor — 0 agents · idle"`; semua status sama → nama status (misal `"Agent Monitor — 2 agents · working"`, `"... · running"` tanpa plugin); campuran → `"Agent Monitor — 2 agents · 1 working"`. Update default tooltip di `create_tray`. [S]
- [ ] **T6: Svelte `App.svelte`** — map status → warna: `working`=hijau + pulse animation, `idle`=abu-abu, `waiting_confirmation`=amber/kuning, `error`=merah, `running`=hijau statis (fallback). **Aggregate status (compact bar + dot utama):** prioritas `error` > `waiting_confirmation` > `working` > `running` > `idle`; text misal `"N agents · working"` / `"N agents · error"`, dan `"idle"` jika semua idle; warna dot utama mengikuti prioritas tertinggi. Setiap agent tetap punya badge status sendiri di expanded panel. Animasi transisi saat status berubah (misal opacity/background transition pada status-dot). [M]
- [ ] **T7: Rust cleanup orphan file** — di `lib.rs`: (a) saat startup monitor, hapus SEMUA file di `{config_dir()}/agents/` (aman karena plugin menulis ulang via heartbeat 10 detik saat proses masih hidup); (b) scan berkala tiap 60 detik: bandingkan file di `agents/` dengan PIDs hasil `scan_agents()` → hapus file yang PID-nya tidak ada. [S]

**Dependency antar-task:**
- T1 (plugin) independen — bisa paralel dengan T2–T5 & T7.
- T2 → T3 → T4 → T5 (reader dulu, lalu scanner memakainya, lalu lib.rs orchestrate, lalu tray render aggregate).
- T7 (cleanup) berdiri sendiri — bisa paralel dengan T3–T5; bagian startup cleanup aman karena heartbeat T1 menjamin file aktif ditulis ulang.
- T6 (frontend) setelah T3 (butuh `AgentInfo.state` yang sudah terisi dari backend).
- Testing/verifikasi: `cargo check` dan `cargo build` di `agent-monitor/src-tauri`.

---

## 7. Open Questions

- [x] **Q1: Sumber data status?** — **Keputusan final: Plugin opencode (`.opencode/plugins/agent-status.ts`) menulis file status per-PID.** Bukan parse stdout/stderr.
- [x] **Q2: Path file status?** — **Keputusan final: `%APPDATA%/agent-monitor/agents/{pid}.json`** (konsisten dengan `config_dir()` di `config.rs`). Di mesin ini: `C:\Users\ikade\AppData\Roaming\agent-monitor\agents\{pid}.json`.
- [x] **Q3: Status set yang ditampilkan?** — **Keputusan final: `working` / `idle` / `waiting_confirmation` / `error`**, fallback `running` jika file tidak ada/stale (TTL 30 detik).
- [x] **Q4: Lokasi tampil?** — **Keputusan final: Overlay Dynamic Island (dot warna + teks compact bar + badge per agent) dan tray tooltip.** Tidak ada notifikasi OS.
- [x] **Q5: Bagaimana handle file stale akibat crash tanpa cleanup?** — **Keputusan final: TTL 30 detik via file mtime** (bukan parse timestamp) + hapus file saat process stopped terdeteksi di `lib.rs` + cleanup orphan (startup & scan berkala 60 detik).
- [x] **Q6: Bagaimana plugin tahu PID-nya?** — **Keputusan final: `process.pid`**; monitor mencocokkan berdasarkan PID `opencode.exe` dari process scan.
- [x] **Q7: Bagaimana mencegah race condition write?** — **Keputusan final: Atomic write Windows/Bun** (tulis `{pid}.json.tmp` → `fs.rmSync(target, {force:true})` → rename; retry 500 ms bila gagal) + debounce 250 ms + heartbeat 10 detik.
- [x] **Q8: Nilai apa yang mungkin dimiliki `session.status`?** — **Keputusan final (PQ8):** hanya `{ type: "idle" } | { type: "retry"; attempt; message; next } | { type: "busy" }` — **tidak pernah `"running"`**. Mapping: `busy` → `working`, `idle` → `idle`, `retry` → `working`.
- [x] **Q9: Bagaimana perilaku handler `session.updated`?** — **Keputusan final (PQ9):** **dihapus dari daftar handler** — payload hanya membawa `Session` tanpa field `status`, tidak bisa dievaluasi menjadi status; aktivitas sudah terwakili `message.updated`/`tool.execute.*`/`session.status`.
- [x] **Q10: Nama event permission yang benar untuk menandai `waiting_confirmation`?** — **Keputusan final (PQ12, alasan dikoreksi PQ15):** `permission.asked` (didapat via Bus; issue anomalyco/opencode#9229) → `waiting_confirmation`; `permission.replied` → `working`. Penting: `permission.replied` ADA di tipe SDK `Event` namun TETAP bus-only — dari sisi plugin, SEMUA event bus (`session.*`, `permission.*`, `message.updated`) diterima HANYA via generic `event` hook + filter `event.type`, bukan named hooks.
- [x] **Q11: Apa definisi "atomic write" dalam plan ini?** — **Keputusan final (PQ11):** *tidak ada pembacaan file status yang partial/corrupt*; pola tmp → rmSync → rename memenuhi. Gap singkat (target absen) → fallback `running`, bukan error.
- [x] **Q12: Format tooltip final untuk semua kasus (dengan & tanpa plugin)?** — **Keputusan final (PQ10):** `"Agent Monitor — {N} agents · {summary}"`; 0 agent → `summary` = `idle`; semua sama → nama status; campuran → `{jumlah} {status prioritas}`; tanpa plugin → `"... · running"`.
- [x] **Q13: Bagaimana plugin menerima event `session.*` / `permission.*` / `message.updated`?** — **Keputusan final (PQ14/PQ15, diverifikasi dari source opencode branch `dev`):** tipe `Hooks` TIDAK punya named hooks untuk event-event itu; core hanya memanggil named hooks `tool.execute.before`/`tool.execute.after` (masing-masing 5 call-site — total 10 — di `packages/opencode/src/session/tools.ts`). SEMUA event bus sampai ke plugin lewat SATU hook `event` generik (`hook["event"]({ event: { id, type, properties } })`, `packages/opencode/src/plugin/index.ts`) yang melakukan `switch (event.type)`. Named hook `permission.ask` ada di tipe tapi DEAD (0 call-site) — tidak dipakai.

---

## 8. Acceptance Criteria

- [ ] **AC1:** Plugin terpasang + opencode jalan → overlay menampilkan agent dengan status `idle` (dot abu-abu).
- [ ] **AC2:** Saat agent sedang generate / tool execute → dot hijau pulse + teks compact bar menampilkan `working`.
- [ ] **AC3:** Saat agent menunggu permission → dot amber/kuning + status `waiting_confirmation`.
- [ ] **AC4:** Saat session error → dot merah + status `error`.
- [ ] **AC5:** Setelah response selesai (`session.idle`) → dot abu-abu + status `idle`.
- [ ] **AC6:** Tray tooltip menampilkan ringkasan status aggregate, contoh `"Agent Monitor — 2 agents · 1 working"`.
- [ ] **AC7:** Tanpa plugin → behavior seperti sebelumnya: status `"running"`, dot hijau statis, tooltip `"Agent Monitor — N agents · running"` (format unifikasi PQ10, bukan `"N agent(s) running"`). Tidak ada regresi status & perilaku.
- [ ] **AC8:** File status dengan umur (mtime) > 30 detik diabaikan → fallback `running`.
- [ ] **AC9:** Saat proses opencode berhenti → file `agents/{pid}.json` dihapus oleh monitor (jika masih ada).
- [ ] **AC10:** Multiple agents simultan → masing-masing menampilkan status sendiri (badge per agent), tray menampilkan aggregate.
- [ ] **AC11:** `cargo check` dan `cargo build` di `agent-monitor/src-tauri` berhasil tanpa error dan tanpa dependency baru.
- [ ] **AC12:** Agent diam/idle terus-menerus > 30 detik tanpa event tetap menampilkan `idle` (abu-abu) — heartbeat 10 detik menjaga file tetap fresh. (resolve PQ1)
- [ ] **AC13:** Orphan file `agents/{pid}.json` (agent mati saat monitor mati/restart) terhapus oleh cleanup startup dan/atau scan berkala 60 detik. (resolve PQ7)
- [ ] **AC14:** Campuran status (misal 1 `error` + 2 `idle`) → compact bar & dot utama mengikuti prioritas `error` > `waiting_confirmation` > `working` > `running` > `idle`; setiap agent tetap tampil badge status sendiri di expanded panel. (resolve PQ6)
- [ ] **AC15:** `session.status` dipetakan via `status.type`: `busy` → `working`, `idle` → `idle`, `retry` → `working`; tidak ada referensi `"running"` sebagai nilai `session.status` di seluruh plan (termasuk §3, T1, diagram alur data). Pola ekspor plugin konsisten di §3 & T1 = fungsi async `export const agentStatus: Plugin = async (ctx) => ({ ... })` (return objek hooks ber-tipe `Hooks`), bukan objek `Hooks` langsung (Q21). (resolve PQ8)
- [ ] **AC16:** Saat permission diminta → status `waiting_confirmation`; setelah `permission.replied` → kembali `working`. KEDUANYA via SATU hook `event` generik + switch `event.type` (`permission.asked` / `permission.replied`) — bukan named hooks (tipe `Hooks` tidak memilikinya). (resolve PQ12/PQ15)

---

## 9. Referensi

- [opencode Plugins Documentation](https://opencode.ai/docs/plugins/)
- [opencode Events (`session.idle`, `session.status`, `tool.execute.before/after`, `permission.asked`, `session.error`)](https://opencode.ai/docs/plugins/#events)
- [Tauri v2 Emitter (`app.emit`)](https://v2.tauri.app/)
- [Serde Documentation](https://serde.rs/)
- [Plan referensi: `planning/agent-monitor.md`](agent-monitor.md)
- [Plan referensi: `planning/agent-monitor-phase1.md`](agent-monitor-phase1.md)
- [config.rs `config_dir()`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/config.rs)
- [state.rs `AgentInfo.state`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/state.rs)
- [process_scanner.rs `scan_agents()`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/process_scanner.rs)
- [SDK types `types.gen.ts` (SessionStatus, Session, Event)](https://github.com/anomalyco/opencode/blob/dev/packages/sdk/js/src/gen/types.gen.ts)
- [Hooks type `packages/plugin/src/index.ts` — daftar named hooks + generic `event` hook](https://github.com/anomalyco/opencode/blob/dev/packages/plugin/src/index.ts)
- [Plugin loader & generic `event` hook `packages/opencode/src/plugin/index.ts` (events.listen → `hook["event"]`)](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/plugin/index.ts)
- [Call-site `plugin.trigger("tool.execute.before"/"tool.execute.after")` `packages/opencode/src/session/tools.ts`](https://github.com/anomalyco/opencode/blob/dev/packages/opencode/src/session/tools.ts)
- [GitHub issue anomalyco/opencode#9229 — `permission.ask`/`permission.asked` hanya dikirim via Bus](https://github.com/anomalyco/opencode/issues/9229)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `2026-07-31` | Planner | Initial draft |
| `1.1.0` | `2026-07-31` | Planner | Resolve review PQ1–PQ7: tambah heartbeat 10 detik di plugin (PQ1); klarifikasi model 1 proses = 1 instance, sub-session reflected sbg working, hapus klaim "1 session aktif per PID" (PQ2); TTL via file mtime, bukan parse ISO8601, `config_dir()` → `pub(crate)` (PQ3); atomic write Windows/Bun tmp → rmSync → rename + retry 500 ms (PQ4); hapus field `agentName` dari skema (PQ5); prioritas aggregate error > waiting_confirmation > working > running > idle (PQ6); orphan cleanup startup + scan 60 detik (PQ7). Update §2–§8, Asumsi A1/A6/A7, Revisi History. |
| `1.2.0` | `2026-07-31` | Planner | Resolve review putaran 2 PQ8–PQ13 + minor: mapping `session.status` via `status.type` (`busy`→`working`, `idle`→`idle`, `retry`→`working`), hapus semua referensi nilai `"running"` pada `session.status` (PQ8); hapus handler `session.updated` sebagai sumber status — `Session` tanpa field status (PQ9); unifikasi format tooltip `"Agent Monitor — {N} agents · {summary}"` untuk semua kasus, termasuk 0 agent & tanpa-plugin (PQ10); definisi eksplisit "atomic write" = tanpa partial/corrupt read + konsistensi R3 (PQ11); `permission.asked` via generic `event` hook + `permission.replied` → `working` (PQ12); koreksi klaim selaras `agent-monitor.md`, hapus A7 duplikat (PQ13); pindahkan Asumsi ke `### Asumsi` di §6, tambah EC17/EC18, AC15/AC16, Q8–Q12, referensi types.gen.ts & issue #9229. |
| `1.3.0` | `2026-07-31` | Planner | Resolve review putaran 3 PQ14–PQ17 (fondasi plugin API diverifikasi dari source opencode branch `dev`): koreksi TOTAL desain hook plugin — hanya named hooks `tool.execute.before`/`tool.execute.after` (5 call-site di `session/tools.ts`) yang JALAN; SEMUA event lain (`session.idle`, `session.status`, `session.error`, `message.updated`, `permission.asked`, `permission.replied`) diterima lewat SATU hook `event` generik + switch `event.type`; tipe `Hooks` TIDAK punya named hooks untuk `session.*`/`permission.*`/`message.updated`; `permission.ask` DEAD (PQ14); koreksi alasan PQ12 — `permission.replied` ada di types.gen.ts tapi tetap bus-only, tidak boleh ada yang memperlakukan event tersebut sebagai named hooks (PQ15); Asumsi A1 ditandai PERLU DIVERIFIKASI + strategi defensif 2+ proses working_dir sama (prefer proses ber-file status, mtime terbaru) + R12 & EC19 (PQ16); `tool.execute.after` → `working` (bukan `idle`), transisi `idle` HANYA via `session.idle`/`session.status` type idle + penjelasan flash idle ditekan (PQ17). Update §3 (paragraf model hook plugin, diagram alur data, tabel mapping + kolom mekanisme hook, pseudo-structure plugin), T1, AC16, Asumsi A1, §4 (R12, EC12, EC17, EC19), Q10 & Q13, §9 referensi source, Revisi History. |
| `1.4.0` | `2026-07-31` | Planner | Resolve review putaran 4 PQ18–PQ20 (access path & filter loader diverifikasi dari source opencode branch `dev`): koreksi access path `session.status` → `event.properties.status.type` (BUKAN `event.properties.type`) di pseudo-code §3 (PQ18) & T1 §6 (PQ19), sertakan contoh payload aktual `{ sessionID, status: { type } }`; pastikan TIGA tempat identik (tabel mapping §3, pseudo-code §3, T1 §6) memakai `status.type`; diagram alur §3 tidak memakai frasa yang terbaca sebagai field top-level `type`; dokumentasikan filter loader `if (event.location?.directory !== ctx.directory) return Effect.void` di §3 model hook plugin + diagram alur (PQ20); tambah item verifikasi runtime di T1 (log setiap `event.type` + `event.location?.directory`, pasangan `permission.replied`-setelah-`permission.asked` sebagai fallback deteksi) + risiko R13 & edge case EC20 di §4. Update metadata, Revisi History. |
| `1.5.0` | `2026-07-31` | Planner | Resolve review putaran 5 (batas maksimal) PQ21–PQ22 (+Q23): koreksi pola ekspor plugin §3 — `export const agentStatus: Plugin = async (ctx) => ({ ... })` (fungsi async terima `ctx`, return objek hooks ber-tipe `Hooks`), bukan `export const agentStatus: Hooks = {...}` yang ditolak loader (`TypeError "Plugin export is not a function"`, di-swallow → plugin tidak dimuat) (Q21); sinkronkan T1 §6 & AC15 agar pola ekspor = fungsi + contoh ekspor benar di pseudo-code & T1 (Q21); item verifikasi runtime T1 ganti `event.location?.directory` (selalu `undefined` — payload `event` di hook hanya `{ id, type, properties }`) → `event.type` + `event.properties.sessionID`, sinkronkan R13 & EC20 (Q22); frasa "Named hooks yang ada" → "hooks yang relevan untuk fitur ini" (Q23); klarifikasi call-site → `tool.execute.before` & `after` masing-masing 5 (total 10) di `session/tools.ts`, sinkronkan Q13; tambah EC21 `permission.v2.asked`/`permission.v2.replied` ada di bus, tidak di-mapping (path v1 aktif). Update metadata, Revisi History. |
