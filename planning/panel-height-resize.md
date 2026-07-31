# Planning: Resize Tinggi Panel Expanded di Overlay Dynamic Island (Agent Monitor)

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Draft` |
| Versi    | `1.2.0` |
| Tanggal  | `2026-08-01` |
| Author   | Planner |
| Reviewer | `(belum direview)` |

---

## 1. Tujuan

Memberikan kemampuan mengubah **tinggi window saat panel dalam keadaan expanded** dengan cara drag di tepi bawah panel. Tinggi efektif saat expanded menjadi **dinamis** (mengganti konstanta `EXPANDED_HEIGHT`), dipertahankan selama sesi (seperti `currentWidth`), dengan batas min/max, tanpa mengubah posisi window (x & y), tanpa merusak fitur yang sudah berjalan (panel-footer "Open Terminal", scroll `.agent-list`, klik item agent, drag-window/klik-toggle pada `.compact-bar`, resize width existing, animasi collapse/expand).

**Requirement (referensi bernomor — satu-satunya sumber definisi untuk rujukan "requirement N" di §3/§7):**

1. **Requirement 1** — Handle resize vertikal di **tepi bawah panel**, **HANYA tampil & aktif saat expanded**. Tidak mengganggu: `.panel-footer` (tombol "Open Terminal"), scroll `.agent-list`, klik item agent, dan drag-window/klik-toggle pada `.compact-bar`.
2. **Requirement 2** — Tinggi efektif saat expanded menjadi **dinamis**: ganti penggunaan konstanta `EXPANDED_HEIGHT`, dipertahankan selama sesi (seperti `currentWidth`), dipakai saat `expand()` memanggil `animateResize` / `resizeWindow`.
3. **Requirement 3** — Batas **min & max tinggi**: min **harus lebih besar dari tinggi bar collapsed** (panel tidak boleh lebih pendek dari bar); max **mempertimbangkan monitor** — window top-anchored, y dipertahankan, height tumbuh ke bawah. Beri nilai konkret + alasan.
4. **Requirement 4** — Posisi **x dan y TIDAK boleh berubah** saat resize height. Jangan pakai `preserve_center_x` untuk height; cukup `set_size` dengan width & height yang ada. Sebutkan apakah perlu perubahan di `lib.rs` — seharusnya **TIDAK** — verifikasi dan tulis.
5. **Requirement 5** — Ikuti pola yang sudah ada: **rAF throttle + flush**, disambiguation gesture (resize vs drag vs click), **DPI scale** (delta × `devicePixelRatio`), **pointer capture**, guard `isResizing`.
6. **Requirement 6** — Perilaku saat **collapse lalu expand lagi**: pakai **tinggi efektif terakhir** yang di-resize user.
7. **Requirement 7** — Perilaku saat **window sudah tidak di posisi y=0**: apakah max height harus disesuaikan agar panel tetap dalam layar? **Putuskan & tulis eksplisit** (boleh documented limitation bila terlalu kompleks).
8. **Requirement 8** — **Scope out**: tidak menyimpan tinggi antar sesi (ephemeral, konsisten dengan width), tidak native OS resize border, tidak corner handle 2D.

**Measurable:**
- Saat expanded, user dapat men-drag tepi bawah panel untuk mengubah tinggi window; tinggi mengikuti posisi kursor secara kontinyu (turun = lebih tinggi, naik = lebih pendek).
- Tinggi ter-clamp antara `minH` (dinamis, **physical px**) dan `MAX_HEIGHT = 520` (physical px, konsisten dengan `resize_window` yang menerima `PhysicalSize`). `minH = max(⌈MIN_HEIGHT × DPR⌉, ⌈(collapsedHeight() + 40) × DPR⌉)` dengan `MIN_HEIGHT = 180` sebagai **floor usable dalam CSS px** (bar ~42 + chrome header ~29 + gap 8×2 ~16 + footer ~36 + padding-bottom 12 ≈ 90 + agent-list ±48 ≈ total 180) — dikonversi × `devicePixelRatio` agar **seluruh perbandingan & penetapan tinggi berjalan dalam physical px** (keputusan Q1/Q2). Konsekuensi: di DPR 1.5, tinggi minimum = 270 physical px (= 180 CSS), **bukan** 180 physical (yang = 120 CSS dan akan men-clip footer).
- Posisi **x dan y** window tidak berubah selama resize height (diverifikasi via `outer_position` sebelum/sesudah); **lebar** (`currentWidth`) tidak disentuh.
- Setelah resize lalu collapse lalu expand lagi → panel memakai **tinggi efektif terakhir** (bukan `DEFAULT_HEIGHT`).
- Perbedaan antara **resize height**, **klik tombol "Open Terminal"**, **klik item agent**, **scroll agent-list**, dan **drag-window/klik-toggle compact-bar** dapat dibedakan dengan andal: drag pada handle tidak pernah memblokir/memicu interaksi lain; interaksi lain tidak pernah memicu resize.
- Pada tinggi minimum (`minH` = `⌈180 × devicePixelRatio⌉` physical px), panel tetap usable **tanpa vertical overflow**: `.panel-header`, `.agent-list` (scroll internal), dan `.panel-footer` semuanya tampil; tombol "Open Terminal" tetap bisa diklik. (Keputusan RQ6 — analisis & AC di §8 AC7/AC8; berlaku untuk semua DPR yang didukung karena `minH` dihitung dalam physical px, Q1.)

---

## 2. Scope

### In Scope

- [ ] **Handle resize di tepi bawah `.expanded-panel`** — elemen `.resize-handle--bottom`, **hanya tampil & aktif saat expanded**: CSS `display: none` saat `.expanded-panel:not(.visible)` + guard `if (!isLocked || !showPanel) return;` di `startResizeHeight`.
- [ ] **State tinggi efektif dinamis** — ganti konstanta `EXPANDED_HEIGHT` (App.svelte:12, :54, :130) dengan `expandedHeight` (mutable, default `DEFAULT_HEIGHT = 260` == nilai existing); `currentHeight` tetap sebagai tinggi window live; `expand()` (App.svelte:126) meng-animasi ke `expandedHeight`.
- [ ] **Clamp min/max** — `minH` dinamis dalam **physical px**: `Math.max(Math.ceil(MIN_HEIGHT * devicePixelRatio), Math.ceil((collapsedHeight() + 40) * devicePixelRatio))` dengan `MIN_HEIGHT = 180` (floor usable, CSS px; Q1) — selalu lebih besar dari tinggi bar collapsed (Requirement 3) dan selalu usable tanpa overflow di semua DPR yang didukung; `MAX_HEIGHT = 520` (physical px), di-enforce di sisi JS.
- [ ] **Handler resize height** — Pointer Events + `setPointerCapture`; disambiguation `preventDefault` + `stopPropagation` + `data-no-drag` (pola 3 lapis existing, meski handle berada di luar `.compact-bar` sehingga tabrakan dengan drag/toggle bar secara struktural mustahil); delta × `window.devicePixelRatio`; **rAF throttle + flush**; guard `isResizing` (shared, App.svelte:156).
- [ ] **Posisi x & y tidak berubah** — invoke `resizeWindow(newHeight)` **tanpa** `preserve_center_x` → `set_size` only di lib.rs:106 → x & y otomatis dipertahankan. **lib.rs TIDAK diubah** (verified §3).
- [ ] **Collapse → expand memakai tinggi efektif terakhir** — `expandedHeight` dipertahankan saat collapse; `expand()` meng-animasi ke nilai tersebut (Requirement 6).
- [ ] **CSS pendukung** — `.expanded-panel { position: relative; }` (anchor handle), `.resize-handle--bottom` (strip 10px di bottom, `cursor: ns-resize`, opacity hover), `.agent-list` mengisi ruang vertikal (`flex: 1 1 auto; min-height: 0`) agar tinggi ekstra benar-benar berguna (keputusan RQ6). Trade-off diterima (keputusan Q3): dengan 1–2 agent muncul void kosong di dalam list (R6/AC11/EC17).

### Out of Scope

- [ ] **Persistensi tinggi antar sesi** — **Keputusan: TIDAK dipersist**. Tinggi bersifat per-session (ephemeral), konsisten dengan lebar yang juga tidak dipersist (bar-width-resize.md, §2 Out of Scope). Tiap start memakai `DEFAULT_HEIGHT = 260`.
- [ ] **Native OS resize border / `resizable: true`** — `tauri.conf.json:22` tetap `"resizable": false`; tidak ada resize border native Windows (Requirement 8).
- [ ] **Corner handle 2D** (resize width + height sekaligus dalam satu drag) — hanya resize **height** dari tepi bawah (Requirement 8).
- [ ] **Perubahan perilaku width resize / lebar saat expanded** — width resize existing (App.svelte:158-207) **tidak disentuh**; tidak ada kontrol lebar tambahan saat expanded.
- [ ] **Perubahan `lib.rs` / `tauri.conf.json` / `generate_handler!`** — **TIDAK ada** (Requirement 4, verified §3).
- [ ] **Dynamic monitor work-area clamp per posisi y** — **Keputusan: documented limitation** (`MAX_HEIGHT` statis 520; lihat §7 Q4). Memerlukan akses andal ke work-area + posisi window (butuh perintah/izin Rust baru) yang bertentangan dengan keputusan "lib.rs tidak diubah" & "tanpa dependency baru".
- [ ] **Mengubah perilaku animasi collapse/expand selain target tinggi** — durasi, easing, dan fase `showPanel` dipertahankan.
- [ ] **Touch gesture kompleks** — hanya pointer/mouse-drag linear vertikal; `touch-action: none` menonaktifkan gesture bawaan pada handle.
- [ ] **Mengubah tinggi bar collapsed** / perilaku `collapsedHeight()` — bar tetap berukuran existing dan `collapsedHeight()` tidak diubah. Namun **konversi satuan** (CSS px → physical px) di call-site `collapse()` (App.svelte:143-145) dan `onMount` (App.svelte:233) **diperbaiki** — ini bug DPI pre-existing yang diwarisi dan dibutuhkan agar fitur ini konsisten (keputusan Q2).

---

## 3. Pendekatan

### Strategi Terpilih

**Frontend-only: seluruh perubahan di `agent-monitor/src/App.svelte`. `lib.rs` TIDAK diubah.** Memanfaatkan command `resize_window` existing yang sudah generic width+height (lib.rs:76-109).

#### 1. Ganti konstanta tinggi dengan state mutable

```js
const DEFAULT_HEIGHT = 260;   // PHYSICAL px == EXPANDED_HEIGHT existing (App.svelte:12)
const MIN_HEIGHT = 180;       // CSS px — floor usable (bar + padding + chrome + agent-list ±48); dikonversi ×devicePixelRatio saat masuk jalur physical (Q1)
const MAX_HEIGHT = 520;       // PHYSICAL px — konsisten dengan resize_window (PhysicalSize) & MAX_WIDTH

let currentHeight = DEFAULT_HEIGHT;   // tinggi window LIVE (PHYSICAL px; berubah saat animasi/collapse) — ganti App.svelte:54
let expandedHeight = DEFAULT_HEIGHT;  // target tinggi saat expanded (PHYSICAL px) — session state (seperti currentWidth)
```

- `EXPANDED_HEIGHT` dihapus total (tiga referensi: App.svelte:12, :54, :130).
- **Dua variabel, bukan satu**: `currentHeight` adalah tinggi aktual window (berubah saat collapse jadi tinggi bar ~42px, saat animasi jadi nilai intermediate), sedangkan `expandedHeight` adalah **target** yang persist per session — diperlukan karena saat collapse `currentHeight` turun ke tinggi bar tapi target expanded harus tetap diingat (Requirement 6). Analogi width: `currentWidth` tunggal karena lebar dipakai bersama untuk bar & panel; tinggi justru punya dua nilai berbeda (bar vs expanded) sehingga butuh target terpisah.
- Nilai `DEFAULT_HEIGHT = 260` = `EXPANDED_HEIGHT` lama → **tanpa perubahan visual saat startup / expand pertama**.

#### 2. Update `expand()` (App.svelte:126-133)

```js
async function expand() {
  const gen = ++generation;
  clearTimeout(phaseTimer);
  isLocked = true;
  await animateResize(currentHeight, expandedHeight, 320, gen);  // ← ganti EXPANDED_HEIGHT (App.svelte:130)
  if (gen !== generation) return;
  showPanel = true;
}
```

- `collapse()` (App.svelte:135-147): logika **tidak berubah** — `expandedHeight` dibiarkan utuh saat collapse sehingga expand berikutnya meng-animasi ke tinggi efektif terakhir. **Satu-satunya perubahan (keputusan Q2): konversi satuan** target tinggi bar `h = collapsedHeight()` (CSS px) → `Math.ceil(h * window.devicePixelRatio)` (physical px) sebelum `animateResize`. Ini memperbaiki **bug DPI pre-existing yang diwarisi**: sebelumnya tinggi bar CSS dipakai langsung sebagai physical px (bar ter-clip di DPR ≠ 1). Konversi yang sama diterapkan di `onMount` (App.svelte:233): `resizeWindow(collapsedHeight())` → `resizeWindow(Math.ceil(collapsedHeight() * window.devicePixelRatio))`. `collapsedHeight()` sendiri tidak diubah — bar tetap berukuran existing (Out of Scope).
- `animateResize` (App.svelte:79-95) & `resizeWindow` (App.svelte:61-72) tidak perlu diubah — `resizeWindow(height)` memakai `currentWidth` dan `preserve_center_x: false` → jalur set_size only.

#### 3. Markup — handle di tepi bawah `.expanded-panel` (di dalam panel, setelah `.panel-footer`, App.svelte:303-307)

```svelte
<div class="expanded-panel" class:visible={showPanel}>
  <!-- panel-header, empty-state/agent-list, panel-footer (existing) ... -->
  <div class="panel-footer">
    <button class="footer-btn" on:click|stopPropagation={/* Open Terminal */}>Open Terminal</button>
  </div>
  <span class="resize-handle--bottom"
        data-no-drag
        on:pointerdown={startResizeHeight}
        on:pointermove={onResizeHeightMove}
        on:pointerup={endResizeHeight}
        on:pointercancel={endResizeHeight}
        on:lostpointercapture={endResizeHeight}
        on:click|stopPropagation
        on:contextmenu|preventDefault></span>
</div>
```

**Disambiguation resize vs interaksi lain (Requirement 1):**

| Interaksi | Elemen | Mekanisme pembeda | Hasil |
|-----------|--------|-------------------|-------|
| **Resize height** | `.resize-handle--bottom` | Strip **absolute di `bottom: 0`**, tinggi 10px, menempati area padding-bottom 12px panel (di **bawah** footer). Pointer Events + `setPointerCapture`; `preventDefault` + `stopPropagation` + `data-no-drag` (pola 3 lapis existing, defensif). Guard `!isLocked \|\| !showPanel` → aktif **hanya saat expanded**. | Tinggi berubah, x/y tidak, tidak ada interaksi lain terpicu |
| **Klik "Open Terminal"** | `.footer-btn` (App.svelte:304) | Button berada **di atas strip** (footer berakhir 12px dari bottom; strip hanya 0–10px) → tidak overlap; z-index strip (5) tidak menutupi button. | Terminal dibuka |
| **Scroll `.agent-list`** | `.agent-list` | List berada di tengah (di atas footer), bukan di strip bawah; strip hanya menutup 10px terbawah. | List tetap scroll |
| **Klik item agent** | `.agent-item` (App.svelte:286) | Item di `.agent-list`, jauh di atas strip; `on:click\|stopPropagation` existing tetap jalan. | `openFolder` dipanggil |
| **Drag-window / toggle `.compact-bar`** | `.compact-bar` (App.svelte:247) | Handle **berada di luar `.compact-bar`** → `startDrag`/`toggleLock` secara struktural **tidak pernah terpicu** dari handle. Pola `data-no-drag`/`stopPropagation`/`preventDefault` tetap dipasang (defensif). | Bar behavior existing utuh |

#### 4. Handler resize height (Pointer Events + capture, rAF throttle + flush)

```js
let resizeStartHeight = null;

function startResizeHeight(e) {
  e.preventDefault();
  e.stopPropagation();
  if (!isLocked || !showPanel) return;        // HANYA aktif saat expanded penuh (Requirement 1)
  if (isResizing) return;                     // guard defensif (shared, App.svelte:156)
  isResizing = true;
  const handle = e.currentTarget;
  handle.setPointerCapture(e.pointerId);      // pointer tetap terima event walau keluar window
  resizeStartHeight = {
    pointerId: e.pointerId,
    startClientY: e.clientY,
    startHeight: expandedHeight,
    minH: Math.max(                                              // PHYSICAL px (Q1/Q2) — kedua term adalah hasil konversi dari CSS px
      Math.ceil(MIN_HEIGHT * window.devicePixelRatio),           // floor usable 180 CSS → physical (180 @DPR1, 270 @DPR1.5, 360 @DPR2)
      Math.ceil((collapsedHeight() + 40) * window.devicePixelRatio), // bar CSS + 40 → physical; guard Requirement 3 (dominated oleh term pertama selama bar ≤ 140 CSS, karena ⌈(bar+40)×DPR⌉ ≤ ⌈180×DPR⌉ saat bar+40 ≤ 180)
    ),
    pendingHeight: null,
    rafId: null,
  };
}

function onResizeHeightMove(e) {
  if (!isResizing || !resizeStartHeight || e.pointerId !== resizeStartHeight.pointerId) return;
  // clientY CSS px → physical px, konsisten dengan resize_window (u32 physical)
  const dy = (e.clientY - resizeStartHeight.startClientY) * window.devicePixelRatio;
  resizeStartHeight.pendingHeight = Math.round(clamp(
    resizeStartHeight.startHeight + dy,
    resizeStartHeight.minH,
    MAX_HEIGHT
  ));
  if (resizeStartHeight.rafId == null) {
    resizeStartHeight.rafId = requestAnimationFrame(applyPendingHeight);
  }
}

function applyPendingHeight() {
  if (!isResizing || !resizeStartHeight) return;
  resizeStartHeight.rafId = null;
  const newHeight = resizeStartHeight.pendingHeight;
  if (newHeight == null) return;
  if (newHeight === expandedHeight) return;   // guard nilai sama (di luar clamp) — skip invoke redundan
  expandedHeight = newHeight;
  resizeWindow(newHeight);                    // preserve_center_x=false → set_size only → x & y TIDAK disentuh
}

function endResizeHeight(e) {
  if (!isResizing) return;
  if (resizeStartHeight && resizeStartHeight.rafId != null) {
    cancelAnimationFrame(resizeStartHeight.rafId);
    resizeStartHeight.rafId = null;
    applyPendingHeight();                     // FLUSH nilai final — tinggi selalu berakhir di posisi kursor terakhir
  }
  if (resizeStartHeight && e.currentTarget?.hasPointerCapture?.(resizeStartHeight.pointerId)) {
    e.currentTarget.releasePointerCapture(resizeStartHeight.pointerId);
  }
  isResizing = false;
  resizeStartHeight = null;
}
```

- **Arah drag (Requirement 4):** window top-anchored (y dipertahankan), height tumbuh ke bawah → `dy` positif (drag ke bawah) = **lebih tinggi**, `dy` negatif = lebih pendek. Tanpa flip arah. Delta relatif terhadap `clientY` awal → benar walaupun window tidak di y=0 (posisi absolut tidak relevan, hanya delta).
- **`setPointerCapture`** — `pointermove`/`pointerup` tetap diterima handle walau kursor meninggalkan area strip/window; cleanup di `pointercancel`/`lostpointercapture` (jika OS membatalkan capture).
- **rAF throttle/coalescing** — `onResizeHeightMove` (event 500–1000Hz) hanya menghitung `pendingHeight` + menjadwalkan **maksimal 1 invoke per frame** (`rafId` guard); guard `newHeight === expandedHeight` (di luar clamp) mencegah invoke redundan; **flush nilai final di `endResizeHeight`** → tidak ada invoke menggantung. Strukturnya mirip width-resize (App.svelte:175-207), tapi lihat koreksi Q5 di bawah — **bukan pola identik**.
- **`isResizing` shared** dengan width resize — aman karena handle width (bar) hanya aktif saat **collapsed** (`.compact-bar.expanded .resize-handle { display: none }`, App.svelte:380) dan handle height hanya saat **expanded** → tidak pernah aktif bersamaan. `toggleLock` (App.svelte:150) & `startDrag` (App.svelte:100) sudah punya guard `if (isResizing) return;`.
- **Koreksi klaim "pola identik width-resize" (keputusan Q5):** `startResize` width (App.svelte:158-173) **TIDAK punya guard `isResizing`** di awal (hanya `if (isLocked) return;`), sedangkan `startResizeHeight` menambah `if (isResizing) return;`. Perbedaan ini **disengaja** dan implementator TIDAK boleh menyalin guard secara asal: width resize aktif hanya saat **collapsed** (guard `isLocked`; satu-satunya pemegang `isResizing` di fase itu adalah dirinya sendiri, jadi guard fase sudah cukup), sedangkan height resize aktif **hanya saat window == expandedHeight** (fase expanded penuh, `showPanel = true`; di fase itu `isLocked` SELALU true karena `expand()` men-set `isLocked = true` (App.svelte:129) dan `isExpanded = isLocked`, sehingga klausa `!isLocked` pada guard tidak pernah aktif — guard aktif murni karena `showPanel = true`) sehingga `isResizing` di `startResizeHeight` murni **defensif** — bukan untuk mutual exclusion. Selain itu `applyPendingWidth` membanding `newWidth === currentWidth` (width resize aktif saat collapsed, `currentWidth` = nilai live yang sekaligus target), sedangkan `applyPendingHeight` membanding `newHeight === expandedHeight` — sah karena saat fase aktif (post-expand) berlaku deterministik `currentHeight === expandedHeight`, dan `expandedHeight` dipilih sebagai target session yang stabil (tidak bergantung nilai transien `currentHeight` selama drag).

#### 5. CSS (`App.svelte` `<style>`)

```css
.expanded-panel { position: relative; }                     /* anchor untuk handle (tambah ke rule App.svelte:431) */

.expanded-panel:not(.visible) .resize-handle--bottom { display: none; }   /* HANYA saat expanded (Requirement 1) */

.resize-handle--bottom {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 10px;
  cursor: ns-resize;
  touch-action: none;                                        /* matikan gesture bawaan touch */
  z-index: 5;
  opacity: 0;                                                /* tipis, muncul saat hover/aktif */
  transition: opacity 0.15s ease;
}
.expanded-panel:hover .resize-handle--bottom { opacity: 0.5; }
.resize-handle--bottom:hover, .resize-handle--bottom:active { opacity: 1; }

/* Pendukung (RQ6): agent-list mengisi ruang vertikal — ganti App.svelte:495-501 */
.agent-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  /* hapus max-height: 140px */
}
```

- **Posisi handle vs footer (RQ7):** strip 10px menempati padding-bottom 12px `.expanded-panel` (App.svelte:436) — area 0–10px dari bottom. `.panel-footer` (App.svelte:598-603) & tombol berada di **atas** area itu → **tidak tertutup**, "Open Terminal" tetap bisa diklik.
- `.agent-list` menjadi `flex: 1 1 auto` → tinggi ekstra hasil resize benar-benar menambah area list (bukan ruang kosong mati di antara list & footer). `min-height: 0` mengizinkan flex child menyusut + `overflow-y: auto` menangani banyak agent (internal scroll, tidak overflow window).

#### 6. Verifikasi `lib.rs` — **TIDAK perlu diubah** (Requirement 4)

Dibaca langsung dari kode asli: `resize_window` (lib.rs:76-109) sudah menerima `width: f64, height: f64, preserve_center_x: Option<bool>`. Branch `preserve_center_x == Some(true)` (lib.rs:85-104) — satu-satunya jalur yang menyentuh posisi — **tidak dijalankan** bila param `None`/`false`. Branch `else` (lib.rs:105-107) hanya `window.set_size(size)` → **x dan y otomatis dipertahankan**. 

Panggilan JS `resizeWindow(newHeight)` → invoke `resize_window` dengan `{ width: currentWidth, height, preserve_center_x: false }` (App.svelte:64-68) → masuk branch `else` → `set_size` only. **Kesimpulan: tidak ada perubahan di `lib.rs`, `generate_handler!` (lib.rs:183-192), maupun `tauri.conf.json`.** Ini berbeda dari fitur width-resize (yang memang perlu param `preserve_center_x` baru); untuk height tidak ada kebutuhan mempertahankan center karena posisi x/y utuh oleh desain `set_size`.

#### Alur data

```
Pointer drag pada .resize-handle--bottom (expanded, showPanel=true)
   │  pointerdown → preventDefault + stopPropagation, guard !isLocked || !showPanel,
   │               setPointerCapture, simpan (startClientY, startHeight=expandedHeight, minH physical px)
   │  pointermove (rate mouse 60–1000Hz):
   │     dy = (clientY - startY) * devicePixelRatio
   │     pendingHeight = clamp(startHeight + dy, minH, MAX_HEIGHT)     ← clamp di sini
   │     schedule applyPendingHeight() via requestAnimationFrame        ← maks 1 invoke per frame
   ▼
applyPendingHeight (1× per frame):
   │     guard newHeight === expandedHeight → skip invoke
   │     expandedHeight = newHeight
   ▼
invoke('resize_window', { width: currentWidth, height: newHeight })    ← preserve_center_x absent/false
   ▼
lib.rs resize_window: preserve_center_x != Some(true) → set_size only (lib.rs:105-107) → x & y TIDAK disentuh
   ▼
pointerup → endResizeHeight: flush applyPendingHeight() (nilai final), release capture, cleanup
   ▼
Window bertambah/berkurang tinggi; posisi x/y & lebar tetap; collapse→expand berikutnya memakai expandedHeight.
```

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| **Command Rust baru (mis. `resize_window_preserve_y`) / clamp work-area dinamis per posisi y** | Requirement 4: "seharusnya TIDAK perlu ubah lib.rs" — verified benar, `resize_window` sudah generic width+height & branch `set_size` only menjaga x/y. Menambah command = dependency baru + melanggar keputusan. Clamp work-area dinamis butuh `work_area()` + posisi window di sisi JS/Rust yang tidak tersedia tanpa kerja/permission baru (§7 Q4). |
| **JS `currentMonitor()` async per drag untuk clamp tinggi** | Tidak ada `workArea` di `Monitor` JS (hanya `size`/`position`/`scaleFactor`), dan pemanggilan async per-frame menambah kompleksitas tanpa jaminan akurasi; tidak dipakai. |
| **Handle menimpa `.panel-footer` (strip di atas tombol)** | Menutupi/bersaing dengan tombol "Open Terminal" (Requirement 1). Strip ditaruh di **bawah** footer (padding-bottom panel) → tidak overlap. |
| **`.agent-list` tetap `max-height: 140px`** | Resize tinggi hanya menambah ruang kosong mati di bawah list; fitur terasa tidak berguna. Ubah ke `flex: 1 1 auto; min-height: 0` (RQ6) — list tumbuh, footer tetap di bawah. |
| **Memakai satu variabel `currentHeight` sebagai target** (tanpa `expandedHeight`) | Saat collapse `currentHeight` turun ke tinggi bar — target expanded hilang → expand berikutnya salah (Requirement 6). Butuh target terpisah. |
| **Persistensi tinggi antar sesi (localStorage/store)** | Ephemeral, konsisten dengan lebar (bar-width-resize.md §2) & Requirement 8. |
| **Corner handle 2D / native resize border** | Requirement 8: out of scope. |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| R1: Handle memblokir tombol "Open Terminal" / klik item agent | Rendah | Sedang | Strip 10px absolute di **bawah** footer (menempati padding-bottom panel); tombol di atasnya; list di tengah. Klik tetap berfungsi (AC8/AC9). |
| R2: Flicker/jank saat resize (banyak invoke per detik) | Sedang | Rendah | rAF throttle/coalescing — maks **1 invoke per frame** (`rafId` guard); guard `newHeight === expandedHeight` (di luar clamp) mencegah invoke redundan; **flush final di `endResizeHeight`**; delta di-round ke integer. Dampak terburuk: satu frame terlambat, self-correcting. |
| R3: Panel menjadi lebih pendek dari bar collapsed | Rendah | Sedang | `minH` dihitung saat drag start dalam **physical px**: `Math.max(Math.ceil(MIN_HEIGHT * devicePixelRatio), Math.ceil((collapsedHeight() + 40) * devicePixelRatio))` (Requirement 3). `collapsedHeight()` (App.svelte:74-77) membaca bar asli dari DOM (CSS px) → dikonversi × `devicePixelRatio` → robust walau ukuran bar berubah & benar di semua DPR (Q1). |
| R4: DPI scaling ≠ 100% membuat tinggi tidak mengikuti kursor / geometri min tidak konsisten | Sedang | Rendah | Delta dikalikan `window.devicePixelRatio` (clientY = CSS px, window size = physical px) — pola width-resize (App.svelte:177); **dan** `minH` dihitung penuh dalam physical px (konversi ×DPR) sehingga di DPR apa pun geometri CSS minimum sama (180 CSS → tidak ada overflow/clip). Kesalahan satuan campuran yang ditemukan reviewer (Q1) diperbaiki di sini, bukan hanya di delta. |
| R5: Resize height saat window sudah tidak di y=0 → bottom panel melewati batas bawah layar | Sedang | Rendah | **Documented limitation** (Requirement 7, §7 Q4): `MAX_HEIGHT = 520` statis — dipilih agar selalu muat di work-area monitor umum **saat window di y=0** (posisi default). Hanya overflow bila user sengaja men-drag window ke bawah (lihat Q4). |
| R6: Regresi fitur existing (toggle, drag window, width resize, animasi, klik item, scroll list) | Sedang | Tinggi | Jalur collapse/expand hanya **target animasi** diganti (`EXPANDED_HEIGHT` → `expandedHeight`); width resize (App.svelte:158-207) & `resizeWindow` tidak diubah; handle di luar `.compact-bar`; checklist regresi manual di AC11. **Regresi kosmetik yang diterima (keputusan Q3):** dengan 1–2 agent, `.agent-list` (`flex: 1 1 auto`) meregang penuh → void kosong antara item terakhir & footer, paling terlihat di `MAX_HEIGHT` 520 — diterima sebagai konsekuensi inherent panel yang dapat di-resize (bukan regresi fungsional; tidak ada elemen ter-overlap); didokumentasikan di AC11 & EC17. |
| R7: Resize height aktif sebelum animasi expand selesai | Rendah | Sedang | Guard `!isLocked \|\| !showPanel` — handle hanya aktif setelah `showPanel = true` (fase akhir expand). `isResizing` guard mencegah race. |

### Edge Case

- [ ] **EC1: Handle diklik/didrag saat tidak expanded** — CSS `display: none` saat `:not(.visible)` + guard `!isLocked || !showPanel` → tidak ada efek (Requirement 1).
- [ ] **EC2: Resize height vs tombol "Open Terminal"** — strip di bawah footer, tidak overlap → tombol tetap bisa diklik (AC8).
- [ ] **EC3: Resize height vs scroll `.agent-list`** — strip hanya menutup 10px terbawah; area scroll list di atasnya → scroll normal (AC9).
- [ ] **EC4: Resize height vs klik item agent** — item di `.agent-list` jauh di atas strip; `openFolder` tetap terpicu (AC9).
- [ ] **EC5: Resize height vs drag-window/toggle `.compact-bar`** — handle **di luar** `.compact-bar` → `startDrag`/`toggleLock` secara struktural mustahil terpicu; `preventDefault` + `stopPropagation` + `data-no-drag` dipasang defensif (AC9).
- [ ] **EC6: Mouse meninggalkan window saat drag resize** — `setPointerCapture` → `pointermove`/`pointerup` tetap diterima; cleanup di `pointercancel`/`lostpointercapture` (mis. alt-tab).
- [ ] **EC7: Drag melewati MIN/MAX** — clamp dihitung saat `pendingHeight` di-compute; guard `newHeight === expandedHeight` (di luar clamp) → tidak ada invoke berulang; tinggi berhenti berubah walau kursor lanjut.
- [ ] **EC8: Release cepat sebelum frame rAF berikutnya** — `cancelAnimationFrame` + **flush langsung** di `endResizeHeight` (`applyPendingHeight()`) → tinggi akhir selalu di-apply, tidak ada pending value hilang / invoke setelah `isResizing = false`.
- [ ] **EC9: DPI 125%/150%** — delta × `devicePixelRatio` (tinggi physical mengikuti kursor) **dan** `minH` dikonversi ×DPR → di tinggi minimum geometri CSS selalu 180 (usable, tanpa clip footer) di semua DPR yang didukung (AC5/AC7; Q1).
- [ ] **EC10: Resize height → collapse → expand lagi** — `expandedHeight` dipertahankan saat collapse → expand berikutnya animasi ke **tinggi efektif terakhir** (Requirement 6, AC4).
- [ ] **EC11: Resize height saat animasi expand belum selesai** — guard `!showPanel` (false selama animasi) → handle tidak aktif; `isResizing` defensif (R7).
- [ ] **EC12: Window di-drag ke bawah (y≠0) lalu resize tinggi** — tinggi tumbuh ke bawah dari posisi saat ini; dengan `MAX_HEIGHT = 520` bottom mungkin melewati work-area bawah bila y besar — **documented limitation** (Requirement 7, §7 Q4), tidak merusak state (x/y tetap). **Konsekuensi praktis (Q4):** strip `.resize-handle--bottom` dapat keluar layar / tertutup taskbar → handle **tidak bisa di-grab** untuk mengecilkan; satu-satunya jalan adalah drag window ke atas via `.compact-bar` lalu resize (lihat EC18).
- [ ] **EC13: Banyak agent (> 5)** — `.agent-list` `flex: 1 1 auto; min-height: 0; overflow-y: auto` → scroll internal; footer tetap di bawah; tidak overflow window.
- [ ] **EC14: Double-click pada strip** — `pointerdown` di-`preventDefault` + `click` di-`stopPropagation` → tidak ada toggle (tidak ada aksi toggle di panel); resize start/stop dua kali secara bersih.
- [ ] **EC15: Alt-tab / kehilangan fokus saat resize** — `lostpointercapture`/`pointercancel` → `endResizeHeight` → state bersih (`isResizing` reset), tidak ada resize macet.
- [ ] **EC16: Resize height saat `currentWidth` sudah diubah user (width resize)** — `resizeWindow(newHeight)` memakai `currentWidth` → lebar tidak berubah selama resize height; kedua fitur konsisten (AC3).
- [ ] **EC17: Sedikit agent (1–2) pada tinggi besar** — `.agent-list` meregang penuh → void kosong antara item terakhir & footer (paling terlihat di `MAX_HEIGHT`). **Diterima** sebagai trade-off eksplisit (keputusan Q3): tinggi ekstra tetap berguna saat agent bertambah; tidak ada elemen yang ter-overlap; footer tetap di bawah; empty-state menangani kasus 0 agent (R6/AC11).
- [ ] **EC18: Strip bottom keluar layar (handle di belakang taskbar)** — window di-drag ke bawah dekat taskbar saat expanded → strip `.resize-handle--bottom` off-screen (tertutup taskbar/batas layar) → user **tidak bisa** mengecilkan tinggi dari strip. **Workaround:** drag window ke atas via `.compact-bar` (drag-window tetap berfungsi karena strip di luar `.compact-bar`) untuk memunculkan kembali strip, baru resize. Tidak merusak state (x/y/tinggi tetap). Documented limitation (§7 Q4).

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| tauri (crate) | sudah ada di `Cargo.toml` | **Tidak dipakai ulang secara baru** — tidak ada command/modifikasi Rust. `resize_window` existing dipakai apa adanya |
| @tauri-apps/api | ^2 (sudah ada di `package.json`) | `invoke` — dipakai existing; **tidak ada API window baru** di sisi JS (resize via command Rust, bukan `appWindow.setSize`) |

### Service

| Service | Tujuan |
|---------|--------|
| N/A | Semuanya lokal, tidak ada service eksternal |

### Internal

| Dependency | Tujuan |
|------------|--------|
| `App.svelte::resizeWindow` (App.svelte:61-72) | **Tidak diubah** — dipanggil `resizeWindow(newHeight)` dengan default `preserve_center_x=false` → set_size only |
| `App.svelte::expand` (App.svelte:126-133) | **Diubah** — target animasi `EXPANDED_HEIGHT` (App.svelte:130) → `expandedHeight` |
| `App.svelte::collapse` (App.svelte:135-147) | **Diubah minimal (keputusan Q2)** — logika & `expandedHeight` utuh (Requirement 6); target `h = collapsedHeight()` (CSS px) dikonversi `Math.ceil(h * window.devicePixelRatio)` (physical px) sebelum `animateResize`. Hal yang sama di `onMount` (App.svelte:233). |
| `App.svelte::collapsedHeight` (App.svelte:74-77) | **Dipakai, tidak diubah** — mengembalikan **CSS px** (`getBoundingClientRect`); dikalikan × `devicePixelRatio` di call-site (`minH`, `collapse`, `onMount`) agar masuk ke jalur physical px (Q1/Q2) |
| `App.svelte::isResizing` (App.svelte:156) | **Dipakai shared** — guard defensif di `startResizeHeight`; sudah ada di `toggleLock` (:150) & `startDrag` (:100) |
| `App.svelte::clamp` (App.svelte:57-59) | **Dipakai** — clamp `pendingHeight` |
| `App.svelte::currentWidth` (App.svelte:55) | **Tidak disentuh** — dipakai `resizeWindow`; lebar tidak berubah saat resize height |
| `lib.rs::resize_window` (lib.rs:76-109) | **TIDAK diubah** — verified: branch `else` (lib.rs:105-107) `set_size` only menjaga x/y |
| `lib.rs::generate_handler!` (lib.rs:183-192) | **TIDAK diubah** — command sudah terdaftar |
| `tauri.conf.json` (baris 12-24) | **TIDAK diubah** — tetap `resizable: false`, `transparent: true`, `decorations: false` |

> **Catatan:** tidak ada perubahan konfigurasi, tidak ada dependency baru, tidak ada perubahan pada `lib.rs`, `tauri.conf.json`, maupun `generate_handler!`. Semua pemanggilan window (`set_size`) tetap dari sisi Rust (command existing) → capability `core:window:*` tetap tidak berlaku untuk kode Rust (konsisten dengan analisis bar-width-resize.md §5).

---

## 6. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [ ] **T1: App.svelte — konstanta & state tinggi** — hapus `const EXPANDED_HEIGHT = 260` (App.svelte:12); tambah `const DEFAULT_HEIGHT = 260` (physical px), `const MIN_HEIGHT = 180` (**CSS px** floor usable; Q1), `const MAX_HEIGHT = 520` (physical px); ganti `let currentHeight = EXPANDED_HEIGHT;` (App.svelte:54) → `let currentHeight = DEFAULT_HEIGHT;` (physical px); tambah `let expandedHeight = DEFAULT_HEIGHT;` (physical px). [S]
- [ ] **T2: App.svelte — update `expand()` + konversi satuan collapse/onMount** — ganti target animasi `animateResize(currentHeight, EXPANDED_HEIGHT, 320, gen)` (App.svelte:130) → `animateResize(currentHeight, expandedHeight, 320, gen)`. Konversi bug DPI pre-existing (Q2): di `collapse()` (App.svelte:143-145) dan `onMount` (App.svelte:233), bungkus `collapsedHeight()` menjadi `Math.ceil(collapsedHeight() * window.devicePixelRatio)` sebelum dipakai sebagai target `animateResize`/`resizeWindow`. [S]
- [ ] **T3: App.svelte — handler resize height** — tambah state `let resizeStartHeight = null;`; fungsi `startResizeHeight` (guard `!isLocked || !showPanel`, `if (isResizing) return;` defensif, `setPointerCapture`, hitung `minH` dalam physical px: `Math.max(Math.ceil(MIN_HEIGHT * window.devicePixelRatio), Math.ceil((collapsedHeight() + 40) * window.devicePixelRatio))`, simpan `{pointerId, startClientY, startHeight, minH, pendingHeight, rafId}`), `onResizeHeightMove` (delta `(clientY - startY) * devicePixelRatio`, clamp `[minH, MAX_HEIGHT]`, rAF schedule via `rafId`), `applyPendingHeight` (guard `newHeight === expandedHeight` skip invoke — banding dengan target session, bukan `currentWidth`; set `expandedHeight`; `resizeWindow(newHeight)`), `endResizeHeight` (cancelAnimationFrame + **flush** `applyPendingHeight()`, releasePointerCapture, reset `isResizing`/`resizeStartHeight`). Semua handler `e.preventDefault()` + `e.stopPropagation()` diawal. Jangan menyalin guard width secara asal (lihat koreksi Q5). [M]
- [ ] **T4: App.svelte — markup handle** — tambah elemen `<span class="resize-handle--bottom" data-no-drag ...>` dengan handler pointer (`pointerdown/move/up/cancel/lostpointercapture`) + `on:click|stopPropagation` + `on:contextmenu|preventDefault` di dalam `.expanded-panel`, **setelah** `.panel-footer` (App.svelte:303-307). [S]
- [ ] **T5: App.svelte — CSS** — tambah `position: relative;` pada `.expanded-panel` (App.svelte:431); `.expanded-panel:not(.visible) .resize-handle--bottom { display: none; }`; `.resize-handle--bottom` (absolute bottom 0 full-width, height 10px, `cursor: ns-resize`, `touch-action: none`, z-index 5, opacity hover/active 0→0.5→1); ubah `.agent-list` (App.svelte:495-501): `max-height: 140px` → `flex: 1 1 auto; min-height: 0;` (pertahankan `overflow-y: auto`). [S]
- [ ] **T6: Verifikasi & build** — `cargo check` + `cargo build` di `agent-monitor/src-tauri` (pastikan **tanpa perubahan** — hanya validasi kompil); `npm run build` di `agent-monitor`; jalankan test matrix AC (termasuk **DPI scaling 125%/150% bila tersedia** — verifikasi AC7/AC9 tinggi minimum di DPR ≠ 1: tidak ada clip footer, `minH` = `⌈180 × DPR⌉`); regresi penuh fitur existing (toggle, drag window, width resize, klik item, scroll list, Open Terminal); verifikasi void kosong saat 1 agent di `MAX_HEIGHT` adalah yang diharapkan (EC17). [M]

**Dependency antar-task:**
- T1 → T2 → T3 (state dulu, baru target expand, baru interaksi pointer).
- T4 & T5 independen terhadap T1–T3 secara logic, tapi markup handle (T4) mendahului CSS-nya (T5) agar target styling jelas; keduanya bisa paralel dengan T3.
- T3 → T6, T4/T5 → T6 (verifikasi setelah semua sisi selesai).

---

## 7. Open Questions

- [x] **Q1: Nilai MIN_HEIGHT?** — **Keputusan (revisi 1.1.0, menjawab kritik satuan campuran):** `MIN_HEIGHT = 180` didefinisikan sebagai **floor usable dalam CSS px** (bukan physical), dan saat drag di-enforce dinamis dalam **physical px**: `minH = Math.max(Math.ceil(MIN_HEIGHT * devicePixelRatio), Math.ceil((collapsedHeight() + 40) * devicePixelRatio))`. Alasan: (a) **harus di atas tinggi bar collapsed** — bar terukur ~42 CSS px via `collapsedHeight()` (App.svelte:74-77, `Math.ceil(... ?? 42)`) dengan `min-height: 40px` (App.svelte:338); term kedua menjamin `minH` > bar + 40 CSS walau ukuran bar berubah, tapi selama bar ≤ 140 CSS term ini **didominasi** term pertama (`180 × DPR`) — karena `⌈(bar+40)×DPR⌉ ≤ ⌈180×DPR⌉` saat `bar+40 ≤ 180`; (b) **panel tetap usable tanpa overflow di semua DPR** — chrome fixed (CSS px): padding-bottom 12 + header ~29 (padding 4+8, teks 13px, border) + gap 8×2 + footer ~36 ≈ 90 → di window 180 CSS (dikurangi bar ~42) tersisa ±48 CSS untuk `.agent-list` ≈ 2 baris agent. **Koreksi kritik reviewer:** klaim "±48px" hanya valid sebagai **CSS px pada DPR=1**. Formula lama (`max(180, collapsedHeight()+40)`) mencampur satuan: di DPR 1.5, 180 **physical** = 120 CSS < 138 CSS kebutuhan chrome → footer ter-clip (`overflow: hidden`) → AC7/AC8 gagal di 125%/150%. Formula baru menghitung `minH` seluruhnya dalam physical px sehingga **geometri CSS minimum selalu 180** di DPR apa pun. (Varian reviewer `⌈(collapsedHeight()+48)×DPR⌉` terlampau kecil — hanya menjamin bar+48, tidak mencakup chrome ~90 CSS; subsumed oleh `⌈180×DPR⌉`.)
- [x] **Q2: Nilai MAX_HEIGHT?** — **Keputusan:** `MAX_HEIGHT = 520`. Alasan: window top-anchored di y=0 secara default (lib.rs setup :158, `set_position(x, 0)`); 520 muat di work-area monitor umum saat window di y=0 — 1366×768 → ~728, 1920×1080 → ~1040, 1440×900 → ~860 (work-area = tinggi minus taskbar); ±2× default 260 → island tetap terasa overlay, bukan jendela penuh; cukup menampung banyak agent (list `flex: 1 1 auto` + scroll). Konsisten dengan `MAX_WIDTH = 640` yang juga dirancang dari proporsi layar.
- [x] **Q3: Satu atau dua state tinggi?** — **Keputusan:** dua — `currentHeight` (tinggi live, sudah ada) + `expandedHeight` (target session, baru). `currentHeight` turun ke tinggi bar (~42px) saat collapse sehingga **tidak bisa** sekaligus menyimpan target expanded (Requirement 6); `expandedHeight` dipertahankan saat collapse.
- [x] **Q4: Window tidak di y=0 — apakah max height disesuaikan agar panel tetap dalam layar?** — **Keputusan: Documented limitation — `MAX_HEIGHT` statis 520, TIDAK di-clamp dinamis terhadap posisi y.** Alasan: (a) clamp work-area dinamis butuh `monitor.work_area()` + posisi window saat itu — di JS Tauri v2, `Monitor` tidak mengekspos work-area dan posisi window butuh permission/`outer_position` yang menambah kerja + melanggar keputusan **"lib.rs tidak diubah"** (Requirement 4) dan "tanpa dependency baru"; (b) window default top-anchored y=0 (lib.rs:158) sehingga 520 selalu muat di work-area umum; (c) satu-satunya skenario overflow adalah user **sengaja** men-drag window ke bawah dekat taskbar → bottom panel mungkin melewati batas bawah work-area (masih dalam layar fisik, bisa tertutup taskbar) — diterima sebagai toleransi, window frameless/transparent tidak rusak oleh ini; **konsekuensi praktis yang eksplisit (keputusan Q4):** pada skenario ini strip `.resize-handle--bottom` bisa keluar layar → handle **tidak bisa di-grab** untuk mengecilkan tinggi; satu-satunya jalan adalah **drag window ke atas via `.compact-bar`** untuk memunculkan kembali strip, baru resize (EC18). Ini bukan sekadar "tidak merusak state" — ini keterbatasan operasional yang diterima; (d) konsisten dengan filosofi ephemeral. **Documented limitation ini eksplisit** (Requirement 7).
- [x] **Q5: Apakah `lib.rs` perlu diubah?** — **Keputusan: TIDAK.** Verified dari kode: `resize_window` (lib.rs:76-109) sudah generic `width`+`height`; branch `preserve_center_x == Some(true)` (lib.rs:85-104) — satu-satunya jalur menyentuh posisi — **tidak dieksekusi** untuk height resize karena JS memanggil tanpa flag (default `false`); branch `else` (lib.rs:105-107) = `set_size` only → x & y otomatis dipertahankan. `generate_handler!` & `tauri.conf.json` juga tidak berubah. Berbeda dengan width-resize yang memang membutuhkan param baru `preserve_center_x`.
- [x] **Q6: Apakah `.agent-list` perlu menyesuaikan untuk tinggi ekstra?** — **Keputusan: Ya, minimal.** Ganti `max-height: 140px` (App.svelte:499) → `flex: 1 1 auto; min-height: 0` (pertahankan `overflow-y: auto`). Tanpa ini, resize tinggi hanya menambah ruang kosong mati di antara list & footer → fitur tidak memberikan nilai (list selalu 140px). Dengan perubahan ini list tumbuh mengisi tinggi baru, footer tetap di bawah, scroll internal untuk banyak agent (EC13). **Trade-off yang diterima (keputusan Q3):** dengan 1–2 agent, `.agent-list` meregang penuh → void kosong antara item terakhir & footer (paling terlihat di `MAX_HEIGHT` 520); alternatif `max-height` dinamis ditolak karena menghidupkan kembali ruang kosong mati di bawah list dan menge-cap nilai fitur (R6/AC11/EC17).
- [x] **Q7: Penempatan handle vs `.panel-footer`?** — **Keputusan:** strip 10px absolute di `bottom: 0` `.expanded-panel` — **di bawah** footer, menempati padding-bottom 12px panel (App.svelte:436). Tombol "Open Terminal" (App.svelte:304-306) berada di atas strip (footer berakhir ±12px dari bottom) → **tidak tertutup**, tetap bisa diklik (AC8). Disambiguation terhadap `.compact-bar` (drag-window/toggle) secara struktural mustahil karena handle **di luar** `.compact-bar`; pola `preventDefault`+`stopPropagation`+`data-no-drag` tetap dipasang defensif untuk konsistensi (Requirement 5).

---

## 8. Acceptance Criteria

> Semua kriteria harus measurable & bisa diverifikasi secara objektif.

- [ ] **AC1 (Requirement 1):** Handle `.resize-handle--bottom` **hanya tampil & aktif saat expanded**: saat collapsed tidak tampil (`display: none` via `:not(.visible)`) dan `startResizeHeight` early-return (`!isLocked || !showPanel`) — verified via klik/drag pada posisi strip saat collapsed → tidak ada perubahan ukuran window.
- [ ] **AC2:** Saat expanded, drag tepi bawah ke bawah → tinggi window bertambah kontinyu mengikuti kursor; drag ke atas → berkurang; ter-clamp di `[minH, MAX_HEIGHT]` (window berhenti berubah saat clamp tercapai walau kursor lanjut, tanpa invoke berulang).
- [ ] **AC3 (Requirement 4):** Posisi **x dan y** window TIDAK berubah selama resize height — nilai `outer_position.x` & `outer_position.y` identik sebelum/sesudah; **lebar** window juga tidak berubah (nilai `outer_size.width` identik).
- [ ] **AC4 (Requirement 6):** Setelah resize height → collapse → expand lagi → panel memakai **tinggi efektif terakhir** yang di-resize user (bukan `DEFAULT_HEIGHT`); repeatable (resize lagi → collapse → expand tetap memakai nilai terbaru).
- [ ] **AC5:** Pada display scaling 125%/150% (bila tersedia), tinggi mengikuti kursor secara proporsional (delta sudah dikonversi `devicePixelRatio`).
- [ ] **AC6 (Requirement 3):** Min efektif selalu > tinggi bar collapsed: saat `expandedHeight` mencapai min, tinggi window ≥ `Math.max(Math.ceil(MIN_HEIGHT * devicePixelRatio), Math.ceil((collapsedHeight() + 40) * devicePixelRatio))` (physical px); bar collapsed **tidak pernah lebih tinggi** dari panel expanded (ukur `collapsedHeight()` dan tinggi min yang tercapai).
- [ ] **AC7:** Pada tinggi minimum (`minH` = `⌈180 × devicePixelRatio⌉` physical px, dihitung dari floor CSS 180), panel tetap usable **tanpa vertical overflow di semua DPR yang didukung** (100%/125%/150%): `.panel-header`, `.agent-list`, dan `.panel-footer` semua tampil; `.agent-list` `overflow-y: auto` berfungsi (banyak agent → scroll internal); `scrollHeight` list tidak memaksa window melebihi ukuran. (Formula Q1 menjamin geometri CSS minimum selalu sama di DPR apa pun — footer tidak pernah ter-clip.)
- [ ] **AC8 (Requirement 1):** Tombol "Open Terminal" di `.panel-footer` tetap bisa diklik setelah fitur ini (terminal terbuka); handle strip tidak menghalangi — verified klik tombol saat window di berbagai tinggi.
- [ ] **AC9 (Requirement 1):** Klik item agent tetap memicu `openFolder`; scroll `.agent-list` normal; drag-window dari `.compact-bar` dan klik-toggle expand/collapse tidak terpengaruh; drag pada handle height **tidak pernah** memicu interaksi lain (dan sebaliknya).
- [ ] **AC10 (Requirement 5):** Selama drag resize cepat (mouse rate tinggi), invoke ke Rust tidak menumpuk — **maksimal 1 invoke per `requestAnimationFrame`** (verified via instrumentasi/`console.count` pada `applyPendingHeight`); nilai tinggi final setelah release **selalu sama** dengan posisi kursor terakhir (flush di `endResizeHeight`).
- [ ] **AC11:** Animasi collapse/expand tetap smooth dan berhenti di target `expandedHeight` yang benar; guard `generation` existing tidak regresi; resize height yang berlangsung saat window kehilangan fokus/alt-tab dibersihkan (`lostpointercapture`/`pointercancel` → `isResizing` reset). **Regresi kosmetik yang diterima (keputusan Q3):** dengan 1–2 agent, `.agent-list` meregang penuh dan muncul void kosong antara item terakhir & footer (paling terlihat di `MAX_HEIGHT` 520) — diterima sebagai konsekuensi inherent panel yang bisa di-resize; tidak ada elemen yang ter-overlap atau tidak bisa diklik (R6/EC17).
- [ ] **AC12:** `cargo check` + `cargo build` (src-tauri) dan `npm run build` sukses tanpa error, **tanpa dependency baru**, **tanpa perubahan `lib.rs`**, `generate_handler!`, maupun `tauri.conf.json` (verified via `git diff` — hanya `App.svelte` yang berubah).
- [ ] **AC13 (Requirement 8):** Tidak ada persistensi tinggi antar sesi (restart aplikasi → `DEFAULT_HEIGHT = 260`); tidak ada native resize border (`resizable` tetap `false`); tidak ada corner handle 2D.

---

## 9. Referensi

- [App.svelte — `EXPANDED_HEIGHT` (:12, :54, :130), `currentHeight` (:54), `currentWidth` (:55), `clamp` (:57-59), `resizeWindow` (:61-72), `collapsedHeight` (:74-77), `animateResize` (:79-95), `expand`/`collapse` (:126-147), `toggleLock` (:149-153), `isResizing` (:156), width-resize handlers (:158-207), `.expanded-panel` (:272, CSS :431-448), `.panel-footer` (:303-307, CSS :598-603), `.agent-list` CSS (:495-501)](file:///D:/dev/experiment-poni-agent/agent-monitor/src/App.svelte)
- [lib.rs — `resize_window` (:76-109), `generate_handler!` (:183-192), setup top-anchored y=0 (:152-160)](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/src/lib.rs)
- [bar-width-resize.md — pola existing: rAF throttle + flush, disambiguation 3 lapis, `preserve_center_x`](file:///D:/dev/experiment-poni-agent/planning/bar-width-resize.md)
- [tauri.conf.json — window overlay (baris 12-24)](file:///D:/dev/experiment-poni-agent/agent-monitor/src-tauri/tauri.conf.json)
- [MDN — Pointer Events & `setPointerCapture`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)
- [Svelte — event modifiers (`stopPropagation`, `preventDefault`)](https://svelte.dev/docs/element-directives#on-eventname)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `2026-08-01` | Planner | Initial draft |
| `1.1.0` | `2026-08-01` | Planner | Revisi menjawab Q1–Q5 reviewer: (Q1) `MIN_HEIGHT` dijadikan floor **CSS px**, `minH` dihitung dalam **physical px** (`max(⌈180×DPR⌉, ⌈(collapsedHeight()+40)×DPR⌉)`) — AC7/AC8 berlaku di 125%/150%; (Q2) seluruh perbandingan/penetapan tinggi dikonversi ke physical px, bug DPI pre-existing di `collapse()`/`onMount` diperbaiki; (Q3) void kosong list saat 1–2 agent diterima & dicatat di R6/AC11/EC17; (Q4) konsekuensi praktis handle di belakang taskbar ditulis eksplisit di §7 Q4 & EC18; (Q5) koreksi klaim "pola identik width-resize" — guard `isResizing` & pembanding `expandedHeight` vs `currentWidth` dijustifikasi di §3 |
| `1.2.0` | `2026-08-01` | Planner | Revisi menjawab 3 koreksi reviewer putaran kedua: (KQ1) §3 koreksi klaim state machine — di fase expanded penuh `isLocked` SELALU true (`expand()` men-set `isLocked = true`, App.svelte:129), sehingga klausa `!isLocked` pada guard tidak pernah aktif; guard aktif murni karena `showPanel = true`; (KQ2) koreksi aritmetika dominasi term `minH` — term2 didominasi term1 saat `bar+40 ≤ 180` yaitu **bar ≤ 140 CSS** (bukan 132), diperbaiki di §3 & §7 Q1; budget §1 diselaraskan (chrome header ~29 + gap ~16 + footer ~36 + padding 12 ≈ 90 + bar ~42 + list ±48 ≈ 180, tanpa double-count padding); (KQ3) shorthand flex `.agent-list` diseragamkan ke `flex: 1 1 auto; min-height: 0` di semua section (sebelumnya `flex: 1` di §3/§4 EC13/§7 Q2) |
