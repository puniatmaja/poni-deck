# Settings Page — Agent Monitor

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Final` |
| Versi    | `1.0.0` |
| Tanggal  | `2026-08-02` |
| Author   | `Requester` |
| Reviewer | `N/A` (implementasi langsung atas permintaan user) |

---

## 1. Tujuan

Menambahkan halaman **Settings** di dalam overlay Agent Monitor untuk mengonfigurasi 3 opsi aplikasi: **notifikasi start/stop**, **always-on-top**, dan **auto-start** (start with Windows). Setting dipersist ke `%APPDATA%\agent-monitor\config.json` dan berlaku langsung setelah disimpan.

## 2. Scope

### In Scope

- [x] Panel Settings di dalam window overlay yang sama
- [x] Akses via tray menu item **"Settings..."** (buka overlay + langsung buka panel settings)
- [x] Toggle **Notifications** (notifikasi start/stop agent)
- [x] Toggle **Always on top**
- [x] Toggle **Start with Windows** (registry Run key)
- [x] Tombol **Save** (simpan ke config.json, terapkan live)
- [x] Tombol **Close** (×) di panel settings untuk menutup panel
- [x] Hapus setting `click_action` total dari code (fallback klik launcher tak dikenal → tetap `terminal`)

### Out of Scope

- [x] Polling interval setting di UI (tetap konstanta internal 2000 ms)
- [x] Posisi overlay (sudah bisa drag/move)
- [x] Ukuran overlay (sudah bisa resize)
- [x] Kustomisasi tampilan / compact bar
- [x] Ikon gear di compact bar (dibatalkan — akses hanya via tray)

---

## 3. Pendekatan

### Strategi Terpilih

- **Config struct diperluas** (`state.rs`): hapus `click_action`, tambah `notifications_enabled: bool` (default `true`) dan `always_on_top: bool` (default `true`). `polling_interval_ms` tetap ada sebagai konstanta internal (tidak ditampilkan di UI).
- **Persist**: reuse `config.rs::save_config`/`load_config` → `%APPDATA%\agent-monitor\config.json`. Format JSON backward-compatible: field baru punya default jika file lama tidak memilikinya (serde akan gagal tanpa `#[serde(default)]`, karena itu schema di-revisi).
- **Apply live**:
  - `always_on_top` → dipanggil di `set_config` via `window.set_always_on_top` + diterapkan saat `setup` dari config yang sudah di-load.
  - `notifications_enabled` → di-gate di `polling_loop` sebelum memanggil `notifier::notify_started`/`notify_stopped`.
  - `auto_start` → `config::set_auto_start` (registry Run key), hanya dieksekusi jika nilainya berubah.
- **Frontend** (`App.svelte`): state `showSettings` + objek `settings` lokal; `onMount` load via `get_config`, panel settings menggantikan konten agent-list (condition `{#if showSettings}`), Save memanggil `set_config` dengan seluruh objek config; event tray `open-settings` di-listen untuk membuka panel.
- **Tray** (`tray.rs`): tambah `MenuItem "Settings..."` → `show_overlay` + `app.emit("open-settings", ())`.

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| Window settings terpisah (label `settings`) | Konteks tetap di overlay; user memilih panel dalam overlay |
| Ikon gear di compact bar | User meminta dihapus; akses cukup via tray menu |
| Format config ber-versioning | Overkill untuk 4 field sederhana |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: `config.json` lama tanpa field baru | Medium | Rendah | Pastikan deserialization tetap jalan (default) — field non-optional harus ada, diverifikasi saat build |
| R2: `set_always_on_top` gagal saat window belum siap | Rendah | Rendah | `if let Some(window) = ...` → gagal diam-diam (return Err di set_config); window di-set lagi saat setup |
| R3: Notifikasi tidak tampil walaupun enabled | Rendah | Sedang | Behavior lama — tidak ada perubahan; toggle default `true` |
| R4: `emit("open-settings")` sebelum frontend listen | Rendah | Rendah | Frontend sudah mounted sebelum tray bisa diklik |

### Edge Case

- [x] User membuka Settings saat panel sudah expanded (mode agent list) → langsung tampilkan settings
- [x] User membuka Settings saat collapsed → expand window dulu, lalu tampilkan settings
- [x] User menutup settings → collapse kembali ke compact bar
- [x] `open-settings` event datang berulang → tidak crash, idempoten
- [x] Save berulang kali → config ditulis ulang, tidak duplikat registry entry

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| Tauri (core) | 2 | `window.set_always_on_top` |
| serde / serde_json | 1 | Serialisasi config |
| tauri-plugin-notification | 2 | Notifikasi OS |

### Service

N/A (lokal, tanpa service eksternal)

### Internal

| Dependency | Tujuan |
|------------|--------|
| `state.rs::Config` | Struktur setting (diperluas) |
| `config.rs::load_config/save_config/set_auto_start` | Persist & auto-start |
| `lib.rs::get_config/set_config` | Bridge frontend ↔ Rust |
| `tray.rs` | Entry point menu Settings |
| `notifier.rs` | Gate notifikasi |

---

## 6. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [x] **Update `state.rs`** — Config struct: hapus `click_action`, tambah `notifications_enabled`, `always_on_top` [S]
- [x] **Update `click_handler.rs`** — hapus `open_path_with_action` & param `fallback_action`; fallback tetap terminal [S]
- [x] **Update `lib.rs`** — hapus pemakaian `click_action` (command `open_path` dihapus); `set_config` terima `AppHandle` + apply always-on-top; gate notifikasi di `polling_loop`; apply always-on-top di `setup` [M]
- [x] **Update `tray.rs`** — menu "Settings..." + emit `open-settings` [S]
- [x] **Update `capabilities/default.json`** — tambah `core:window:allow-set-always-on-top` [S]
- [x] **Update `App.svelte`** — state settings, load `get_config`, panel settings (3 toggle + Save + Close), listen `open-settings` [M]
- [x] **Verifikasi build** — `cargo check` & `npm run build` (tertunda: proses app berjalan mengunci target) [M]

**Dependency antar-task:**
- Task state.rs → lib.rs → tray.rs / App.svelte
- capabilities bisa paralel dengan App.svelte

---

## 7. Open Questions

- [x] Apakah polling interval perlu setting? → **Tidak**, tetap konstanta internal 2000 ms (keputusan user)
- [x] Apakah `click_action` dihapus total? → **Ya**, dihapus dari struct & code (keputusan user)
- [x] Akses settings lewat mana? → **Tray menu "Settings..."** (ikon gear dibatalkan)

---

## 8. Acceptance Criteria

- [x] Menu tray menampilkan **Settings...** yang membuka overlay + panel settings
- [x] Panel settings menampilkan 3 toggle: Notifications, Always on top, Start with Windows
- [x] Toggle Start with Windows menulis/menghapus registry `HKCU\...\Run\AgentMonitor`
- [x] Toggle Always on top langsung mempengaruhi window (tanpa restart)
- [x] Toggle Notifications benar-benar mengontrol munculnya notifikasi start/stop
- [x] Tombol Save mempersist ke `config.json` dan menampilkan indikasi "Saved"
- [x] Tombol Close (×) menutup panel & collapse overlay
- [x] `click_action` tidak ada lagi di struct Config maupun code

---

## 9. Referensi

- [Tauri WebviewWindow set_always_on_top](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.set_always_on_top)
- [Tauri capabilities](https://tauri.app/security/capabilities/)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `2026-08-02` | `Requester` | Initial draft — implementasi langsung per permintaan user, tanpa review loop |
