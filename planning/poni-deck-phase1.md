# Planning: Poni Deck Phase 1 — Simple Process Scanner

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Draft` |
| Versi    | `1.3.0` |
| Tanggal  | `2026-07-30` |
| Author   | Planner |
| Reviewer | `(belum di-review)` |

---

## 1. Tujuan

Membangun **aplikasi desktop kecil** yang memonitor proses `opencode` di Windows dengan cara polling daftar proses (process listing). Saat proses opencode terdeteksi started/stopped, tampilkan notifikasi OS. Klik notifikasi → langsung buka folder agent di terminal atau VS Code. Tidak ada WebSocket, adapter, protocol, registry, heartbeat, atau dashboard kompleks.

**Measurable:**
- Aplikasi mendeteksi proses `opencode.exe` muncul/hilang dalam ≤ 5 detik.
- Notifikasi OS muncul dalam ≤ 2 detik setelah deteksi.
- Klik notifikasi membuka terminal/cmd atau VS Code di folder working directory agent.

---

## 2. Scope

### In Scope

- [ ] **Process scanning** — polling daftar proses Windows, cocokkan executable name (`opencode.exe`).
- [ ] **Deteksi started** — trigger saat proses opencode baru muncul.
- [ ] **Deteksi stopped** — trigger saat proses opencode hilang, catat exit code.
- [ ] **Notifikasi OS** — native Windows toast notification untuk started & stopped (dengan exit code).
- [ ] **Click action** — klik notifikasi → buka terminal (cmd/pwsh) atau VS Code di folder agent.
- [ ] **System tray icon** — icon di system tray, indikasi jumlah agent hidup.
- [ ] **Status sederhana** — hanya tahu: hidup/mati, PID, exit code, folder path.
- [ ] **Auto-start** — opsi register aplikasi agar auto-start saat Windows login.

### Out of Scope

- [ ] WebSocket server/client — tidak ada komunikasi network.
- [ ] Adapter pattern — tidak perlu generic adapter untuk multi-tool.
- [ ] Protocol definitions — tidak ada shared types atau message schema.
- [ ] Instance registry — cukup polling process table.
- [ ] Heartbeat mechanism — tidak perlu, polling sudah cukup.
- [ ] Dashboard UI — tidak ada window dashboard, hanya tray icon + notifikasi.
- [ ] Session tree / log stream / timeline / resource monitor — semua dari plan sebelumnya.
- [ ] Confirmation modal — tidak perlu.
- [ ] Quick actions (cancel, retry, pause, skip) — tidak perlu.
- [ ] Multi-tool support (Claude Code, Cursor, dll) — hanya opencode.
- [ ] macOS / Linux support — Windows utama (bisa ditambah nanti).
- [ ] Web frontend / Svelte / Tailwind — tidak perlu.

---

## 3. Pendekatan

### Strategi Terpilih

**Stack: Rust + Tauri 2.0 (minimal, tanpa web frontend)**

Alasan:
- Rust memberikan akses langsung ke Win32 API untuk process scanning dan notifikasi.
- Tauri 2.0 bisa dipakai tanpa web frontend (headless + system tray plugin).
- Ukuran binary kecil (≤ 10 MB) — cocok untuk background tool.
- Tauri plugin `notification` untuk native toast, `shell` untuk buka terminal/VS Code, `tray-icon` untuk system tray.

**Arsitektur:**

```
┌─────────────────────────────────────────────┐
│         poni-deck-phase1 (Tauri App)      │
│                                               │
│  ┌─────────────────────────────────────────┐  │
│  │  ui_overlay/ (Frontend — WebView)        │  │
│  │  • Dynamic Island overlay                │  │
│  │  • Compact (idle) / expanded (list)      │  │
│  │  • Bounce + expand animations (CSS)      │  │
│  │  • Menampilkan instance-agent terdeteksi  │  │
│  └─────────────────────────────────────────┘  │
│                                               │
│  ┌─────────────────────────────────────────┐  │
│  │  src-tauri/ (Rust Backend — Main Loop)   │  │
│  │                                           │  │
│  │  ┌─────────────────────┐                  │  │
│  │  │  process_scanner.rs  │  ── polling ──┐  │  │
│  │  │  • enum_process()    │               │  │  │
│  │  │  • match name        │               │  │  │
│  │  │  • track PID + state │               │  │  │
│  │  └──────────┬──────────┘                │  │  │
│  │             │ detect change              │  │  │
│  │  ┌──────────▼──────────┐                │  │  │
│  │  │  notifier.rs         │               │  │  │
│  │  │  • on_started()       │               │  │  │
│  │  │  • on_stopped(code)  │               │  │  │
│  │  │  • toast notification │               │  │  │
│  │  └──────────┬──────────┘                │  │  │
│  │             │                            │  │  │
│  │  ┌──────────▼──────────┐                │  │  │
│  │  │  click_handler.rs    │               │  │  │
│  │  │  • resolve path      │               │  │  │
│  │  │  • open terminal/code│               │  │  │
│  │  └─────────────────────┘                │  │  │
│  │                                           │  │
│  │  ┌─────────────────────┐                  │  │
│  │  │  tray.rs             │                  │  │
│  │  │  • system tray icon  │                  │  │
│  │  │  • context menu      │                  │  │
│  │  │  • status indicator  │                  │  │
│  │  └─────────────────────┘                  │  │
│  └─────────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

**Alur Proses:**

1. **Startup:** Aplikasi jalan di background (system tray). Mulai interval timer (misal 2 detik).
2. **Polling:** Tiap interval, panggil `CreateToolhelp32Snapshot` (Win32) untuk list proses. Cari proses dengan nama `opencode.exe`.
3. **State tracking:** Bandingkan dengan snapshot sebelumnya.
   - Proses baru → trigger `on_started(PID)`. Dapatkan working directory via metode Q1 (lihat sub-bagian "Bagaimana dapat folder path agent").
   - Proses hilang → trigger `on_stopped(PID, exit_code)`.
4. **Notifikasi:**
   - **Started:** "opencode agent started" + path folder.
   - **Stopped:** "opencode agent finished" + exit code + path folder.
   - Gunakan `tauri-plugin-notification` untuk native toast notification.
5. **Click action:**
   - Semua klik notifikasi (started/stopped) → buka terminal (cmd/pwsh) atau `code .` di folder agent. Behavior seragam, dapat dikonfigurasi via settings (`click_action`: `"terminal"` / `"code"`).
   - Gunakan `Shell::open()` atau `std::process::Command`.
6. **System tray:**
   - Icon default (agent hidup/ada vs tidak ada).
   - Menu: Status (agent count), Open Folder, Settings (auto-start), Quit.

**Bagaimana dapat folder path (working directory) agent?**
- **Metode terpilih: Option 1 — Query via Windows API + parse argumen command**
  1. Panggil `CreateToolhelp32Snapshot` + `Process32First/Next` untuk enumerasi proses — dapatkan PID.
  2. Untuk setiap PID yang cocok: `OpenProcess` → `QueryFullProcessImageNameW` untuk path executable (reliabilitas: high).
  3. Baca command line via `NtQueryInformationProcess` (baca PEB → `RTL_USER_PROCESS_PARAMETERS` → `CommandLine`).
  4. Parse command line untuk cari flag `--cwd <path>` atau `--dir <path>` yang mungkin disertakan agent (reliabilitas: medium — tergantung agent menyertakan flag path).
  5. **Fallback:** Jika tidak ada flag path, gunakan direktori dari executable path (`std::path::Path::parent()`) sebagai estimasi working directory.
- **Mengapa bukan opsi lain?**
  - Opsi 2 (environment variable `AGENT_MONITOR_CWD`) butuh user action untuk menyetelnya — tidak ideal untuk Phase 1 yang ingin zero-config.
  - Opsi 3 (opencode tulis file `~/.opencode/monitor-status.json`) butuh modifikasi opencode — di luar scope Phase 1.

### UI Spesifik

**Bentuk:** Kotak rounded (rounded square), bukan pill/bulat.

**Posisi:** Atas tengah (top center), bukan pojok/pinggir.

**Style:** Dynamic Island-like:
- **Idle:** Compact box kecil di atas tengah layar (menampilkan status singkat atau icon jumlah agent aktif).
- **Hover:** Muncul bouncing animation, lalu expand smooth menjadi list panel yang menampilkan instance-agent yang terdeteksi.
- **Animasi:** Bouncing + expand menggunakan CSS transitions.

**Implementasi:**
- Tauri window overlay transparan (`always_on_top`, `decorations: false`, `transparent: true`) diposisikan di atas tengah layar.
- HTML/CSS/JS minimal untuk rendering dua state (compact ↔ expanded).
- State toggle via hover: masuk → bounce → expand; keluar → collapse → compact.

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **Python + PyQt/PySide** | Bundle size besar (~200MB) via PyInstaller, rawan false positive antivirus. Butuh Python runtime terinstall. |
| **Node.js + Electron** | Sama seperti Python — terlalu berat untuk aplikasi yang hanya polling proses + notifikasi. |
| **Python + pystray + plyer** | Bisa jadi opsi lebih ringan, tapi butuh Python runtime terinstall. Packaging ke .exe via PyInstaller rawan false positive antivirus. |
| **Node.js + node-notifier + node-windows** | Tidak se-ringan Rust. Butuh Node.js runtime. Tray icon tidak native di Node.js tanpa Electron. |
| **Go + Wails** | Kurang mature dibanding Tauri + Rust untuk system tray dan notifikasi Windows native. |
| **Rust saja (tanpa Tauri)** | Bisa pakai `winapi` + `windows-rs` langsung, tapi Tauri memberikan packaging (.msi), auto-update, dan plugin ecosystem yang siap pakai. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Polling terlalu sering → CPU usage tinggi | Sedang | Sedang | Interval 2 detik. Gunakan `CreateToolhelp32Snapshot` (ringan). Batasi CPU threshold. |
| R2: Notifikasi tidak muncul (Windows notification settings) | Rendah | Sedang | Fallback ke tray icon balloon tip. Cek `ToastNotifier.setting`. |
| R3: Proses opencode crash cepat (start-stop dalam < 1 detik) | Rendah | Rendah | Debounce 1 detik sebelum trigger notifikasi. Track last event time. |
| R4: Tidak dapat folder path dari proses | Sedang | Rendah | Fallback: tampilkan path executable saja. User bisa klik "Open Folder" manual dari tray menu. |
| R5: Dua instance opencode berjalan simultan | Rendah | Sedang | Track semua instance (multi-entry). Notifikasi per-instance. Tampilkan count di tray icon. |
| R6: Notifikasi diklik tapi folder sudah tidak ada | Rendah | Rendah | Validasi path exists sebelum buka. Jika tidak, tampilkan error notification. |
| R7: Antivirus flag polling process | Rendah | Rendah | Gunakan Win32 API (CreateToolhelp32Snapshot) yang legitimate. Sign binary. |

### Edge Case

- [ ] **EC1: Proses opencode berjalan dari network drive** — folder path mungkin tidak bisa diakses langsung. Fallback ke executable path.
- [ ] **EC2: Exit code tidak terbaca (proses force-kill)** — catat exit code sebagai `-1` (unknown).
- [ ] **EC3: Notifikasi di-click saat aplikasi sedang sibuk** — queue action, proses setelah handler selesai.
- [ ] **EC4: System tray icon ganda karena restart** — pastikan hanya 1 instance aplikasi berjalan (mutex).
- [ ] **EC5: User mematikan aplikasi via Task Manager** — tidak sempat cleanup. Saat restart, polling akan snap fresh state.
- [ ] **EC6: Banyak notifikasi bertumpuk** — gunakan Windows notification grouping. Jangan spam.
- [ ] **EC7: VS Code tidak terinstall** — fallback: buka terminal/cmd saja.
- [ ] **EC8: Interval polling tidak konsisten (system sleep)** — reset timer setelah wake, refresh full state.

---

## 5. Dependency

### Library

| Library | Versi (Estimasi) | Lisensi | Tujuan |
|---------|------------------|---------|--------|
| tauri | ^2.0 (feature: `"tray-icon"`) | MIT | Desktop app framework (headless + system tray icon & context menu) |
| tauri-plugin-notification | ^2.0 | MIT | Native OS notification |
| tauri-plugin-shell | ^2.0 | MIT | Buka terminal/VS Code |
| tauri-plugin-process | ^2.0 | MIT | Manage process lifecycle |
| windows-sys / windows-rs | ^0.58 | MIT | Win32 API untuk process enumeration & query |
| serde | ^1.0 | MIT | JSON config serialization |
| serde_json | ^1.0 | MIT | Config file parsing |
| chrono | ^0.4 | MIT | Timestamp untuk logging |
| tokio | ^1.0 | MIT | Async timer & polling interval |
| anyhow | ^1.0 | MIT | Error handling |

### Service

| Service | Tujuan |
|---------|--------|
| N/A | Semuanya lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| File `config.json` di `%APPDATA%/poni-deck-phase1/` | Settings: polling interval, click action, auto-start flag |
| Windows Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | Auto-start registration |

---

## 6. Task Breakdown

### Asumsi

Asumsi-asumsi yang digunakan dalam plan ini:

| # | Asumsi | Catatan |
|---|--------|---------|
| A1 | Nama proses agent yang dimonitor adalah `opencode.exe` | Jika user me-rename binary, monitor tidak akan mendeteksi. Dapat dikonfigurasi via settings di fase mendatang. |
| A2 | WebView2 harus terinstal di sistem | Prasyarat Tauri di Windows. Windows 11 sudah include, Windows 10 perlu install manual. |
| A3 | Monitor hanya berjalan untuk single user session | Tidak handle multi-session / RDP. Setiap user butuh instance sendiri. |
| A4 | User yang menjalankan monitor punya akses read ke folder agent dan hak akses enumerasi proses | Tanpa akses, `CreateToolhelp32Snapshot` dan `OpenProcess` akan gagal. |
| A5 | `opencode` menyertakan flag `--cwd <path>` atau `--dir <path>` di command line saat dijalankan | Jika tidak ada flag path, fallback ke direktori tempat executable berada. |

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

### Phase 1 — Core Implementation

- [ ] **T1: Setup proyek Tauri** — `npm create tauri-app` dengan Rust template minimal (tanpa frontend). Setup Cargo.toml dengan dependencies: tauri, tauri-plugin-notification, tauri-plugin-shell, tauri-plugin-process, windows-sys, tokio, serde, anyhow. [S]
- [ ] **T2: Implementasi process scanner** — `process_scanner.rs`: gunakan Win32 API `CreateToolhelp32Snapshot` + `Process32First/Next` untuk enumerate proses. Filter nama executable `opencode.exe`. Simpan state HashMap<PID, ProcessInfo>. [M]
- [ ] **T3: Dapatkan working directory dari proses** — `process_scanner.rs`: gunakan `CreateToolhelp32Snapshot` + `OpenProcess` + `QueryFullProcessImageNameW` untuk executable path. Baca command line via `NtQueryInformationProcess` (PEB → `RTL_USER_PROCESS_PARAMETERS`), parse argumen `--cwd`/`--dir`. Fallback: direktori executable. [M]
- [ ] **T4: State diff engine** — bandingkan snapshot lama vs baru. Deteksi `started` (PID baru), `stopped` (PID hilang), simpan exit code saat proses hilang via `GetExitCodeProcess`. [M]
- [ ] **T5: Notifikasi OS** — `notifier.rs`: gunakan tauri-plugin-notification untuk native toast. Format pesan: "opencode agent started — D:\project" / "opencode agent finished — exit code 0". [M]
- [ ] **T6: UI Dynamic Island overlay** — `ui_overlay/`: buat Tauri window overlay transparan di posisi atas tengah. Dua state: compact (idle, kotak kecil) dan expanded (list panel agent). Implementasi HTML/CSS/JS: animasi bouncing + expand smooth via CSS transitions. [M]
- [ ] **T7: Click handler** — `click_handler.rs`: saat notifikasi diklik, parse action intent. Buka terminal (cmd/pwsh) di folder agent, atau `code .` jika VS Code terdeteksi. [S]
- [ ] **T8: System tray** — `tray.rs`: implementasi Tauri system tray icon. Context menu: "Status: X agent running", "Open Folder...", "Auto-start: ON/OFF", "Quit". Ikon berubah sesuai status (ada/tidak ada agent). [M]
- [ ] **T9: Single instance lock** — gunakan named mutex (Win32 `CreateMutexW`) untuk mencegah multiple instance aplikasi. [S]
- [ ] **T10: Config & auto-start** — `config.rs`: baca/tulis `config.json` di `%APPDATA%/poni-deck-phase1/`. Field: `polling_interval_ms`, `click_action` (terminal/code), `auto_start`. Toggle auto-start via registry `Run` key. [M]
- [ ] **T11: Packaging** — konfigurasi Tauri build untuk Windows `.msi` / `.exe`. Sign binary jika ada sertifikat. [M]

**Dependency antar-task:**
- T1 → T2 → T4 → T5 → T7
- T3 paralel dengan T2 (bagian dari process_scanner.rs)
- T6 independen setelah T4 (butuh data agent dari state)
- T8 independen setelah T4 (perlu state untuk status)
- T9, T10 independen, bisa paralel
- T11 → setelah semua selesai

---

## 7. Open Questions

- [x] **Q1: Metode dapatkan working directory proses?** — **Keputusan final: Option 1 (Query Windows API + parse argumen command).** Detail lengkap di §3 — "Bagaimana dapat folder path (working directory) agent?".
- [ ] **Q2: Apakah perlu menampilkan tray menu "Open Folder" tanpa notifikasi?** — **Keputusan: Ya.** Tray menu berisi daftar folder agent yang aktif (jika ada), user bisa klik langsung.
- [x] **Q3: Action default saat klik notifikasi?** — **Keputusan final: Buka terminal (cmd/pwsh).** Behavior seragam untuk semua notifikasi. Dapat dikonfigurasi via settings (`click_action`).
- [x] **Q4: Apakah perlu menampilkan counter di tray icon (badge)?** — **Keputusan final: Tidak.** Windows tray icon tidak support badge native. Cukup update tooltip text: "X agent(s) running".

---

## 8. Acceptance Criteria

- [ ] **AC1:** Aplikasi jalan di background (system tray) tanpa window.
- [ ] **AC2:** Dalam ≤ 5 detik setelah opencode.exe start, muncul notifikasi "opencode agent started — {folder}".
- [ ] **AC3:** Dalam ≤ 5 detik setelah opencode.exe berhenti, muncul notifikasi "opencode agent finished — exit code {code}".
- [ ] **AC4:** Klik notifikasi "started" membuka terminal/cmd di folder agent (default behavior; dapat dikonfigurasi ke `code` via setting `click_action`).
- [ ] **AC5:** Klik notifikasi "stopped" membuka terminal/cmd di folder agent (sama dengan AC4 — behavior seragam, dapat dikonfigurasi).
- [ ] **AC6:** System tray icon berubah/tooltip berisi count agent yang sedang hidup.
- [ ] **AC7:** Hanya satu instance aplikasi bisa berjalan.
- [ ] **AC8:** Auto-start toggle bekerja (register/unregister dari Windows startup).
- [ ] **AC9:** Aplikasi tetap berjalan walau tidak ada agent (idle state).
- [ ] **AC10:** Dynamic Island overlay muncul di atas tengah layar saat aplikasi running.
- [ ] **AC11:** Compact box idle menampilkan indikator jumlah agent hidup.
- [ ] **AC12:** Hover pada compact box memicu bouncing animation lalu expand ke list panel agent.
- [ ] **AC13:** Expanded list panel menampilkan instance-agent yang terdeteksi (PID, folder path).
- [ ] **AC14:** Binary hasil build berfungsi di Windows 10/11 tanpa install dependency tambahan.

---

## 9. Referensi

- [Tauri v2 Documentation](https://v2.tauri.app/)
- [Tauri Plugin Notification](https://v2.tauri.app/plugin/notification/)
- [Tauri Plugin Shell](https://v2.tauri.app/plugin/shell/)
- [Tauri System Tray](https://v2.tauri.app/guides/system-tray/)
- [Windows Process Enumeration (CreateToolhelp32Snapshot)](https://learn.microsoft.com/en-us/windows/win32/toolhelp/taking-a-snapshot-and-viewing-processes)
- [Windows Toast Notifications](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/toast-notifications)
- [QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
- [GetExitCodeProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getexitcodeprocess)
- [windows-rs crate](https://github.com/microsoft/windows-rs)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.3.0` | `2026-07-30` | Planner | Tambah §3 UI Spesifik (Dynamic Island overlay, rounded square, top center), update arsitektur diagram (ui_overlay/), T6 UI component, AC10-AC13 untuk UI, renumber T6-T10 → T7-T11 |
| `1.2.0` | `2026-07-30` | Planner | Tambah A5 (asumsi flag --cwd/--dir), resolve Q4, klarifikasi AC4/AC5 (terminal default), perbaiki metadata Author |
| `1.1.0` | `2026-07-30` | Planner (Phase 1) | Revisi berdasarkan review: metode working directory (Option 1 — Win32 API + parse argumen), konsistenkan klik notifikasi (→ terminal), metode notifikasi (→ tauri-plugin-notification), tambah tray dependency & asumsi, perbaiki alternatif & T3 |
| `1.0.0` | `2026-07-30` | Planner | Initial draft |
