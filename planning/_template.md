# Planning Template

> **Petunjuk:** Salin file ini untuk setiap fitur baru. Isi semua section dengan `N/A` jika tidak relevan (sertakan alasan).

---

## Metadata

| Field    | Value |
|----------|-------|
| Status   | `Draft` / `Reviewed` / `Final` |
| Versi    | `1.0.0` |
| Tanggal  | `YYYY-MM-DD` |
| Author   | `Nama` |
| Reviewer | `Nama` |

---

## 1. Tujuan

<!-- Deskripsi singkat dan jelas tentang apa yang ingin dicapai. Harus spesifik & measurable. -->

**Contoh:** Memberikan kemampuan autentikasi pengguna via email/password dan Google OAuth.

---

## 2. Scope

### In Scope

- [ ] List fitur yang termasuk
- [ ] Contoh: Registrasi pengguna baru
- [ ] Contoh: Login dengan email & password
- [ ] Contoh: Login dengan Google OAuth

### Out of Scope

- [ ] List fitur yang TIDAK termasuk
- [ ] Contoh: Reset password (akan dikerjakan di fase 2)
- [ ] Contoh: Two-factor authentication

---

## 3. Pendekatan

### Strategi Terpilih

<!-- Penjelasan teknis bagaimana fitur akan diimplementasikan. Sertakan alur data, arsitektur, komponen, dll. -->

**Contoh:**
- Backend: Gunakan NextAuth.js di API route `/api/auth/[...nextauth]`
- Database: Simpan user di tabel `User` Prisma dengan field `email`, `passwordHash`, `provider`
- Frontend: Halaman `/login` dan `/register` sebagai client component
- Flow: Register → hash password (bcrypt) → simpan → redirect ke dashboard

### Alternatif yang Dipertimbangkan

| Alternatif | Alasan Tidak Dipilih |
|------------|----------------------|
| Lucia Auth | Masih terlalu baru, dokumentasi kurang mature |
| Supabase Auth | Menambah dependency eksternal untuk fitur sederhana |

---

## 4. Risiko & Edge Case

### Tabel Risiko

| Risiko | Probabilitas | Dampak | Mitigasi |
|--------|-------------|--------|----------|
| C1: Email sudah terdaftar | Medium | Rendah | Validasi di server, return error 409 |
| C2: Password lemah | Medium | Sedang | Validasi minimal 8 karakter + kombinasi huruf/angka |
| C3: Token JWT expired | Rendah | Rendah | Auto-refresh token, redirect ke login |

### Edge Case

- [ ] User mencoba register dengan email yang sudah terdaftar
- [ ] User mencoba login dengan password salah > 5 kali → lockout sementara
- [ ] User menekan tombol submit ganda → disabled state + loading spinner
- [ ] Session expired saat user sedang mengisi form → simpan draft sebelum redirect

---

## 5. Dependency

### Library

| Library | Versi | Tujuan |
|---------|-------|--------|
| NextAuth.js | ^4.24 | Autentikasi |
| bcryptjs | ^2.4 | Hash password |
| Prisma | ^5.0 | ORM database |

### Service

| Service | Tujuan |
|---------|--------|
| Google Cloud Console | Mendapatkan Client ID & Secret untuk OAuth |
| Neon / PostgreSQL | Database production |

### Internal

| Dependency | Tujuan |
|------------|--------|
| Modul `lib/prisma.ts` | Koneksi database (existing) |
| Layout `app/layout.tsx` | Provider session wrapping |

---

## 6. Task Breakdown

> **Effort estimasi:** S = < 1 jam, M = 1–3 jam, L = 3–8 jam, XL = > 8 jam

- [ ] **Setup dependency** — install next-auth, bcryptjs [S]
- [ ] **Buat Prisma schema** — model User dengan field email, passwordHash, provider, image [M]
- [ ] **Buat API route auth** — `pages/api/auth/[...nextauth].ts` [M]
- [ ] **Buat halaman Register** — form email + password + validasi client [M]
- [ ] **Buat halaman Login** — form email + password + link Google OAuth [M]
- [ ] **Buat middleware** — proteksi route `/dashboard` dan `/profile` [M]
- [ ] **Testing** — unit test & integration test [L]

**Dependency antar-task:**
- Task 1 → Task 2 → Task 3
- Task 4 & 5 bisa paralel setelah Task 3

---

## 7. Open Questions

<!-- Pertanyaan yang belum terjawab. Harus kosong sebelum implementasi dimulai. -->

- [ ] Q1: Apakah perlu role-based access control (admin/user) dari awal?
- [ ] Q2: Bagaimana handle refresh token — pakai JWT refresh atau session-based?

---

## 8. Acceptance Criteria

> Semua kriteria harus measurable & bisa diverifikasi secara objektif.

- [ ] User bisa register dengan email & password yang valid
- [ ] User bisa login dengan email & password yang benar
- [ ] User bisa login dengan Google OAuth
- [ ] User mendapat pesan error jika email sudah terdaftar
- [ ] User mendapat pesan error jika password salah
- [ ] User di-redirect ke halaman `/login` jika belum login dan mengakses route terproteksi
- [ ] Semua test passing (minimal coverage 80%)

---

## 9. Referensi

- [NextAuth.js Documentation](https://next-auth.js.org/)
- [Prisma Authentication Example](https://www.prisma.io/docs/guides/authentication)
- [bcryptjs NPM](https://www.npmjs.com/package/bcryptjs)

---

## Revisi History

| Versi   | Tanggal     | Author | Perubahan |
|---------|-------------|--------|-----------|
| `1.0.0` | `YYYY-MM-DD` | `Nama` | Initial draft |
