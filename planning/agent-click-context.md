# Planning: Klik Item Agent Membuka Konteks Asal opencode (VSCode vs Terminal)

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Final` |
| Versi    | `1.3.0` |
| Tanggal  | `2026-08-01` |
| Author   | Planner |
| Reviewer | `Reviewer` (disetujui putaran ke-3, 3 pertanyaan kritis + 4 catatan toleransi — lihat Revisi History) |

---

## 1. Tujuan

Klik item agent di overlay Dynamic Island saat ini membuka folder sesuai setting global `click_action` — **default-nya** terminal (`cmd /K cd /d <path>`), tetapi jika user menyetel `click_action = "code"`, klik saat ini membuka VSCode (`open_vscode`) — karena dispatch di `App.svelte:140` memakai `state.click_action`. Perilaku ini tidak mengikuti konteks asal opencode. User ingin klik item agent membuka **konteks yang sama dengan tempat opencode dijalankan**:

- Jika opencode dibuka dari **VSCode** (integrated terminal) → buka VSCode di folder tersebut (`code <path>`).
- Jika opencode dibuka dari **terminal biasa** (cmd / PowerShell / Windows Terminal) → buka terminal di folder tersebut.

Deteksi konteks memakai **Plugin opencode sebagai sumber utama + fallback parent-chain detection di Rust**. Konteks disimpan per-agent sebagai field `launcher` (`"vscode"` / `"terminal"`). Setting global `click_action` hanya menjadi fallback jika launcher tidak diketahui.

**Measurable:**
- Klik item agent yang opencode-nya dijalankan di VSCode integrated terminal → terbuka jendela VSCode dengan `working_dir` tersebut (tidak membuka terminal baru).
- Klik item agent yang opencode-nya dijalankan di cmd/PowerShell/Windows Terminal → terbuka terminal baru dengan `working_dir` tersebut.
- Konteks yang dipakai ditentukan **per-agent** (bukan per-setting global): agent A (VSCode) dibuka di VSCode, agent B (terminal) dibuka di terminal, dalam polling yang sama.
- Tanpa plugin terpasang / file status stale → fallback parent-chain detection menghasilkan launcher yang benar untuk kedua skenario di atas (verified manual).
- Menyetel `click_action = "code"` **tidak** mengubah perilaku klik agent yang launcher-nya `"terminal"` (perilaku klik mengikuti launcher, bukan click_action).

---

## 2. Scope

### In Scope

- [ ] **Plugin opencode** — `.opencode/plugins/agent-status.ts`: deteksi launcher via env vars proses opencode, tulis field `launcher` ke payload file status `{APPDATA}/agent-monitor/agents/{pid}.json`.
- [ ] **Rust `status_reader.rs`** — parse field `launcher` (Option + whitelist), fungsi `read_state_and_launcher(pid) -> Option<(String, Option<String>)>` dengan TTL 30 detik yang sama dengan `read(pid)`; scanner memakai fungsi ini sehingga state + launcher dibaca dari file yang sama dalam **satu read per agent** (menggantikan pemakaian `read(pid)` di `scan_agents()`).
- [ ] **Rust `state.rs`** — field `launcher: String` di `AgentInfo` (default `"terminal"`), ter-serialize ke frontend.
- [ ] **Rust `process_scanner.rs`** — isi `launcher` dari status file; fallback parent-chain detection (walk `th32ParentProcessID` maks 5 hop, memakai `PROCESSENTRY32W`).
- [ ] **Rust `click_handler.rs`** — fungsi `open_for_launcher(path, launcher)` (dispatch `"vscode"` → `open_vscode`, `"terminal"` → `open_terminal`, tak dikenal → fallback `open_path_with_action` dengan `click_action` dari config).
- [ ] **Rust `lib.rs`** — command Tauri baru `open_for_launcher` + registrasi di `tauri::generate_handler!`.
- [ ] **Svelte `App.svelte`** — `openFolder(agent)` memakai `agent.launcher` (bukan `click_action`); indikator kecil konteks di baris agent (teks `VSCode` / `Terminal`).
- [ ] **Backward compatibility** — file status lama (tanpa field `launcher`) tetap berfungsi: `read_state_and_launcher` → INNER `None` → fallback parent-chain.
- [ ] **Testing & verifikasi** — manual test matrix kedua skenario (VSCode vs terminal), `cargo check`/`cargo build`.

### Out of Scope

- [ ] Perubahan tombol footer **"Open Terminal"** di panel — tetap memakai aksi eksplisit terminal (`open_terminal`), TIDAK berubah.
- [ ] Menghapus command `open_path` / `open_terminal` / `open_vscode` yang sudah ada — tetap dipertahankan untuk kompatibilitas (footer & command lain).
- [ ] Mengubah semantik setting global `click_action` — tetap ada & berlaku sebagai fallback ketika launcher tak diketahui; tidak dihapus.
- [ ] Deteksi editor lain selain VSCode (mis. IntelliJ, Cursor standalone, `code-insiders.exe`) — hanya `vscode` vs `terminal`.
- [ ] Dukungan non-Windows (WSL, macOS, Linux) — plan ini khusus Windows (monitor memakai `windows` crate / Toolhelp32).
- [ ] Perubahan pada event/hook opencode atau plugin status detection yang sudah ada (tidak menyentuh mapping status, debounce, heartbeat, atomic write).
- [ ] Unit test Rust (belum ada harness test di proyek ini) — verifikasi manual + `cargo build`.

---

## 3. Pendekatan

### Strategi Terpilih

**Deteksi launcher berlapis: Plugin (env vars) sebagai sumber utama → fallback parent-chain Rust jika file status tidak tersedia.**

#### Lapisan 1 — Plugin opencode (`agent-status.ts`)

Plugin sudah menulis file status per-PID. Tambahkan deteksi launcher: nilai dihitung **SATU KALI** di init, **SEBELUM** `flush()` pertama dipanggil (`agent-status.ts:115`) — env vars (`TERM_PROGRAM` / `VSCODE_*`) immutable per proses, sehingga tidak perlu evaluasi ulang saat menulis; **file pertama yang ditulis sudah memuat field `launcher`**:

```ts
function detectLauncher(): string {
  if (process.env.TERM_PROGRAM === "vscode") return "vscode"
  if (Object.keys(process.env).some((k) => k.startsWith("VSCODE_"))) return "vscode"
  return "terminal"
}
```

- `TERM_PROGRAM === "vscode"` di-set oleh VSCode pada integrated terminal; env `VSCODE_*` (mis. `VSCODE_GIT_ASKPASS`, `VSCODE_INJECTION`, `VSCODE_IPC_HOOK`) mengikuti proses yang lahir dari VSCode. Jika salah satu terpenuhi → `"vscode"`.
- `WT_SESSION` (Windows Terminal) **diabaikan** oleh deteksi — kehadirannya tidak mengubah hasil: fallback `"terminal"`.
- **Urutan init (Q3):** di awal fungsi plugin, set `const launcher = detectLauncher()` **sebelum** `mkdirSync`/`flush()` (`agent-status.ts:114-115`), lalu payload `flush()` menyertakan `launcher`. Karena env vars tidak berubah selama proses hidup, nilai ini konsisten untuk semua `flush()` berikutnya (init, debounce 250 ms, heartbeat 10 dtk) — tidak ada frasa "evaluasi ulang saat menulis".
- Field `launcher` ditambahkan ke payload file status. Payload menjadi:
  ```json
  {
    "status": "working",
    "pid": 1234,
    "cwd": "D:\\dev\\experiment-poni-agent",
    "launcher": "vscode",
    "timestamp": "2026-08-01T08:00:00.000Z"
  }
  ```
- **Backward compatibility:** payload lama tanpa `launcher` tetap valid — plugin versi lama yang tetap berjalan (sebelum update) menghasilkan file tanpa field; sisi Rust menghandle via `#[serde(default)]` → `None` → fallback parent-chain (Lapisan 2).
- Tidak ada perubahan pada debounce 250 ms, heartbeat 10 detik, atomic write (tmp → rmSync → rename), maupun hook mapping status — hanya menambah satu field.

#### Lapisan 2 — Fallback Rust parent-chain (`process_scanner.rs`)

Jika launcher **tidak** didapat dari file status (file tidak ada / stale melewati TTL / JSON tidak valid / field `launcher` tidak ada atau tak dikenal), `scan_agents()` memanggil helper baru untuk menelusuri parent chain proses `opencode.exe` memakai **parent map yang dibangun SATU KALI per scan**.

**Build parent map (sekali per `scan_agents()`):**

- `scan_agents()` sudah membuat satu snapshot `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` untuk enumerasi proses (baris 128-170). Dari snapshot **yang sama** — tidak ada snapshot tambahan — iterasi semua entry `PROCESSENTRY32W` dan bangun map `HashMap<u32, (u32, String)>` → `pid -> (th32ParentProcessID, szExeFile)` (nama dibaca sampai null-terminator, sama seperti di `scan_agents()` baris 137-139).
- Map di-pass ke helper untuk setiap agent. **Tidak ada snapshot terpisah per agent per polling** — saat plugin tidak terpasang (EC16), N agent menghasilkan 0 snapshot tambahan, bukan N snapshot.

**Detail walk parent chain (helper baru `detect_launcher_from_parent_chain(pid: u32, parent_map: &HashMap<u32, (u32, String)>) -> String`):**

1. Walk dari PID agent (`cur = pid`):
   - `for _ in 0..MAX_HOPS` (`MAX_HOPS = 5` — jumlah **hop** parent ke atas; node yang diperiksa = `opencode.exe` + hingga 5 ancestor):
     - `parent_map.get(&cur)` → ambil `(parent, name)`.
     - Bandingkan nama **case-insensitive** (`name.to_lowercase() == "code.exe"`) → jika cocok, return `"vscode"`.
     - Guard **loop tak terbatas**: jika `parent == cur` (contoh PID 4 `System` → parent dirinya sendiri) → break.
     - Guard parent mati: jika `parent` tidak ada di map → break.
     - `cur = parent`; lanjut.
   - Jika walk selesai tanpa menemukan `code.exe` → return `"terminal"`.
2. Setiap PID yang tidak ada di map (parent sudah mati / racy) → berhenti → `"terminal"`.

> **Mengapa sekali per scan cukup (Q2a):** launcher dianggap stabil dalam jangka pendek — `opencode.exe` dan chain parent-nya tidak berpindah induk secara reguler, dan ketika file status ada (plugin aktif) chain bahkan tidak dipakai. Parent map dibangun dari **snapshot yang sama** dengan enumerasi agent, sehingga seluruh keputusan dalam satu scan konsisten terhadap satu "citra" proses yang sama. Rebuild per polling (2 detik) menjaga hasil segar tanpa biaya tambahan per agent.

> **Race & reparenting (Q2b):** karena snapshot agent & parent map berasal dari **satu snapshot Toolhelp32 yang sama**, setiap PID yang ter-enum sebagai agent dijamin ada di parent map pada titik snapshot — tidak ada proses yang "hilang" di tengah scan. Satu-satunya ketidaksesuaian muncul bila sebuah proses mati **setelah** snapshot diambil lalu PID-nya **di-reparent** ke induk lain sebelum walk dieksekusi; walk akan mengikuti chain induk baru (bukan chain asli saat opencode lahir). Dampak: launcher bisa salah-tersimpulkan `"terminal"` (bila `code.exe` tidak ada di chain baru) atau `"vscode"` (bila chain baru menyentuh `code.exe`). Diterima: kedua arah hasil aman (fallback terminal membuka konteks yang ada; `"vscode"` membuka VSCode yang memang terbuka), dan polling berikutnya mengoreksi. Lihat EC20.

**Urutan penentuan `AgentInfo.launcher` di `scan_agents()` (SATU read per agent):**

```
// SATU read per agent: state + launcher dari file yang SAMA (mencegah TOCTOU — Q1)
let (state, launcher) = status_reader::read_state_and_launcher(pid);
agent.state = state.unwrap_or("running".to_string());       // OUTER None (absent/stale/invalid) → fallback "running"
match launcher {                                             // INNER Option: nilai launcher
    Some(l) => agent.launcher = l,                           // file fresh + whitelist → file = otoritatif
    None => agent.launcher = detect_launcher_from_parent_chain(pid, &parent_map), // fallback (stale/tua/tak valid)
}
```

> Catatan penting: **fallback parent-chain OVERRIDE default `"terminal"`.** Alasan: `"terminal"` adalah default awal, sehingga pengecekan "apakah launcher sudah diketahui" harus berbasis "apakah file memberikan nilai valid", bukan berbasis nilai default. Jika file tidak memberikan launcher valid → parent-chain menentukan (bisa menghasilkan `"vscode"` walau default-nya terminal). Ini memenuhi keputusan user: *"jika file status tidak ada / stale / tidak valid → parent-chain detection"*.

> **Catatan TOCTOU (Q1):** `scan_agents()` **tidak lagi memanggil `read(pid)` terpisah** untuk state (panggilan lama di proses_scanner.rs:173 dihapus/diganti). `read_state_and_launcher(pid)` membaca **satu file sekali** dan me-return `(state, launcher)` dari versi file yang sama pada titik waktu yang sama — tidak mungkin state & launcher berasal dari dua versi berbeda (plugin menulis ulang file via atomic write `tmp → rmSync → rename`, yang bisa terjadi di antara dua read). Opsi `read_launcher(pid)` **tidak dipakai** scanner (tidak ada dual-option). Fungsi `prefer_status_file_source` (proses_scanner.rs:182-217) **tidak disentuh**: ia hanya memodifikasi `state` (menormalkan agent duplikat per `working_dir` ke `"running"`), tidak membaca/menulis `launcher` — sehingga tetap aman dipanggil setelah read gabungan.

> **Catatan startup monitor (non-blocker reviewer, resolusi Q21):** `clear_agents_dir()` dipanggil di `setup` **sebelum** polling dimulai (`lib.rs:124`) menghapus SEMUA file status setiap monitor start → beberapa poll pertama selalu melewati jalur parent-chain (bukan file) sampai plugin menulis ulang (init `flush()` segera setelah plugin dimuat + heartbeat ≤ 10 dtk). Tidak kontradiksi dengan "file = otoritatif": selama jendela itu tidak ada file fresh sehingga file memang tidak bisa jadi otoritatif → fallback parent-chain adalah perilaku yang benar sesuai aturan; hasil launcher sementara (`"terminal"`) terkoreksi saat file tertulis (polling ≤ 2 dtk). Lihat EC16.

**Alur data:**

```
opencode process env (TERM_PROGRAM / VSCODE_*)
   │
   ▼
Plugin agent-status.ts ──detectLauncher()──▶ launcher: "vscode" | "terminal"
   │  (debounce 250ms, heartbeat 10s, atomic write — tidak berubah)
   ▼
%APPDATA%/agent-monitor/agents/{pid}.json  (payload + field "launcher")
   │
   ▼
┌────────────────────────────────────────────┐
│ scan_agents() per polling (2 dtk)           │
│   (state, launcher) ← status_reader::read_state_and_launcher(pid)  // SATU read
│   if launcher is None → detect_launcher_from_parent_chain(pid, &parent_map)
│        parent_map: dibangun SATU KALI per scan dari snapshot yang sama
│        → walk th32ParentProcessID ≤ 5 hop → "code.exe"?
│   AgentInfo { pid, exe_path, working_dir, state, launcher }
└────────────────────────────────────────────┘
   │ emit "agent-update" (AgentInfo termasuk launcher)
   ▼
App.svelte: klik item agent
   │  openFolder(agent) → invoke('open_for_launcher', { path, launcher: agent.launcher ?? '' })
   ▼
lib.rs: open_for_launcher → click_handler::open_for_launcher(path, launcher, config.click_action)
   │  "vscode"    → open_vscode(path)      = code <path> (fallback terminal bila `code` tak ada)
   │  "terminal"  → open_terminal(path)    = cmd /C start cmd /K cd /d <path>
   │  lain (fallback) → open_path_with_action(path, config.click_action)
```

**Skenario parent-chain yang diharapkan (Windows):**

| Skenario | Chain (dari opencode ke atas) | Hasil |
|----------|------------------------------|-------|
| VSCode integrated terminal (biasa) | `opencode.exe` → `pwsh.exe`/`cmd.exe` → `Code.exe` | `vscode` (ditemukan di hop 2) |
| VSCode via ConPTY (ada node conpty) | `opencode.exe` → `pwsh.exe` → `conhost.exe`/`OpenConsole.exe` → `Code.exe` | `vscode` (hop 3 ≤ 5) |
| VSCode dengan node perantara ekstra | `opencode.exe` → `pwsh.exe` → `OpenConsole.exe` → `conhost.exe` → `Code.exe` (atau shell berlapis) | `vscode` (hop 4 ≤ 5) |
| VSCode via chain sangat panjang (> 5 hop) | `opencode.exe` → … 6+ hop → `Code.exe` | `terminal` (silent fallback — lihat catatan MAX_HOPS) |
| Windows Terminal | `opencode.exe` → `pwsh.exe` → `WindowsTerminal.exe` → `explorer.exe` → … | `terminal` (tidak ada `code.exe`) |
| cmd/konsole biasa | `opencode.exe` → `cmd.exe` → `conhost.exe` → `explorer.exe` → … | `terminal` |
| Agent dijalankan langsung (explorer) | `opencode.exe` → `explorer.exe` → … | `terminal` |

Node perantara (`conhost.exe`, `OpenConsole.exe`, `WindowsTerminal.exe`, `explorer.exe`) **tidak menghentikan walk** — mereka bukan `code.exe`, walk tetap lanjut ke ancestor berikutnya sampai hop 5 (`MAX_HOPS`).

> **Kecukupan `MAX_HOPS = 5` untuk VSCode nyata (Q6):** chain VSCode integrated terminal nyata berisi 2–4 hop (`opencode.exe → <shell> → [OpenConsole.exe] → Code.exe`). Bila ada node ConPTY tambahan (`conhost.exe`/`OpenConsole.exe`) dan/atau wrapper shell berlapis, total hop dari `opencode.exe` ke `Code.exe` praktis tidak melebihi 4 → `MAX_HOPS = 5` menyisakan 1 hop cadangan. Jika konfigurasi eksotis menghasilkan chain > 5 hop, hasilnya `"terminal"` secara **silent** (tanpa error) — diterima karena tetap membuka konteks yang benar-benar ada, tanpa regresi terhadap perilaku lama. Lihat R2, EC21, AC15.

**Command Tauri & frontend:**

- `lib.rs`: command baru
  ```rust
  #[tauri::command]
  fn open_for_launcher(
      state: tauri::State<'_, AppState>,
      path: String,
      launcher: String,
  ) -> Result<(), String> {
      let fallback = state.config.lock().map(|c| c.click_action.clone()).unwrap_or_default();
      click_handler::open_for_launcher(&path, &launcher, &fallback).map_err(|e| e.to_string())
  }
  ```
  Daftarkan `open_for_launcher` di `generate_handler!` (lib.rs:142-150).
- `click_handler.rs`:
  ```rust
  pub fn open_for_launcher(path: &str, launcher: &str, fallback_action: &str) -> Result<()> {
      match launcher {
          "vscode" => open_vscode(path),
          "terminal" => open_terminal(path),
          _ => open_path_with_action(path, fallback_action),
      }
  }
  ```
- `App.svelte` `openFolder(agent)`:
  ```js
  async function openFolder(agent) {
    try {
      await invoke('open_for_launcher', { path: agent.working_dir, launcher: agent.launcher ?? '' });
    } catch (e) { console.error('Failed to open path:', e); }
  }
  ```
  Panggilan `get_config` dihapus dari `openFolder` — fallback click_action ditangani di sisi Rust (arm `_` pada `click_handler::open_for_launcher`). Indikator konteks ditambahkan di baris agent (mis. badge kecil `VSCode`/`Terminal` di samping status).

> **Keputusan fallback "launcher tak dikenal" (resolusi Q17):** frontend memakai **`agent.launcher ?? ''`** (BUKAN `|| 'terminal'`) — hanya `null`/`undefined` yang di-coalesce ke `""`, sehingga launcher yang benar-benar tak diketahui diteruskan apa adanya ke Rust dan masuk arm `_`. `open_for_launcher` (Rust): `"vscode"` → `open_vscode`, `"terminal"` → `open_terminal`, **selain itu** (termasuk `""`/nilai aneh) → `open_path_with_action(path, config.click_action)` (fallback global = perilaku lama). Jalur ini **HANYA reachable via payload lama/undefined** (mis. `agent-update` lama saat hot-reload frontend, atau field `launcher` hilang), karena `scan_agents()` selalu menormalkan `launcher` ke whitelist `{vscode, terminal}` → normalisasi scan membuat jalur ini jarang (defense-in-depth). Tidak ada kontradiksi: frontend tidak pernah mengubah `"terminal"` menjadi `""` (hanya undefined yang di-coalesce), dan `""`/nilai aneh tidak pernah dihasilkan oleh scan.

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **Deteksi murni di Rust (parent-chain saja, tanpa plugin)** | Plugin dapat mendeteksi via env vars `TERM_PROGRAM`/`VSCODE_*` yang **tidak tersedia** di sisi monitor (monitor tidak melihat env proses lain tanpa hak istimewa). Plugin lebih akurat & murah (0 snapshot tambahan). Parent-chain tetap dipakai sebagai fallback sesuai keputusan user. |
| **Deteksi murni di plugin (env vars saja, tanpa fallback Rust)** | Jika file status stale/absent (plugin mati, versi lama, write gagal), launcher hilang → perilaku klik kembali tak pasti. Fallback Rust menjamin konteks benar walau plugin tidak memberikan data. |
| **Satu setting global `launcher` dipakai semua agent** | Tidak memenuhi kebutuhan per-agent — dua konteks berbeda (VSCode & terminal) bisa hidup bersamaan di mesin yang sama; klik harus mengikuti launcher masing-masing agent. |
| **Deteksi via window title / foreground window** | Fragile & tidak deterministik (agent background tidak punya window aktif); parent-chain lebih terstruktur. |
| **Hitung ulang launcher tiap klik (re-scan saat event click)** | Memindahkan biaya deteksi ke event handler dan memerlukan akses snapshot di thread command; menentukan sekali per polling lebih sederhana & konsisten dengan data yang sudah di-`emit`. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Plugin mendeteksi `"terminal"` padahal sebenarnya VSCode (mis. `TERM_PROGRAM` tidak ter-set di beberapa skenario VSCode remote/embedded) | Rendah | Sedang | Deteksi memakai **dua** sinyal (`TERM_PROGRAM === "vscode"` ATAU ada env `VSCODE_*`). File yang ditulis plugin = otoritatif; bila masih keliru, fallback parent-chain tidak menimpa file fresh — didokumentasikan sebagai limitasi. |
| R2: Chain parent lebih dalam dari 5 hop sehingga `Code.exe` terlewat | Rendah | Rendah | MAX_HOPS = 5 menutup chain VSCode nyata (`opencode→shell→[conpty]→Code.exe` = 2–4 hop; bahkan dengan node ekstra = 4). Jika terlewat → hasil `"terminal"` **silent** (Q6): aman, tetap membuka konteks yang benar-benar ada, tanpa regresi. WSL di luar scope (Windows only). |
| R3: Snapshot racy — parent proses mati di tengah walk / tidak ada di map | Rendah | Rendah | Walk stop pada parent yang tidak ditemukan → `"terminal"`. Polling berikutnya (≤ 2 detik) mengoreksi. Tidak pernah infinite loop. |
| R4: Loop tak terbatas (parent = dirinya sendiri, mis. PID 4 `System`) | Sangat Rendah | Tinggi | Guard eksplisit `if parent == cur { break }` + batas iterasi `MAX_HOPS = 5` → walk dijamin terminasi. |
| R5: `code.exe` bukan satu-satunya nama VSCode (mis. `code-insiders.exe`) | Rendah | Rendah | Hanya whitelist `"code.exe"` (case-insensitive) sesuai keputusan; `code-insiders` → `"terminal"`. Di luar scope, dicatat sebagai limitasi. |
| R6: Nilai `launcher` di file status tak dikenal/corrupt | Rendah | Rendah | Whitelist di `read_state_and_launcher` (`["vscode", "terminal"]`); nilai lain → INNER `None` → fallback parent-chain. Tidak crash. |
| R7: File status lama (tanpa field `launcher`) | Sedang | Rendah | `#[serde(default)]` → `None` → fallback parent-chain. Semua file lama tetap berfungsi. |
| R8: `code` CLI tidak terpasang saat launcher `"vscode"` | Rendah | Sedang | Perilaku existing `open_vscode` (click_handler.rs:11-20) sudah fallback ke `open_terminal` + pesan error. Tidak ada regresi. |
| R9: Biaya snapshot untuk fallback parent-chain saat plugin tidak terpasang | Rendah | Rendah | Parent map dibangun **sekali per `scan_agents()`** dari snapshot yang sudah dipakai enumerasi proses (baris 128-170) — **0 snapshot tambahan per agent per polling** (Q2). Walk per agent O(hop ≤ 5) di atas map O(N) sekali per scan. |
| R10: Ketidakcocokan plugin vs scanner (file fresh bilang `"terminal"`, chain bilang `"vscode"`) | Rendah | Rendah | Keputusan desain: **file = otoritatif** selama fresh & valid. Chain hanya jalan ketika file tidak memberi jawaban. Konsisten dengan keputusan user. |

### Edge Case

- [ ] **EC1: opencode di VSCode integrated terminal** — env berisi `TERM_PROGRAM=vscode` (dan/atau `VSCODE_*`) → plugin menulis `launcher: "vscode"` → klik membuka VSCode.
- [ ] **EC2: opencode di Windows Terminal** — `WT_SESSION` ter-set, `TERM_PROGRAM`/`VSCODE_*` tidak → `"terminal"` (WT_SESSION sengaja diabaikan).
- [ ] **EC3: opencode di cmd/pwsh biasa** — tidak ada sinyal VSCode → `"terminal"`.
- [ ] **EC4: `launcher: "vscode"` tapi `code` CLI tidak ditemukan** — `open_vscode` fallback ke `open_terminal` + error message (perilaku existing, tidak berubah).
- [ ] **EC5: File status lama tanpa field `launcher`** — `read_state_and_launcher` → INNER `None` → fallback parent-chain.
- [ ] **EC6: File status stale (mtime > 30 dtk)** — `read_state_and_launcher` → OUTER `None` → state `"running"`, parent-chain fallback untuk launcher.
- [ ] **EC7: File status JSON tidak valid** — parse gagal → `None` → fallback parent-chain; polling lanjut, tidak crash.
- [ ] **EC8: Parent proses sudah mati** — walk stop → `"terminal"`.
- [ ] **EC9: Chain terminal berisi `WindowsTerminal.exe` / `conhost.exe` / `OpenConsole.exe` / `explorer.exe`** — node perantara bukan `code.exe`, walk lanjut → akhirnya `"terminal"` (benar).
- [ ] **EC10: Chain VSCode dengan node ConPTY** — `opencode → pwsh → conhost/OpenConsole → Code.exe` → ditemukan di hop 3 ≤ 5 → `"vscode"`.
- [ ] **EC11: Walk mencapai PID 4 `System` (parent dirinya sendiri)** — guard `parent == cur` → break → `"terminal"`.
- [ ] **EC12: Dua agent dengan launcher berbeda** — field `launcher` per-agent; klik agent A → VSCode, klik agent B → terminal, dalam panel yang sama.
- [ ] **EC13: `click_action` di-set `"code"` oleh user, agent launcher-nya `"terminal"`** — klik tetap membuka terminal (launcher menang); click_action hanya dipakai jika launcher tak dikenal (EC14).
- [ ] **EC14: Launcher tak dikenal (nilai aneh dari file / data lama / undefined)** — frontend `agent.launcher ?? ''` meneruskan nilai tak dikenal/undefined apa adanya ke Rust → arm `_` `open_for_launcher` → `open_path_with_action(path, config.click_action)` (perilaku lama). Jalur ini HANYA reachable via payload lama/undefined (defense-in-depth), karena `scan_agents()` menormalkan launcher ke whitelist `{vscode, terminal}` — nilai aneh tidak pernah dihasilkan scan (resolusi Q17).
- [ ] **EC15: PID reuse cepat (restart < 30 dtk)** — plugin baru menimpa file saat init (launcher baru ikut tertulis); jika file belum ditulis, TTL + parent-chain menangani.
- [ ] **EC16: Plugin tidak terpasang sama sekali** — tidak ada file → untuk semua agent parent-chain dijalankan: opencode di VSCode → `vscode`, di terminal → `terminal`. Perilaku klik tetap mengikuti konteks asal, bukan selalu terminal. **Catatan startup monitor (non-blocker reviewer):** `clear_agents_dir()` dipanggil di setup sebelum polling dimulai (`lib.rs:124`) menghapus **semua** file status → beberapa poll pertama setelah monitor start melewati jalur parent-chain sampai plugin menulis ulang (init flush segera, lalu heartbeat ≤ 10 dtk). Ini tidak kontradiksi dengan "file = otoritatif": selama jendela itu tidak ada file fresh → file memang belum bisa jadi otoritatif → fallback parent-chain sesuai aturan; hasil launcher bisa `"terminal"` sementara lalu terkoreksi saat file tertulis (polling ≤ 2 dtk).
- [ ] **EC17: opencode di WSL / non-Windows** — di luar scope; parent-chain memakai Toolhelp32 yang khusus Windows, hasil di platform lain tidak dijamin (plan ini Windows-only).
- [ ] **EC18: VSCode window sudah tertutup tapi file status masih fresh bilang `"vscode"`** — klik `code <path>` → VSCode membuka jendela baru (default `code` behavior). Tidak ada crash.
- [ ] **EC19: `AgentInfo.launcher` undefined/absent di payload `agent-update` lama saat hot-reload frontend** — `agent.launcher ?? ''` di `App.svelte` meneruskan undefined apa adanya (`""`) ke Rust (BUKAN di-coerce ke `"terminal"`) → arm `_` → `open_path_with_action(path, config.click_action)` (perilaku lama). Setelah `agent-update` berikutnya, launcher terisi hasil scan (whitelist `{vscode, terminal}`). Konsisten dengan EC14 (resolusi Q17).
- [ ] **EC20: Proses induk mati lalu di-REPARENT di antara snapshot & walk (Q2b)** — chain yang dilacak menjadi chain baru (bukan chain asli saat opencode lahir). Hasil tetap salah satu dari `"vscode"`/`"terminal"` yang aman: `"vscode"` hanya jika `code.exe` ada di ancestor saat ini (VSCode memang terbuka), `"terminal"` bila tidak. Tidak ada hang/infinite loop; polling berikutnya mengoreksi. Snapshot agent & parent map berasal dari snapshot Toolhelp32 yang sama → tidak ada PID agent yang absen dari map.
- [ ] **EC21: Chain parent > 5 hop padahal ancestor menyentuh `Code.exe` (Q6)** — walk berhenti di hop 5 (`MAX_HOPS = 5`) tanpa menemukan `code.exe` → hasil `"terminal"` **silent** (tanpa pesan error). Diterima: membuka terminal di folder yang sama tetap membuka konteks yang benar-benar ada; skenario ini tidak menghentikan/merusak polling.

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| serde | 1 (sudah ada di `Cargo.toml`) | Deserialize field `launcher` di `status_reader.rs` |
| serde_json | 1 (sudah ada) | Parsing file status |
| windows | 0.58 (sudah ada, feature `Win32_System_Diagnostics_ToolHelp`) | `CreateToolhelp32Snapshot` / `PROCESSENTRY32W` / `th32ParentProcessID` untuk parent-chain walk |
| anyhow | 1 (sudah ada) | Error handling di `click_handler.rs` |

> **Tidak ada dependency baru.** Plugin opencode memakai Node `process.env` bawaan — tanpa dependency eksternal.

### Service

| Service | Tujuan |
|---------|--------|
| N/A | Semuanya lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| `status_reader.rs::read_state_and_launcher(pid)` (baru) | Sumber state+launcher utama dari file status (TTL sama dengan `read`); dipakai scanner menggantikan `read(pid)` — SATU read per agent (Q1) |
| `state.rs::AgentInfo` (tambah field `launcher`) | Membawa launcher dari scanner → frontend via `agent-update` / `get_agents` |
| `process_scanner.rs::scan_agents()` | Mengisi `AgentInfo.launcher` (read_state_and_launcher dulu, parent-chain fallback memakai parent map sekali per scan) |
| `click_handler.rs::open_vscode` / `open_terminal` / `open_path_with_action` | Dasar dispatch `open_for_launcher` (semua sudah ada) |
| `config.rs::agents_dir()` | Path file status (`{pid}.json`) — sudah dipakai `status_reader` |
| `lib.rs::polling_loop` + command `open_for_launcher` (baru) | Orchestrasi scan → emit; entry point klik dari frontend |
| `App.svelte::openFolder` (ubah) | Konsumen `agent.launcher`; sumber event klik |
| Plugin opencode `.opencode/plugins/agent-status.ts` | Sumber deteksi env var (Lapisan 1) |

> **Catatan serde `AgentInfo` (non-blocker reviewer, resolusi Q22):** saat ini Rust **tidak** mendeserialisasi `AgentInfo` dari JSON — `AgentInfo` hanya di-`Serialize` ke frontend (`agent-update`/`get_agents`; derive `Deserialize` ada tapi tidak dipakai untuk struct ini). Karena itu menambah field `launcher: String` TIDAK butuh `#[serde(default)]` sekarang. Jika di masa depan `AgentInfo` dideserialisasi dari JSON (mis. save/load snapshot), tambahkan `#[serde(default = ...)]` ber-default `"terminal"` agar kompatibel mundur. **Berbeda** dengan `StatusFile.launcher` di `status_reader.rs` yang memang di-deserialize dari payload plugin — di sana `#[serde(default)]` **wajib** (T2).

---

## 6. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [ ] **T1: Plugin `agent-status.ts` — deteksi launcher** — tambah `detectLauncher()` (env check `TERM_PROGRAM === "vscode"` ATAU ada key `VSCODE_*`, else `"terminal"`); hitung **sekali** `const launcher = detectLauncher()` di awal init **SEBELUM** `mkdirSync`/`flush()` (`agent-status.ts:114-115`) — env vars immutable, tidak ada evaluasi ulang saat menulis (Q3); tambahkan field `launcher` ke payload `flush()` sehingga file pertama yang ditulis sudah memuatnya. **Tidak** mengubah debounce/heartbeat/atomic write/mapping status. Verifikasi manual: log payload ke console (debug) untuk memastikan field tertulis sejak flush pertama. [S]
- [ ] **T2: Rust `status_reader.rs` — parse launcher + read gabungan** — tambah `#[serde(default)] launcher: Option<String>` di struct `StatusFile`; konstanta `VALID_LAUNCHERS: [&str; 2] = ["vscode", "terminal"]`; refactor internal `fn read_fresh(pid) -> Option<StatusFile>` (cek `is_fresh` + parse + return struct); fungsi utama **`pub fn read_state_and_launcher(pid) -> Option<(String, Option<String>)>`** (state + launcher dari file yang sama): **OUTER `None`** saat file absent/stale/JSON invalid/status invalid (state fallback `"running"` — semantik `read()` lama dipertahankan), **INNER `None`** untuk launcher saat file fresh namun field `launcher` missing/tak dikenal (agar parent-chain fallback tetap jalan — Q4). `read()` lama boleh dipertahankan sebagai wrapper kompatibilitas; `read_launcher(pid)` tidak wajib ada — scanner WAJIB memakai `read_state_and_launcher` (Q1). [S]
- [ ] **T3: Rust `state.rs` — field launcher** — tambah `pub launcher: String` di `AgentInfo`; saat struct literal dibuat, default `"terminal".to_string()`. Tidak ada perubahan pada `Config`/`AppState`. **Catatan serde (resolusi Q22):** `AgentInfo` saat ini hanya di-`Serialize` (tidak pernah di-`Deserialize` dari JSON) → field baru TIDAK butuh `#[serde(default)]` sekarang; jika nanti ada deserialisasi `AgentInfo`, tambahkan `#[serde(default = ...)]` ber-default `"terminal"` untuk kompatibilitas mundur (beda dengan `StatusFile.launcher` di T2 yang wajib `#[serde(default)]` karena memang dideserialisasi). [S]
- [ ] **T4: Rust `process_scanner.rs` — isi launcher + parent-chain** — (a) di `scan_agents()`, setelah snapshot proses diambil (baris 128-170), bangun parent map `HashMap<u32, (u32, String)>` **SATU KALI** dari snapshot yang sama; helper baru `fn detect_launcher_from_parent_chain(pid: u32, parent_map: &HashMap<u32, (u32, String)>) -> String` menerima map tsb (tidak membuat snapshot sendiri — Q2). Walk ≤ 5 hop (`MAX_HOPS = 5` — opencode + hingga 5 ancestor) dengan guard `parent == cur` & `parent` tidak ada di map, bandingkan `name.to_lowercase() == "code.exe"` → `"vscode"`, selain itu `"terminal"`. (b) di `scan_agents()`, **ganti panggilan `read(pid)` (proses_scanner.rs:173) dengan satu panggilan `status_reader::read_state_and_launcher(pid)`** yang mengisi state & launcher sekaligus (SATU read per agent — Q1; tidak ada opsi `read_launcher`); launcher dari file (whitelist) → otoritatif, `None` → fallback helper dengan parent_map. **Wajib: fallback parent-chain OVERRIDE default `"terminal"`** (lihat catatan §3) — jangan jadikan default sebagai "sudah diketahui". Fungsi `prefer_status_file_source` (proses_scanner.rs:182-217) **tidak disentuh** — hanya menyentuh `state`, tidak menyentuh `launcher` (Q1). [M]
- [ ] **T5: Rust `click_handler.rs` — `open_for_launcher`** — tambah `pub fn open_for_launcher(path: &str, launcher: &str, fallback_action: &str) -> Result<()>`: `"vscode"` → `open_vscode(path)`, `"terminal"` → `open_terminal(path)`, `_` → `open_path_with_action(path, fallback_action)`. Arm `_` (fallback global) menangani SEMUA nilai selain `"vscode"`/`"terminal"`, termasuk `""` dari `agent.launcher ?? ''` (resolusi Q17). [S]
- [ ] **T6: Rust `lib.rs` — command `open_for_launcher`** — command Tauri baru yang membaca `config.click_action` sebagai fallback (lock `state.config`) lalu memanggil `click_handler::open_for_launcher`; daftarkan `open_for_launcher` di `generate_handler!` (lib.rs:142-150). Command `open_path`/`open_terminal`/`open_vscode` tetap dipertahankan. [S]
- [ ] **T7: Svelte `App.svelte` — openFolder + indikator konteks** — (a) ubah `openFolder` menjadi `openFolder(agent)` → `invoke('open_for_launcher', { path: agent.working_dir, launcher: agent.launcher ?? '' })` (**`?? ''`**, BUKAN `|| 'terminal'` — undefined diteruskan apa adanya ke arm `_` Rust → fallback click_action; resolusi Q17); hapus pemanggilan `get_config` dari fungsi ini; (b) update call-site baris agent (`on:click|stopPropagation={() => openFolder(agent)}`); (c) tambah indikator kecil konteks di baris agent (mis. badge teks `VSCode`/`Terminal` di samping status, style konsisten dengan `.badge`/`.status-label` yang ada). Footer "Open Terminal" TIDAK diubah. [S]
- [ ] **T8: Verifikasi manual & build** — `cargo check` + `cargo build` di `agent-monitor/src-tauri` (tanpa dependency baru); `npm run build`/dev di frontend; test matrix: (1) opencode di VSCode integrated terminal → klik buka VSCode, (2) opencode di cmd/pwsh/Windows Terminal → klik buka terminal, (3) tanpa plugin → parent-chain benar untuk kedua skenario, (4) file lama tanpa launcher → fallback, (5) `click_action="code"` + launcher `"terminal"` → tetap terminal, (6) footer Open Terminal masih eksplisit terminal, (7) dua agent launcher berbeda dalam satu panel. **Untuk jalur "launcher tak dikenal" (tidak reachable via UI karena scan menormalkan ke whitelist):** test via **invoke langsung** — panggil `invoke('open_for_launcher', { path, launcher: 'foo' })` dan `invoke('open_for_launcher', { path, launcher: '' })` → cek membuka sesuai `config.click_action` tanpa crash (resolusi Q19). [M]

**Dependency antar-task:**
- T1 (plugin) independen — bisa paralel dengan T2–T7.
- T2 → T4 → T7 (reader dulu → scanner isi launcher → frontend konsumsi `agent.launcher`).
- T3 → T4 (field `launcher` harus ada sebelum scanner mengisinya).
- T4 → T6 (command memakai launcher dari `AgentInfo`, walau secara kode hanya butuh string).
- T5 → T6 (command memanggil `click_handler::open_for_launcher`).
- T7 → T6 (frontend memanggil command `open_for_launcher`).
- T8 (verifikasi) setelah T1–T7.

---

## 7. Open Questions

- [x] **Q1: Siapa sumber otoritatif launcher?** — **Keputusan final:** plugin (file status fresh + valid) = otoritatif; parent-chain hanya fallback saat file tidak ada/stale/tidak valid. Sesuai keputusan user.
- [x] **Q2: Sinyal env apa untuk deteksi VSCode di plugin?** — **Keputusan final:** `TERM_PROGRAM === "vscode"` ATAU keberadaan env `VSCODE_*` → `"vscode"`; selainnya (termasuk jika hanya ada `WT_SESSION`) → `"terminal"`.
- [x] **Q3: Berapa max hop parent-chain?** — **Keputusan final:** 5 hop (`MAX_HOPS = 5`) — jumlah **hop** parent ke atas; node yang diperiksa = opencode + hingga 5 ancestor. Menutup chain tipikal `opencode → shell → [conpty] → Code.exe` (hop 2–4).
- [x] **Q4: Nama exe apa yang dianggap VSCode?** — **Keputusan final:** `"code.exe"` case-insensitive. `code-insiders.exe` dan editor lain di luar scope.
- [x] **Q5: Di mana fallback `click_action` dieksekusi?** — **Keputusan final:** di sisi Rust — command `open_for_launcher` membaca `state.config.click_action` dan mem-pass sebagai `fallback_action` ke `click_handler::open_for_launcher`. Frontend cukup pass `agent.launcher` (dengan `agent.launcher ?? ''` agar nilai undefined masuk arm `_` — lihat Q17).
- [x] **Q6: Bagaimana kalau default `"terminal"` menutup kasus "launcher tak diketahui"?** — **Keputusan final:** fallback parent-chain **override** default. "Tak diketahui" didefinisikan sebagai "file tidak memberikan nilai valid", bukan "nilai = terminal". Lihat catatan §3.
- [x] **Q7: Apakah field `launcher` memakai TTL yang sama?** — **Keputusan final:** ya, TTL 30 detik via file mtime (dipakai bersama `read()` via refactor `read_fresh`).
- [x] **Q8: Apakah tombol footer "Open Terminal" berubah?** — **Keputusan final:** tidak. Tetap `invoke('open_terminal', ...)` eksplisit.
- [x] **Q9: Apakah command `open_path`/`open_vscode`/`open_terminal` dihapus?** — **Keputusan final:** tidak. Dipertahankan untuk kompatibilitas (footer pakai `open_terminal`; `open_vscode`/`open_path_with_action` dipakai internal oleh `open_for_launcher`).
- [x] **Q10: Apakah parent-chain memakai snapshot baru atau reuse snapshot `scan_agents()`?** — **Keputusan final (revisi v1.1.0):** parent map dibangun **SATU KALI per `scan_agents()`** dari snapshot Toolhelp32 yang sama dengan enumerasi proses (baris 128-170) dan di-pass ke helper — bukan snapshot terpisah per agent (resolusi Q2a). Lihat §3 Lapisan 2.
- [x] **Q11 (reviewer): TOCTOU double-read file status?** — **Keputusan final:** `scan_agents()` mengganti panggilan `read(pid)` dengan satu panggilan `read_state_and_launcher(pid)` → state & launcher dari versi file yang sama (resolusi Q1). Lihat §3 catatan TOCTOU, T2, T4.
- [x] **Q12 (reviewer): race/reparenting saat parent mati lalu di-reparent?** — **Keputusan final:** walk bisa mengikuti chain baru, hasil tetap aman (vscode/terminal) — diterima (resolusi Q2b). Lihat §3 catatan race, EC20.
- [x] **Q13 (reviewer): launcher vs `flush()` init pertama?** — **Keputusan final:** launcher dihitung sekali sebelum `flush()` init (`agent-status.ts:115`), file pertama sudah ber-field `launcher` (resolusi Q3). Lihat §3 Lapisan 1, T1.
- [x] **Q14 (reviewer): preservasi semantik read saat refactor?** — **Keputusan final:** OUTER `None` (absent/stale/invalid → state fallback `"running"`), INNER `None` (file fresh, launcher missing/invalid → parent-chain fallback jalan) (resolusi Q4). Lihat §3 urutan penentuan, T2.
- [x] **Q15 (reviewer): klaim "selalu membuka terminal"?** — **Keputusan final:** §1 direvisi — default terminal, tapi `click_action = "code"` membuka VSCode (resolusi Q5). Lihat §1.
- [x] **Q16 (reviewer): MAX_HOPS=5 cukup untuk chain VSCode nyata?** — **Keputusan final:** cukup (chain nyata 2–4 hop); chain > 5 hop → `"terminal"` silent, diterima (resolusi Q6). Lihat §3 skenario, R2, EC21, AC15.
- [x] **Q17 (reviewer): fallback "launcher tak dikenal" kontradiktif (EC19 vs EC14 vs §3)?** — **Keputusan final (resolusi Q2):** frontend memakai `agent.launcher ?? ''` (bukan `|| 'terminal'`) sehingga launcher yang benar-benar tak diketahui (undefined) diteruskan apa adanya ke Rust; `open_for_launcher`: `"vscode"` → `open_vscode`, `"terminal"` → `open_terminal`, selain itu → `open_path_with_action(path, click_action)`. Jalur "launcher tak dikenal" hanya reachable via payload lama/undefined (defense-in-depth) karena `scan_agents()` menormalkan ke whitelist. Lihat §3, T5/T7, EC14, EC19, AC9.
- [x] **Q18 (reviewer): terminologi MAX_DEPTH tidak konsisten (node vs hop)?** — **Keputusan final (resolusi Q3):** seragamkan ke satuan **hop** — `MAX_HOPS = 5` (jumlah hop parent ke atas; node yang diperiksa = opencode + hingga 5 ancestor; ≤ 5 hop = ≤ 5 ancestor diperiksa setelah opencode). Konsisten di §2/§3/tabel skenario, R2/R4/R9, EC10/EC21, T4, Q3/Q16, AC15.
- [x] **Q19 (reviewer): T8(8)/AC9 tidak testable via UI?** — **Keputusan final (resolusi Q4):** skenario "launcher tak dikenal → fallback click_action" dihapus dari matrix UI test manual (tidak reachable karena normalisasi scan) dan dijadikan test via invoke langsung — panggil command `open_for_launcher` dengan launcher `"foo"` / `""` → cek memakai `config.click_action`. Lihat T8, AC9.
- [x] **Q20 (reviewer): referensi baris §1/§9 salah?** — **Keputusan final (resolusi Q1):** `App.svelte:117` (collapse) dikoreksi ke `App.svelte:140` (dispatch `open_path` memakai `state.click_action`). Audit §9 & semua referensi terhadap kode aktual: `generate_handler!` dikoreksi dari 148-156 → **lib.rs:142-150**; referensi lain terverifikasi benar (`agent-status.ts:114-115/:115`, `process_scanner.rs:128-170/:137-139/:173/:182-217`, `click_handler.rs:11-20`, `lib.rs:124` untuk `clear_agents_dir()`).
- [x] **Q21 (catatan non-blocker): `clear_agents_dir()` saat monitor start?** — **Keputusan final:** catat sebagai edge case — `clear_agents_dir()` (lib.rs:124) menghapus semua file status saat monitor start → beberapa poll pertama melewati parent-chain sampai plugin menulis ulang (heartbeat ≤ 10 dtk); tidak kontradiksi dengan "file = otoritatif" (tidak ada file fresh → file memang belum bisa jadi otoritatif). Lihat §3 catatan startup, EC16.
- [x] **Q22 (catatan non-blocker): `AgentInfo.launcher` butuh `#[serde(default)]`?** — **Keputusan final:** TIDAK sekarang — Rust tidak pernah mendeserialisasi `AgentInfo` dari JSON (hanya `Serialize` ke frontend); jika nanti ada deserialisasi, perlu `#[serde(default = ...)]` ber-default `"terminal"` agar kompatibel mundur. Beda dengan `StatusFile.launcher` yang dideserialisasi (wajib `#[serde(default)]`). Lihat §5, T3.

---

## 8. Acceptance Criteria

- [ ] **AC1:** Buka opencode di VSCode integrated terminal → `agent-update` membawa `launcher: "vscode"` → klik item agent membuka VSCode di `working_dir` (tidak ada terminal baru).
- [ ] **AC2:** Buka opencode di cmd/pwsh/Windows Terminal → `launcher: "terminal"` → klik item agent membuka terminal (`cmd /K cd /d <path>`).
- [ ] **AC3:** Tanpa plugin (tidak ada file status) → parent-chain: opencode di VSCode → klik buka VSCode; opencode di terminal → klik buka terminal.
- [ ] **AC4:** File status lama tanpa field `launcher` → fallback parent-chain, tidak crash, tidak selalu terminal.
- [ ] **AC5:** Dua agent dengan launcher berbeda dalam panel yang sama → klik masing-masing membuka konteks masing-masing.
- [ ] **AC6:** Set `click_action = "code"` sementara agent ber-launcher `"terminal"` → klik tetap membuka terminal (launcher menang; click_action hanya fallback).
- [ ] **AC7:** Indikator konteks (teks `VSCode`/`Terminal`) tampil di baris agent, nilai sesuai launcher.
- [ ] **AC8:** Tombol footer "Open Terminal" tetap membuka terminal secara eksplisit (tidak terpengaruh launcher).
- [ ] **AC9:** Panggil command `open_for_launcher` secara langsung dengan launcher tak dikenal (`"foo"` dan `""` dari `agent.launcher ?? ''`) → tidak crash, membuka sesuai `config.click_action` (arm `_`, fallback global) — verified via invoke langsung (T8), karena `scan_agents()` menormalkan launcher ke whitelist `{vscode, terminal}` sehingga nilai aneh tidak reachable lewat UI (resolusi Q19).
- [ ] **AC10:** Walk parent-chain terminasi (tidak infinite loop) — diverifikasi manual pada chain normal, chain dengan parent mati, dan chain yang menyentuh PID `System`.
- [ ] **AC11:** `cargo check` dan `cargo build` di `agent-monitor/src-tauri` sukses tanpa error dan **tanpa dependency baru**.
- [ ] **AC12:** Payload `agent-update` / `get_agents` menyertakan field `launcher` (ter-serialize oleh serde).
- [ ] **AC13 (Q1):** `scan_agents()` membaca file status **sekali** per agent via `read_state_and_launcher` (tidak ada lagi panggilan `read(pid)` terpisah) — verified via code review; hasil poll konsisten (state & launcher dari versi file yang sama) walau plugin menulis ulang file di tengah polling.
- [ ] **AC14 (Q2a):** Saat plugin tidak terpasang (EC16) dengan N agent, fallback parent-chain memakai **satu parent map per scan** (bukan N snapshot) — verified via code review + `cargo build`; hasil launcher untuk setiap agent benar untuk kedua skenario (VSCode vs terminal).
- [ ] **AC15 (Q6):** Chain parent > 5 hop (tidak ada `code.exe` dalam 5 hop pertama) → klik membuka terminal secara **silent** (tanpa error/modal), polling tidak terganggu.
- [ ] **AC16 (Q2b):** Reparenting / parent mati di tengah walk → hasil tetap `"vscode"` atau `"terminal"` yang aman, tidak ada hang/infinite loop; polling berikutnya mengoreksi (EC20).

---

## 9. Referensi

- [Plugin opencode saat ini — `.opencode/plugins/agent-status.ts`](file:///D:/dev/experiment-poni-agent/.opencode/plugins/agent-status.ts)
- [click_handler.rs — `open_terminal`, `open_vscode`, `open_path_with_action`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/click_handler.rs)
- [status_reader.rs — struct `StatusFile`, `read(pid)`, TTL 30 detik](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/status_reader.rs)
- [state.rs — `AgentInfo` & `Config`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/state.rs)
- [process_scanner.rs — `scan_agents()`, `PROCESSENTRY32W`, `th32ParentProcessID`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/process_scanner.rs)
- [lib.rs — `polling_loop`, `generate_handler!`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/lib.rs)
- [App.svelte — `openFolder`, baris agent, footer](file:///D:/dev/experiment-poni-agent/agent-monitor/src/App.svelte)
- [config.rs — `agents_dir()`](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/config.rs)
- [Plan referensi: `planning/agent-status-change.md`](agent-status-change.md) — asal plugin & file status
- [windows crate — Toolhelp32Snapshot / PROCESSENTRY32W](https://docs.rs/windows/latest/windows/Win32/System/Diagnostics/ToolHelp/index.html)
- [VSCode terminal env vars (`TERM_PROGRAM`)](https://code.visualstudio.com/docs/terminal/shell-integration)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `2026-08-01` | Planner | Initial draft |
| `1.1.0` | `2026-08-01` | Planner | Resolve 6 pertanyaan kritis reviewer: Q1 (satu read per agent — `read_state_and_launcher` menggantikan `read(pid)`; hapus opsi `read_launcher` dari T4; `prefer_status_file_source` hanya sentuh state), Q2 (parent map sekali per scan dari snapshot sama + edge case reparenting EC20), Q3 (launcher dihitung sekali sebelum `flush()` init; hapus frasa "evaluasi ulang saat menulis"), Q4 (semantik OUTER/INNER `None` `read_state_and_launcher` dipertahankan), Q5 (kalimat §1 "selalu terminal" direvisi), Q6 (MAX_DEPTH=5 cukup untuk chain VSCode nyata; chain > 5 → `"terminal"` silent; EC21/AC15). Update §1–§9, T1–T4, R2/R9, EC20–21, Q10–Q16, AC13–16. |
| `1.2.0` | `2026-08-01` | Planner | Resolve review putaran #2 — 4 pertanyaan kritis + 2 catatan non-blocker: Q17 (keputusan fallback "launcher tak dikenal": frontend `agent.launcher ?? ''` bukan `|| 'terminal'`; Rust arm `_` → `open_path_with_action(click_action)`; jalur hanya reachable via payload lama/undefined — §3, T5/T7, EC14, EC19, AC9), Q18 (terminologi hop: `MAX_DEPTH` → `MAX_HOPS = 5` = opencode + hingga 5 ancestor; seragamkan di §2/§3/tabel skenario, R2/R4/R9, EC10/EC21, T4, Q3/Q16, AC15), Q19 (T8 item 8 & AC9 tidak testable via UI → pindah ke test invoke langsung `open_for_launcher`), Q20 (referensi baris §1 `App.svelte:117` → `140`; audit §9: `generate_handler!` 148-156 → lib.rs:142-150), catatan (a) startup `clear_agents_dir()` (lib.rs:124) → catatan §3 + EC16 tanpa kontradiksi "file = otoritatif", catatan (b) `AgentInfo` tidak di-deserialize → tanpa `#[serde(default)]` sekarang, perlu jika nanti deserialisasi — §5/T3. Update §1–§9, T3–T8, R2/R4/R9, EC14/EC16/EC19/EC21, Q3/Q5/Q16–Q22, AC9/AC15. |
| `1.3.0` | `2026-08-01` | Planner | Finalisasi metadata setelah review putaran ke-3 disetujui (SEMPURNA): Status `Draft` → `Final`, Reviewer diisi, catatan toleransi reviewer tetap tercatat (referensi §1 dispatch `App.svelte:145` bukan 140; `AgentInfo` tanpa `#[serde(default)]` saat ini; label Q2a/Q2b internal; lokasi `VALID_STATUSES` di `read_fresh` bebas). |
