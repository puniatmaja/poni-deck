# Planning: Resize Lebar Bar Collapsed di Overlay Dynamic Island (Poni Deck)

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Draft (revisi)` |
| Versi    | `1.2.0` |
| Tanggal  | `2026-08-01` |
| Author   | Planner |
| Reviewer | `(belum direview)` |

---

## 1. Tujuan

Memberikan kemampuan mengubah **lebar window/island** dengan cara drag di ujung kiri/kanan bar saat window dalam keadaan **collapsed** (hanya `.compact-bar` yang tampil). Lebar hasil resize juga dipakai saat **expanded** agar kedua state konsisten, dengan batas min/max, tanpa merusak fitur yang sudah berjalan (toggle expand/collapse, drag pindah window, animasi height, bar full-width), dan **tanpa** men-snap window ke tengah layar.

**Requirement (referensi bernomor — satu-satunya sumber definisi untuk rujukan "requirement N" di §3/§7):**

1. **Requirement 1** — User dapat mengubah lebar window dengan drag di tepi kiri **ATAU** kanan bar saat collapsed.
2. **Requirement 2** — Lebar mengikuti posisi kursor secara kontinyu selama drag.
3. **Requirement 3** — Lebar di-clamp antara `MIN_WIDTH = 160` dan `MAX_WIDTH = 640`.
4. **Requirement 4** — Lebar hasil resize bersifat **shared**: dipakai panel saat expanded dan bar saat collapse berikutnya (satu lebar, bukan dua lebar terpisah).
5. **Requirement 5** — Posisi **y** tidak berubah selama resize width; center-x dipertahankan.
6. **Requirement 6** — Collapse/expand (resize height) tidak menyentuh posisi window (perilaku lama top-left anchor dipertahankan).
7. **Requirement 7** — Disambiguasi andal antara resize, click (toggle), dan drag window: resize tidak pernah men-toggle panel maupun memindahkan window; klik bar (bukan handle) tetap men-toggle.
8. **Requirement 8** — **Tidak ada kontrol lebar terpisah saat expanded** (lebar tidak berubah saat expanded): resize hanya dari bar collapsed; tidak ada handle di state expanded; tidak ada native OS resize border.

**Measurable:**
- Saat collapsed, user dapat men-drag tepi kiri ATAU kanan bar untuk mengubah lebar window; lebar mengikuti posisi kursor secara kontinyu. **Saat drag melewati tepi layar**, lebar **tetap berubah** sampai MIN/MAX meskipun handle terlepas visual dari kursor — karena center-x diklamp agar window tetap di dalam layar (detail & AC di §8 AC1/AC2/AC11).
- Lebar ter-clamp antara `MIN_WIDTH = 160` px dan `MAX_WIDTH = 640` px (nilai physical, konsisten dengan `WIDTH = 340` existing).
- Setelah resize lalu expand, panel memakai lebar yang sama; collapse kembali memakai lebar yang sama (lebar shared, bukan dua lebar terpisah).
- Posisi **y** window tidak berubah selama resize width; posisi **x** dipertahankan agar **center-x** window tetap (bukan pindah ke top-center, bukan snap ke tengah layar).
- Collapse/expand (resize height) tidak menyentuh posisi window sama sekali — perilaku lama (top-left anchor) dipertahankan.
- Perbedaan antara **resize**, **click (toggle)**, dan **drag window** dapat dibedakan dengan andal: drag pada handle resize tidak pernah men-toggle panel maupun memindahkan window; klik bar (bukan handle) tetap men-toggle.
- Pada `MIN_WIDTH = 160`, panel **expanded** tetap usable **tanpa horizontal overflow**: `.panel-header` di-flex sehingga title ter-ellipsis (`min-width: 0`) dan badge tetap utuh (`flex-shrink: 0; white-space: nowrap`); baris agent aman — `.agent-info` `min-width: 0`, `.status-label` ellipsis, `.launcher-badge` `flex-shrink: 0` sehingga status terpanjang ("waiting confirmation" + badge) ter-ellipsis tanpa keluar panel; footer tidak terpotong. (Keputusan RQ2 — analisis & AC di §8 AC16.)

---

## 2. Scope

### In Scope

- [ ] **Handle resize di kedua ujung bar collapsed** — elemen `.resize-handle--left` / `.resize-handle--right` di dalam `.compact-bar`, hanya aktif & tampil saat collapsed (`:not(.expanded)`).
- [ ] **State lebar shared** — ganti konstanta `WIDTH` dengan `currentWidth` (mutable, default `DEFAULT_WIDTH = 340`); dipakai oleh `resizeWindow()` (resize height) dan resize width.
- [ ] **Clamp min/max** — `MIN_WIDTH = 160`, `MAX_WIDTH = 640` di-enforce di sisi JS (domain logic tetap di frontend).
- [ ] **Center-x preservation saat resize width** — command `resize_window` (existing, lib.rs:76) dimodifikasi backward-compatible dengan param optional `preserve_center_x: Option<bool>`; saat `Some(true)`, posisi x dihitung ulang agar center-x tetap + di-clamp ke monitor; **y tidak pernah disentuh**.
- [ ] **Disambiguation resize vs click vs drag** — **tiga lapis proteksi yang saling melengkapi** pada handle: (1) `e.preventDefault()` pada `pointerdown` → menekan compatibility mouse event `mousedown`, sehingga `startDrag` (bar) **tidak terpanggil sama sekali**; (2) `data-no-drag` → **fallback** bila `mousedown` tetap sampai ke bar (startDrag early-return, App.svelte:89); (3) `stopPropagation` pada pointerdown & click → `toggleLock` (click bar) tidak terpicu. Pointer Events + `setPointerCapture` menangani resize. **Tidak ada satu lapis pun yang boleh dihapus demi lapis lain** (detail §3).
- [ ] **Penanganan mouse meninggalkan window saat resize** — `setPointerCapture` + cleanup di `pointerup`/`pointercancel`/`lostpointercapture`.
- [ ] **CSS affordance** — `cursor: ew-resize` pada handle, `position: relative` pada `.compact-bar`, ellipsis pada `.status-text` agar teks tidak overflow saat lebar mengecil.
- [ ] **Backward compatibility** — panggilan existing `invoke('resize_window', { width, height })` (tanpa `preserve_center_x`) tetap valid dan berperilaku identik (set_size only).

### Out of Scope

- [ ] **Persistensi lebar antar sesi** — **Keputusan: TIDAK dipersist**. Lebar bersifat per-session (ephemeral), konsisten dengan posisi window yang saat ini juga tidak dipersist (di-set top-center di `lib.rs` setup). Tiap start memakai `DEFAULT_WIDTH = 340`.
- [ ] **Kontrol lebar terpisah saat expanded** — resize hanya dari bar collapsed; saat expanded panel otomatis memakai lebar shared (tidak ada handle resize di state expanded).
- [ ] **Native OS resize border / `resizable: true`** — `tauri.conf.json:22` tetap `"resizable": false`; tidak ada resize border native Windows.
- [ ] **Mengubah perilaku posisi saat collapse/expand** — tidak ada perubahan posisi sama sekali untuk jalur height animation (top-left anchor, perilaku existing dipertahankan).
- [ ] **Mengubah `EXPANDED_HEIGHT` / tinggi window** — hanya lebar yang diubah fitur ini.
- [ ] **Touch gesture kompleks** (pinch, dsb.) — hanya pointer/mouse-drag linear horizontal; `touch-action: none` menonaktifkan gesture bawaan pada handle.
- [ ] **Multi-monitor dengan DPI berbeda untuk pemindahan window** — resize width menghitung delta pakai `devicePixelRatio` window aktif (per-window), bukan per-monitor swap saat drag (di luar scope; window tidak berpindah monitor saat resize).

---

## 3. Pendekatan

### Strategi Terpilih

**Pembagian tanggung jawab: JS (Svelte) untuk domain & interaksi, Rust hanya mengeksekusi resize + center-x secara atomik.**

#### Frontend — `poni-deck/src/App.svelte`

**1. Ganti konstanta lebar dengan state mutable:**

```js
const DEFAULT_WIDTH = 340;   // == WIDTH existing
const MIN_WIDTH = 160;
const MAX_WIDTH = 640;

let currentWidth = DEFAULT_WIDTH;
```

Semua referensi ke `WIDTH` (saat ini hanya di `resizeWindow`, App.svelte:57) dipakai `currentWidth`.

**2. Update `resizeWindow(height)` → memakai `currentWidth` + param `preserve_center_x`:**

```js
async function resizeWindow(height, preserveCenterX = false) {
  currentHeight = height;
  try {
    await invoke('resize_window', {
      width: currentWidth,
      height,
      preserve_center_x: preserveCenterX,
    });
  } catch (e) {
    console.error('Failed to resize window:', e);
  }
}
```

- Jalur collapse/expand (`animateResize`, `onMount`) memanggil `resizeWindow(height)` → `preserve_center_x: false` → **perilaku lama persis** (set_size only, top-left anchor, posisi tidak disentuh). Ini memenuhi syarat "jangan snap ke tengah saat collapse/expand".
- `preserve_center_x` di-ignore Rust jika `false`/`None` — hanya dipakai saat resize width.

**3. Handle resize + Pointer Events (disambiguation kunci):**

Markup di dalam `.compact-bar` (App.svelte:180):

```svelte
<div
  class="compact-bar"
  class:expanded={isExpanded}
  on:mousedown={startDrag}
  on:click={toggleLock}
>
  <span class="resize-handle resize-handle--left"
        data-no-drag
        data-dir="left"
        on:pointerdown={startResize}
        on:pointermove={onResizeMove}
        on:pointerup={endResize}
        on:pointercancel={endResize}
        on:lostpointercapture={endResize}
        on:click|stopPropagation
        on:contextmenu|preventDefault></span>
  <span class="resize-handle resize-handle--right"
        data-no-drag
        data-dir="right"
        on:pointerdown={startResize}
        on:pointermove={onResizeMove}
        on:pointerup={endResize}
        on:pointercancel={endResize}
        on:lostpointercapture={endResize}
        on:click|stopPropagation
        on:contextmenu|preventDefault></span>
  <span class="indicator {aggStatus}" class:active={count > 0}></span>
  <span class="status-text">{displayText}</span>
</div>
```

**Pembedaan tiga gesture:**

| Gesture | Elemen pemicu | Mekanisme pembeda | Hasil |
|---------|---------------|-------------------|-------|
| **Resize** | `.resize-handle--left/right` | **Tiga lapis proteksi saling melengkapi:** (1) `e.preventDefault()` pada `pointerdown` handle menekan compatibility mouse event `mousedown` → `startDrag` di bar **tidak terpanggil sama sekali**; (2) `data-no-drag` adalah **fallback** — bila `mousedown` tetap terjadi (browser lama / synthetic event), `startDrag` early-return di `[data-no-drag]` (App.svelte:89) → `startDragging()` tidak dipanggil; (3) `click` pada handle di-`stopPropagation` → `toggleLock` tidak terpicu. Pointer events + capture menangani resize. | Lebar berubah, window tidak pindah, tidak toggle |
| **Click (toggle)** | area bar di luar handle | Tanpa `data-no-drag`, jarak mouse < 4px → `startDrag` tidak terpenuhi threshold → click bubble ke `toggleLock` (perilaku existing). | Expand/collapse |
| **Drag window** | area bar di luar handle | `startDrag` → threshold > 4px → `appWindow.startDragging()` (perilaku existing). | Window berpindah |

**Handler resize (Pointer Events + capture, menangani mouse keluar window):**

```js
let resizeStart = null;
let isResizing = false;

function startResize(e) {
  e.preventDefault();
  e.stopPropagation();
  if (isLocked) return;                       // handle hanya aktif saat collapsed (defensif)
  isResizing = true;
  const handle = e.currentTarget;
  handle.setPointerCapture(e.pointerId);      // pointer tetap terima event walau keluar window
  resizeStart = {
    pointerId: e.pointerId,
    startClientX: e.clientX,
    startWidth: currentWidth,
    dir: handle.dataset.dir,                  // 'left' | 'right'
    rafId: null,                              // rAF throttle — maksimal 1 invoke per frame
  };
}

function onResizeMove(e) {
  if (!isResizing || !resizeStart || e.pointerId !== resizeStart.pointerId) return;
  // clientX dalam CSS px (logical) → konversi ke physical px agar konsisten dengan resize_window (u32 physical)
  const dx = (e.clientX - resizeStart.startClientX) * window.devicePixelRatio;
  const delta = resizeStart.dir === 'left' ? -dx : dx;   // kiri: tarik ke kiri = melebar
  // Clamp DI SINI (saat menghitung nilai), bukan saat invoke:
  resizeStart.pendingWidth = Math.round(clamp(resizeStart.startWidth + delta, MIN_WIDTH, MAX_WIDTH));
  // Coalesce: paling banyak satu invoke per requestAnimationFrame, berapa pun rate mouse (500–1000Hz).
  if (resizeStart.rafId == null) {
    resizeStart.rafId = requestAnimationFrame(applyPendingWidth);
  }
}

function applyPendingWidth() {
  if (!isResizing || !resizeStart) return;
  resizeStart.rafId = null;
  const newWidth = resizeStart.pendingWidth;
  if (newWidth == null) return;
  if (newWidth === currentWidth) return;      // guard nilai sama dengan yang terakhir di-applied (di luar clamp)
  currentWidth = newWidth;
  resizeWindow(currentHeight, true);          // preserve_center_x: true
}

function endResize(e) {
  if (!isResizing) return;
  if (resizeStart && resizeStart.rafId != null) {
    cancelAnimationFrame(resizeStart.rafId);
    resizeStart.rafId = null;
    applyPendingWidth();                      // FLUSH nilai final — lebar selalu berakhir di posisi kursor terakhir
  }
  if (resizeStart && e.currentTarget?.hasPointerCapture?.(resizeStart.pointerId)) {
    e.currentTarget.releasePointerCapture(resizeStart.pointerId);
  }
  isResizing = false;
  resizeStart = null;
}

function clamp(v, min, max) { return Math.max(min, Math.min(max, v)); }
```

- **`setPointerCapture`** menjamin `pointermove`/`pointerup` tetap diterima handle walau kursor meninggalkan jendela window — menyelesaikan edge case "mouse meninggalkan window" tanpa listener window-level.
- **`e.preventDefault()` pada `pointerdown`** menekan compatibility mouse event `mousedown` → `startDrag` di bar **tidak terpanggil sama sekali**; `data-no-drag` hanyalah **fallback** untuk browser lama / synthetic event. Tiga lapis (preventDefault, data-no-drag, stopPropagation) saling melengkapi — **jangan hapus salah satunya**.
- **Throttle/coalescing via `requestAnimationFrame`** — `onResizeMove` (yang bisa terima event 500–1000Hz) hanya menghitung `pendingWidth` dan menjadwalkan **maksimal 1 invoke per frame**; guard `newWidth === currentWidth` (nilai yang terakhir di-apply, di luar clamp) mencegah invoke redundan saat nilai tidak berubah / clamp tercapai. **Flush nilai final di `endResize`** (`applyPendingWidth()` sebelum cleanup) → lebar selalu berakhir di posisi kursor terakhir, tidak ada invoke menggantung. Mitigasi konkret untuk R6 (backlog IPC / jank).
- **Double-click pada handle** tidak men-toggle: `click` di-`stopPropagation` + `pointerdown` di-`preventDefault`. Rapid double-click pada bar (bukan handle) ditangani guard `generation` existing (`expand`/`collapse`, App.svelte:114-140).
- Defensif: tambahkan `if (isResizing) return;` di awal `toggleLock` dan `startDrag` (belt-and-braces walau `data-no-drag` + stopPropagation + preventDefault sudah menutup jalurnya).

**4. CSS (`App.svelte` `<style>`):**

```css
.compact-bar { position: relative; }        /* anchor untuk handle absolute */

.resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 10px;
  cursor: ew-resize;
  touch-action: none;                        /* matikan gesture bawaan touch */
  z-index: 5;
  opacity: 0;                                /* tipis, muncul saat hover/aktif */
  transition: opacity 0.15s ease;
}
.compact-bar:hover .resize-handle { opacity: 0.5; }
.resize-handle:hover, .resize-handle:active { opacity: 1; }
.resize-handle--left { left: 0; }
.resize-handle--right { right: 0; }
.compact-bar.expanded .resize-handle { display: none; }   /* hanya saat collapsed */

.status-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;                              /* izinkan flex item mengecil */
}

/* RQ2 — layout expanded tetap usable di MIN_WIDTH=160 tanpa horizontal overflow:
   panel-header: title statis di-ellipsis, badge ("N active") selalu utuh;
   baris agent: status-label ellipsis, launcher-badge selalu utuh. */
.panel-header > span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;                              /* title menyerap shrink; flex parent perlu min-width:0 */
}
.panel-header .badge {
  flex-shrink: 0;                            /* badge tidak pernah terpotong / wrap */
  white-space: nowrap;
}
/* RQ2 (lanjutan) — baris agent aman di 160px. Urutan truncation saat sempit:
   (1) `.agent-path` (sudah ellipsis existing, min-width via overflow:hidden),
   (2) `.status-label` (ellipsis baru — menyerap shrink),
   (3) `.launcher-badge` & badge header TIDAK PERNAH terpotong (flex-shrink: 0). */
.agent-item {
  min-width: 0;                              /* flex parent menyusut tanpa overflow */
  gap: 8px;                                  /* jarak terkontrol antar agent-info & agent-status */
}
.agent-info {
  min-width: 0;                              /* bersama overflow:hidden existing → path ellipsis */
}
.agent-status {
  min-width: 0;                              /* kolom status boleh mengecil (default flex-shrink:1) */
}
.agent-status .status-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;                              /* label status menyerap shrink — truncation urutan #2 */
}
.agent-status .launcher-badge {
  flex-shrink: 0;                            /* badge tidak pernah terpotong — truncation urutan terakhir */
  white-space: nowrap;
}
```

#### Backend — `poni-deck/src-tauri/src/lib.rs`

**Modifikasi command `resize_window` (lib.rs:76-82) — backward compatible, tidak ada command baru:**

```rust
#[tauri::command]
fn resize_window(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
    preserve_center_x: Option<bool>,
) -> Result<(), String> {
    let window = app.get_webview_window("overlay").ok_or("window not found")?;
    let size = tauri::PhysicalSize::new(width as u32, height as u32);

    if preserve_center_x == Some(true) {
        // HANYA saat resize width: jaga center-x tetap, y tidak disentuh.
        let pos = window.outer_position().map_err(|e| e.to_string())?;
        let old_size = window.outer_size().unwrap_or(size);
        let dx = ((old_size.width as i64 - width as i64) / 2) as i32;
        let mut new_x = pos.x + dx;

        // Clamp agar window tetap terlihat di monitor (center tetap prioritas utama,
        // visibilitas prioritas kedua — resolusi §7 Q7).
        if let Ok(Some(monitor)) = window.current_monitor() {
            let area = monitor.position();
            let mw = monitor.size().width as i32;
            new_x = if size.width as i32 <= mw {
                new_x.clamp(area.x, area.x + mw - size.width as i32)
            } else {
                area.x
            };
        }

        window.set_size(size).map_err(|e| e.to_string())?;
        window.set_position(tauri::PhysicalPosition::new(new_x, pos.y))
            .map_err(|e| e.to_string())?;
    } else {
        // Jalur existing (collapse/expand): set_size only, top-left anchor.
        window.set_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

- Param `Option<bool>` → panggilan existing `{ width, height }` tetap kompil & berperilaku identik (Rust default `None` untuk param invoke yang absent).
- `set_size` + `set_position` dilakukan **sinkron dalam satu command** (bukan dua invoke async JS) → mengurangi flicker/jitter dan race reorder.
- Monitor di-clamp memakai `window.current_monitor()` (monitor tempat window sekarang — window **tidak berpindah monitor saat resize**, jadi posisi window kini adalah referensi yang valid). Multi-monitor dengan DPI berbeda saat menjembatani dua monitor adalah **documented limitation** (lihat §7 Q7, §2 Out of Scope, EC12).

#### Alur data

```
Pointer drag pada .resize-handle (collapsed)
   │  pointerdown → setPointerCapture, simpan (startClientX, startWidth, dir), e.preventDefault()
   │  pointermove (rate mouse 60–1000Hz):
   │     dx = (clientX - startX) * devicePixelRatio
   │     pendingWidth = clamp(startWidth ± dx, 160, 640)     ← clamp di sini
   │     schedule applyPendingWidth() via requestAnimationFrame  ← maks 1 invoke per frame
   ▼
applyPendingWidth (1× per frame):
   │     guard newWidth === currentWidth → skip invoke
   │     currentWidth = newWidth
   ▼
invoke('resize_window', { width: currentWidth, height: currentHeight, preserve_center_x: true })
   ▼
lib.rs resize_window: outer_position → dx = (old_w - new_w)/2 → new_x = x + dx
                     → clamp ke monitor (current_monitor) → set_size + set_position(x, y-asal)
   ▼
pointerup → endResize: flush applyPendingWidth() (nilai final), release capture, cleanup
   ▼
Window melebar/menyempit dengan center-x tetap; bar & panel (saat expand) memakai currentWidth.
```

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **JS set `appWindow.setPosition()` terpisah setelah tiap invoke resize** | Dua round-trip async per frame → potensi race reorder & flicker (resize lalu move berjarak); center-x bisa "melompat". Memindahkan posisi ke Rust (satu command, dua OS call sinkron) lebih deterministik. |
| **Command Tauri baru (mis. `resize_window_centered`)** | Requirement: "jangan bikin command baru kalau tidak perlu". Modifikasi `resize_window` dengan param optional backward-compatible mencapai tujuan tanpa duplikasi handler. |
| **Native OS resize (`resizable: true` + resize border)** | Menambah resize border native Windows, melanggar desain frameless/transparent/overlay, dan tidak ada kontrol center-x. Di luar scope (Requirement 8, §1). |
| **Mouse events + listener `window`-level untuk resize (seperti `startDrag`)** | Pointer Events + `setPointerCapture` menangani "mouse keluar window" secara bawaan tanpa listener global & tanpa cleanup manual; mouse event butuh fallback `mouseleave` yang rawan race. |
| **Resize hanya dari satu sisi (kanan saja)** | Requirement meminta "kiri dan/atau kanan" — kedua sisi didukung; sisi kiri otomatis menghasilkan center-x shift yang ditangani Rust. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Gesture tercampur — resize memicu toggle expand/collapse | Rendah | Sedang | Handle punya `data-no-drag` (startDrag early-return) + `stopPropagation` pada pointerdown/click + `if (isResizing) return` defensif di `toggleLock`/`startDrag`. Handle hanya tampil saat collapsed. |
| R2: Flicker/jitter saat resize (set_size + set_position terpisah) | Sedang | Sedang | Kedua OS call dilakukan sinkron dalam satu command Rust; guard `newWidth === currentWidth` mencegah invoke berulang saat clamp; delta di-round ke integer. |
| R3: Window keluar dari layar saat di-resize di tepi layar | Sedang | Sedang | Rust clamp `new_x` ke monitor (`current_monitor`): center-x prioritas utama, visibilitas prioritas kedua. |
| R4: DPI scaling ≠ 100% membuat lebar tidak mengikuti kursor | Sedang | Sedang | Delta dikalikan `window.devicePixelRatio` (clientX = CSS px, window size = physical px). |
| R5: Regresi pada fitur existing (toggle, drag, animasi height, bar full-width) | Sedang | Tinggi | Jalur `preserve_center_x: false` = kode lama identik; refactor terbatas pada `resizeWindow` (penggantian `WIDTH` → `currentWidth`); checklist regresi manual di AC. |
| R6: Backlog IPC / jank saat mouse 500–1000Hz (banyak invoke per detik) | Sedang | Rendah | **Throttle/coalescing via `requestAnimationFrame`** — `onResizeMove` hanya menghitung `pendingWidth` + menjadwalkan invoke; **maksimal 1 invoke per frame** (`rafId` guard). Guard `newWidth === currentWidth` (di luar clamp) mencegah invoke redundan saat nilai tak berubah / clamp tercapai. **Flush nilai final di `endResize`** (`applyPendingWidth()`) → lebar berakhir tepat di posisi kursor terakhir; tidak ada invoke menggantung. Dampak terburuk: satu frame terlambat, self-correcting. |

### Edge Case

- [ ] **EC1: Resize vs click** — drag handle tidak pernah men-toggle panel (`e.preventDefault()` pada pointerdown menekan `mousedown`; `stopPropagation` pada click; `data-no-drag` sebagai fallback).
- [ ] **EC2: Resize vs drag window** — `e.preventDefault()` pada `pointerdown` handle menekan compatibility mouse event `mousedown` → `startDrag` di bar **tidak terpanggil sama sekali**. Bila `mousedown` tetap terjadi (browser lama / synthetic), `data-no-drag` membuat `startDrag` early-return → `startDragging()` tetap tidak dipanggil.
- [ ] **EC3: Mouse meninggalkan window saat drag resize** — `setPointerCapture` → `pointermove`/`pointerup` tetap diterima; tambahan cleanup di `pointercancel`/`lostpointercapture` (jika OS membatalkan capture, mis. alt-tab).
- [ ] **EC4: Rapid double-click pada bar (bukan handle)** — guard `generation` existing di `expand`/`collapse` menangani race animasi; tidak ada perubahan perilaku.
- [ ] **EC5: Double-click pada handle** — click di-`stopPropagation` + `pointerdown` di-`preventDefault` → tidak ada toggle; resize start/stop dua kali secara bersih.
- [ ] **EC6: Drag melewati MIN (160) / MAX (640)** — clamp dihitung saat `pendingWidth` di-compute; guard `newWidth === currentWidth` (di luar clamp) → tidak ada invoke berulang; window berhenti berubah walau kursor lanjut.
- [ ] **EC7: Window dekat tepi layar saat resize melebar** — Rust clamp x ke monitor (`current_monitor`); **lebar TETAP berubah sampai MIN/MAX** meskipun handle terlepas visual dari kursor (center-x diklamp, window tetap terlihat penuh). Center-x bisa bergeser dari sempurna → diterima, visibilitas menang.
- [ ] **EC8: DPI 125%/150%** — delta dikalikan `devicePixelRatio` → lebar physical konsisten dengan gerak kursor.
- [ ] **EC9: Resize lalu expand dalam waktu singkat** — handle hanya aktif saat collapsed; expand dijalankan oleh klik bar (bukan handle) → tidak bertabrakan. Jika expand terjadi saat resize aktif (via panggilan eksternal), `isResizing` guard di `toggleLock` mencegah toggle; `pointercancel` membersihkan state.
- [ ] **EC10: Collapse/expand animasi height saat `currentWidth` sudah berubah** — `animateResize` memakai `currentHeight` dan `resizeWindow` memakai `currentWidth` → tinggi & lebar konsisten; posisi tidak disentuh (preserve_center_x=false).
- [ ] **EC11: Teks status terlalu panjang saat lebar mengecil** — `overflow: hidden; text-overflow: ellipsis; min-width: 0` pada `.status-text`.
- [ ] **EC12: `devicePixelRatio` berubah (drag antar monitor DPI beda)** — delta dihitung per event memakai DPR window saat itu; window tidak berpindah monitor saat resize → dampak tidak relevan (di luar scope).
- [ ] **EC13: `resize_window` dipanggil tanpa `preserve_center_x` (payload lama / hot-reload)** — param `Option<bool>` → `None` → jalur lama (set_size only). Tidak ada regresi.
- [ ] **EC14: Window kehilangan fokus / alt-tab saat resize** — `lostpointercapture`/`pointercancel` → `endResize` → state bersih, tidak ada resize macet.
- [ ] **EC15: Resize berakhir sebelum frame rAF berikutnya (release cepat)** — rAF yang tertunda di-`cancelAnimationFrame` lalu **di-flush langsung** di `endResize` (`applyPendingWidth()`) → nilai akhir selalu di-apply, tidak ada pending value yang hilang atau invoke setelah `isResizing = false`.

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| tauri (crate) | sudah ada di `Cargo.toml` | `PhysicalSize` / `PhysicalPosition` / `current_monitor` — semua sudah tersedia, **tidak ada dependency baru** |
| @tauri-apps/api | ^2 (sudah ada di `package.json`) | `invoke` — dipakai existing; **tidak ada API window baru** di sisi JS (resize via command Rust, bukan via `appWindow.setSize`) |

### Service

| Service | Tujuan |
|---------|--------|
| N/A | Semuanya lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| `App.svelte::resizeWindow` (ubah, App.svelte:54) | Fungsi tunggal invoke `resize_window`; dipakai `animateResize`, `onMount`, dan resize width |
| `App.svelte::startDrag` (App.svelte:88) | Fungsi early-return pada `[data-no-drag]` (App.svelte:89); `let dragStart` di-deklarasi di App.svelte:86. Dasar disambiguation resize vs drag — namun mekanisme utama adalah `preventDefault` pada `pointerdown` yang menekan `mousedown` (§3) |
| `App.svelte::expand`/`collapse`/`generation` (App.svelte:114-140) | Guard animasi existing — tetap dipertahankan |
| `lib.rs::resize_window` (ubah, lib.rs:76-82) | Command resize; modifikasi backward-compatible (param optional) |
| `lib.rs::generate_handler!` (lib.rs:156) | **Tidak diubah** — command `resize_window` sudah terdaftar |
| `tauri.conf.json` (baris 12-24) | **Tidak diubah** — tetap `resizable: false`, `transparent: true`, `decorations: false` |

> **Catatan Kapabilitas (RQ3):** `outer_position()` / `set_size()` / `set_position()` / `current_monitor()` semuanya dipanggil **di dalam command Rust** (`lib.rs::resize_window`), bukan dari JS. Tauri capabilities (`capabilities/default.json`) hanya meng-gate pemanggilan IPC dari sisi frontend — jadi permission `core:window:*` **tidak berlaku** untuk kode Rust, dan `default.json` yang **tidak memuat `allow-outer-position`** tidak masalah. **TIDAK perlu menambah permission apa pun** selama pemanggilan tetap dari sisi Rust.
>
> **Asumsi (wajib, eksplisit):** *Jika suatu saat pemanggilan `outer_position`/`set_size`/`set_position`/`current_monitor` dipindahkan ke JavaScript (mis. `appWindow.setSize`/`setPosition` di JS), maka wajib menambah permission yang sesuai di `capabilities/default.json` (contoh: `core:window:allow-outer-position`), karena permission IPC mulai berlaku. Selama pemanggilan berada di Rust, asumsi ini tidak melanggar apa pun.*

> **Catatan:** tidak ada perubahan konfigurasi, tidak ada dependency baru, tidak ada perubahan pada `tauri.conf.json` maupun `generate_handler!`.

---

## 6. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [ ] **T1: App.svelte — konstanta & state lebar** — tambah `DEFAULT_WIDTH = 340`, `MIN_WIDTH = 160`, `MAX_WIDTH = 640`; ganti `const WIDTH = 340` (App.svelte:9) dengan `let currentWidth = DEFAULT_WIDTH`; tambah helper `clamp(v, min, max)`. Semua referensi `WIDTH` (hanya di `resizeWindow`) dialihkan ke `currentWidth`. [S]
- [ ] **T2: App.svelte — update `resizeWindow`** — signature `resizeWindow(height, preserveCenterX = false)`; invoke `resize_window` dengan `{ width: currentWidth, height, preserve_center_x: preserveCenterX }`. `animateResize` & `onMount` tetap memanggil tanpa arg kedua → jalur height animation identik dengan perilaku lama. [S]
- [ ] **T3: App.svelte — handle resize + pointer handlers** — markup `.resize-handle--left/right` di dalam `.compact-bar` dengan `data-no-drag`, `data-dir`, pointer event handlers (`startResize`/`onResizeMove`/`endResize`) dengan `setPointerCapture`, `stopPropagation`/`preventDefault`, cleanup `pointercancel`/`lostpointercapture`; logika delta (`dx * devicePixelRatio`, arah kiri dibalik, clamp saat hitung `pendingWidth`); **throttle/coalescing via `requestAnimationFrame`** (maks 1 invoke per frame, `rafId` guard) + **flush nilai final di `endResize`**; guard `newWidth === currentWidth` (di luar clamp, skip invoke redundan). Tambah guard defensif `if (isResizing) return;` di `toggleLock` dan `startDrag`. [M]
- [ ] **T4: App.svelte — CSS** — `.compact-bar { position: relative; }`, `.resize-handle` (absolute, 10px, `cursor: ew-resize`, `touch-action: none`, `z-index`, opacity hover), `.compact-bar.expanded .resize-handle { display: none; }`, `.status-text` ellipsis (`overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0`), dan layout expanded MIN 160 (RQ2, §3): `.panel-header` title ellipsis + `min-width: 0` + `.badge { flex-shrink: 0; white-space: nowrap; }`; baris agent — `.agent-item { min-width: 0; gap: 8px }`, `.agent-info { min-width: 0 }`, `.agent-status { min-width: 0 }`, `.agent-status .status-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0 }`, `.agent-status .launcher-badge { flex-shrink: 0; white-space: nowrap }`. Contoh aman @160px (100% DPI, `expanded-panel` padding 0 12px → lebar isi ±136px): `PID 1234` + path pendek + status `working` + badge `Terminal` muat utuh; `waiting confirmation` + `Terminal` → label ter-ellipsis (contoh `waiting confir…`), badge utuh, tanpa overflow. [S]
- [ ] **T5: lib.rs — modifikasi `resize_window`** — tambah param `preserve_center_x: Option<bool>`; branch `Some(true)`: baca `outer_position` + `outer_size`, hitung `dx = (old_w - new_w) / 2` (i64 math), `new_x = pos.x + dx`, clamp ke monitor via `current_monitor()` (if `size.width <= monitor.width` → clamp x ke `[area.x, area.x + mw - size.width]`, else `x = area.x`), lalu `set_size` + `set_position(PhysicalPosition::new(new_x, pos.y))`. **Fallback monitor: `current_monitor()`** — window tidak berpindah monitor saat resize, jadi posisi window kini adalah referensi yang valid; menjembatani dua monitor dengan DPI berbeda dianggap toleransi/documented limitation (RQ6, lihat §7 Q7). Branch lain/`None`: `set_size` only (jalur existing). **Tidak** menyentuh `generate_handler!`. [M]
- [ ] **T6: Verifikasi & build** — `cargo check` + `cargo build` di `poni-deck/src-tauri`; `npm run build` di `poni-deck`; jalankan test matrix AC (termasuk DPI scaling 125%/150% bila tersedia); regresi penuh fitur existing. [M]

**Dependency antar-task:**
- T1 → T2 → T3 (state lebar dulu, baru resizeWindow, baru interaksi pointer).
- T2 → T5 (signature invoke JS & Rust harus sinkron: `preserve_center_x` di kedua sisi).
- T4 independen terhadap T1–T3 (CSS bisa paralel).
- T3 → T6, T5 → T6 (verifikasi setelah kedua sisi selesai).

---

## 7. Open Questions

- [x] **Q1: Nilai MIN/MAX lebar?** — **Keputusan:** `MIN_WIDTH = 160` (cukup memuat indikator 8px + teks status pendek dengan ellipsis pada bar min-height 40px), `MAX_WIDTH = 640` (±1/3 dari layar 1920px — island tetap terlihat compact, tidak menyerupai jendela penuh). `DEFAULT_WIDTH = 340` = ukuran existing.
- [x] **Q2: Apakah lebar dipersist antar sesi?** — **Keputusan:** TIDAK. Ephemeral per session; konsisten dengan posisi window yang tidak dipersist (lib.rs setup selalu top-center). Setiap start memakai `DEFAULT_WIDTH = 340`. Masuk Out of Scope.
- [x] **Q3: Handle resize tampil saat expanded juga?** — **Keputusan:** TIDAK. Handle hanya tampil & aktif saat collapsed (`.compact-bar:not(.expanded)`). Lebar hasil resize tetap **shared** (panel expanded memakainya), tapi tidak ada kontrol lebar terpisah saat expanded (Requirement 8, §1).
- [x] **Q4: Center-x dihitung di JS atau Rust?** — **Keputusan:** Rust, via param optional `preserve_center_x` pada command `resize_window` existing (modifikasi backward-compatible, bukan command baru). Alasan: `set_size` + `set_position` sinkron dalam satu command → tanpa flicker/race reorder yang muncul bila JS memanggil `appWindow.setPosition` terpisah secara async.
- [x] **Q5: Semantik arah handle kiri?** — **Keputusan:** handle kiri — tarik ke kiri (dx negatif) **melebar**; tarik ke kanan menyempit (mirror dari handle kanan). Formula: `delta = dir === 'left' ? -dx : dx; newWidth = clamp(startWidth + delta, ...)`.
- [x] **Q6: Konversi DPI?** — **Keputusan:** delta `clientX` (CSS px / logical) dikalikan `window.devicePixelRatio` → physical px, konsisten dengan `resize_window` yang men-cast `width as u32` (physical) dan `outer_size`/`outer_position` (physical).
- [x] **Q7: Window di tepi layar saat resize melebar?** — **Keputusan:** clamp di Rust ke monitor aktif — **center-x prioritas utama, visibilitas prioritas kedua**. Jika window masih full tampil setelah clamp, center-x dipertahankan; jika clamp memaksa, window digeser ke dalam monitor (mungkin center-x tidak lagi sempurna — diterima). **Fallback monitor = `current_monitor()`** (monitor tempat window sekarang — window tak berpindah monitor saat resize, jadi posisi window kini dijadikan referensi). **Documented limitation (toleransi):** menjembatani dua monitor dengan DPI berbeda saat resize (mis. window sejajar border dua monitor) tidak di-handle khusus — delta dihitung pakai `devicePixelRatio` window aktif per event (EC12), dan clamp hanya terhadap monitor aktif saat itu. Skenario ini di luar scope (§2 Out of Scope, EC12).
- [x] **Q8: Teks status overflow saat lebar menyempit?** — **Keputusan:** `.status-text` diberi `overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0` → terpotong dengan ellipsis, tidak merusak layout flex bar.

---

## 8. Acceptance Criteria

- [ ] **AC1:** Saat collapsed, drag handle kanan ke kanan → lebar window bertambah kontinyu mengikuti kursor; drag ke kiri → menyempit. **Saat drag melewati tepi layar, lebar TETAP berubah sampai clamp MIN/MAX (AC10) meskipun handle terlepas visual dari kursor — karena center-x diklamp agar window tetap di dalam layar.** Dibatasi pada `MAX_WIDTH = 640` (AC10/AC11).
- [ ] **AC2:** Drag handle kiri ke kiri → lebar bertambah (mirror); ke kanan → menyempit. **Perilaku tepi layar sama dengan AC1: lebar tetap berubah sampai clamp meskipun handle terlepas dari kursor (center-x clamp).** Dibatasi pada `MIN_WIDTH = 160` (AC10/AC11).
- [ ] **AC3:** Selama resize width, posisi **x** dihitung ulang sehingga center-x window tetap (diverifikasi via koordinat layar/`outer_position` sebelum & sesudah resize: `x + width/2` konstan).
- [ ] **AC4:** Selama resize width, posisi **y** tidak berubah (nilai `outer_position.y` identik sebelum/sesudah).
- [ ] **AC5:** Setelah resize lalu expand → panel memakai lebar yang sama; collapse kembali → lebar tetap. (Tidak ada lebar terpisah untuk expanded.)
- [ ] **AC6:** Collapse/expand (animasi height) **tidak** mengubah posisi window sama sekali — `x` dan `y` identik sebelum/sesudah (tidak ada snap ke tengah layar / top-center).
- [ ] **AC7:** Drag window dari area bar (bukan handle, threshold > 4px) masih memindahkan window (perilaku existing).
- [ ] **AC8:** Klik bar (bukan handle) masih men-toggle expand/collapse; klik handle **tidak** men-toggle; double-click handle tidak men-toggle.
- [ ] **AC9:** Mouse meninggalkan window saat drag resize → resize tetap berjalan (pointer capture) dan berhenti bersih saat release (state `isResizing` reset, tidak ada resize macet).
- [ ] **AC10:** Nilai lebar tidak pernah < 160 dan tidak pernah > 640 — verified saat drag melewati batas (window berhenti berubah walau kursor lanjut, dan tidak ada invoke berulang).
- [ ] **AC11:** Window di-resize melebar saat berada di tepi layar → window tetap terlihat penuh di monitor (x di-clamp, tidak keluar layar), **dan lebar TETAP berubah sampai MIN/MAX sesuai kursor (bukan berhenti saat handle menyentuh tepi)** — konsisten dengan AC1/AC2.
- [ ] **AC12:** Pada display scaling 125%/150% (bila tersedia di mesin test), lebar mengikuti kursor secara proporsional (delta sudah dikonversi `devicePixelRatio`).
- [ ] **AC13:** `cargo check` + `cargo build` (src-tauri) dan `npm run build` sukses tanpa error, **tanpa dependency baru**, tanpa perubahan `tauri.conf.json`.
- [ ] **AC14:** Panggilan `invoke('resize_window', { width, height })` tanpa `preserve_center_x` tetap valid (backward compatible) — verified via jalur collapse/expand (AC6) dan kode review.
- [ ] **AC15:** Rapid double-click pada bar tidak menghasilkan state aneh (guard `generation` existing); resize yang berlangsung saat window kehilangan fokus/alt-tab dibersihkan via `lostpointercapture`/`pointercancel` (AC9/EC14).
- [ ] **AC16 (RQ2):** Pada `MIN_WIDTH = 160` (100% DPI), panel **expanded** tetap usable **tanpa horizontal overflow**: `.panel-header` menampilkan badge ("N active") utuh + title ter-ellipsis; baris agent aman — status terpanjang (`waiting confirmation` + badge `Terminal`) menampilkan label ter-ellipsis (contoh `waiting confir…`) dengan badge utuh, `.agent-path` juga ellipsis; `scrollWidth ≤ clientWidth` pada `.expanded-panel`, `.panel-header`, `.agent-item`, dan `.panel-footer` — verified via inspect di window 160px (kandidat `scrollWidth > clientWidth` tidak boleh ada).
- [ ] **AC17 (RQ5):** Selama drag resize cepat (mouse rate tinggi, ±1000Hz), invoke ke Rust tidak menumpuk — **maksimal 1 invoke per `requestAnimationFrame`** (verified via instrumentasi/`console.count` pada `applyPendingWidth`); nilai lebar final setelah release **selalu sama** dengan posisi kursor terakhir (flush di `endResize`), tidak ada nilai pending yang hilang (EC15).

---

## 9. Referensi

- [App.svelte — `WIDTH`, `resizeWindow`, `startDrag`, `expand`/`collapse`, `.compact-bar`](file:///D:/dev/experiment-poni-agent/poni-deck/src/App.svelte)
- [lib.rs — `resize_window` (lib.rs:76-82), `generate_handler!` (lib.rs:156)](file:///D:/dev/experiment-poni-agent/poni-deck/src-tauri/src/lib.rs)
- [tauri.conf.json — window overlay (baris 12-24)](file:///D:/dev/experiment-poni-agent/poni-deck/src-tauri/tauri.conf.json)
- [Tauri v2 — PhysicalSize / PhysicalPosition / `current_monitor`](https://docs.rs/tauri/latest/tauri/)
- [MDN — Pointer Events & `setPointerCapture`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)
- [Svelte — event modifiers (`stopPropagation`, `preventDefault`)](https://svelte.dev/docs/element-directives#on-eventname)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `2026-08-01` | Planner | Initial draft |
| `1.1.0` | `2026-08-01` | Planner | Revisi jawab review: RQ1 — AC eksplisit perilaku tepi layar (AC1/AC2/AC11 konsisten); RQ2 — keputusan layout expanded di 160px (CSS `.panel-header` ellipsis + badge flex-shrink) + AC16; RQ3 — rationale capability Rust (`core:window:*` tidak berlaku) + asumsi eksplisit di §5; RQ4 — koreksi disambiguation (`preventDefault` menekan `mousedown`, `data-no-drag` fallback, tiga lapis proteksi); RQ5 — throttle/coalescing via `requestAnimationFrame` + flush di `endResize` (mitigasi R6) + EC15 + AC17; RQ6 — fallback clamp `current_monitor()` + documented limitation DPI multi-monitor; RQ7 — fix referensi `startDrag` (App.svelte:88, `let dragStart` di :86) dan `lib.rs` (76-82, `generate_handler!` :156) |
| `1.2.0` | `2026-08-01` | Planner | Revisi jawab review putaran kedua: (1) **unifikasi konvensi label** — rujukan pertanyaan reviewer diberi prefix `RQ1–RQ7` (semua rujukan `Q2`/`Q3`/`Q5`/`Q6` di §1, §3 CSS, §3 Catatan Kapabilitas, T4, AC16, AC17, serta Revisi History 1.1.0), sedangkan rujukan pertanyaan internal §7 ditulis eksplisit `§7 Qx` (komentar code Rust §3 `resolusi Q7` → `resolusi §7 Q7`); (2) **definisi "requirement 8"** — tambah daftar Requirement 1–8 bernomor di §1 (Requirement 8 = tanpa kontrol lebar terpisah saat expanded: tidak ada handle di expanded, tanpa native resize border) dan ganti semua `(requirement 8)` di §3 Alternatif & §7 Q3 menjadi `(Requirement 8, §1)`; (3) **AC16 diperkuat** — tambah CSS baris agent aman di 160px (`.agent-item`/`.agent-info`/`.agent-status` `min-width: 0`, `.agent-status .status-label` ellipsis, `.agent-status .launcher-badge` `flex-shrink: 0`) di §3, update T4 & AC16 dengan contoh ukuran aman (`waiting confirmation` + `Terminal` → label `waiting confir…` ter-ellipsis, badge utuh, `scrollWidth ≤ clientWidth`). Update §1, §3, §6 (T4/T5), §7, §8 (AC16/AC17), Revisi History. |
