# Multi-Agent Workflow Setup

## Agents

| Agent | Role | Cara Invoke |
|-------|------|-------------|
| `@requester` | Menerima request, orchestrate workflow planning | Default (Tab to switch from primary) |
| `@planner` | Membuat & merevisi planning fitur dalam file `.md` | `@planner` mention |
| `@reviewer` | Mengajukan pertanyaan kritis pada planning, minta revisi planner | `@reviewer` mention |
| `@implementator` | Implementasi kode dari planning yang sudah final & ter-review | `@implementator` mention |

## Workflow (Discussion Loop)

Planner dan reviewer berdiskusi hingga planning dianggap sempurna.

```
User ──▶ Requester
              │
              ▼
        Planner ──▶ writes plan.md
              │
              ▼
        Reviewer ──▶ reads plan.md, ajukan pertanyaan/keberatan
              │
         ┌──────┴──────┐
       SUDAH            MASIH ADA
      SEMPURNA          KELEMAHAN
         │                 │
         │                 ▼
         │          Planner (revisi plan.md)
         │                 │
         │                 ▼
         │          Reviewer (re-review)  ◀── loop
         │
         ▼
       User ◀── planning final
              │
              ▼
        Implementator (implementasi dari plan.md)
              │
              ▼
        User ◀── hasil implementasi
```

### Langkah Workflow

1. **Requester** menerima task fitur dari user
2. **Requester** invoke `@planner` untuk membuat file `plan.md` di `./planning/`
3. **Requester** invoke `@reviewer` untuk mengkritis planning
4. **Reviewer** mengajukan pertanyaan kritis:
   - Jika **sempurna** → kembalikan ke requester, planning final
   - Jika **ada kelemahan** → invoke `@planner` untuk revisi → loop kembali ke reviewer
5. Setelah planning final → **Requester** invoke `@implementator` untuk implementasi
6. **Implementator** mengimplementasikan sesuai `plan.md` yang sudah final, lalu report ke user via requester

### Aturan Diskusi Planner ↔ Reviewer

- **Reviewer wajib melakukan review sangat detail** — bukan sekadar "OK / tidak OK". Setiap putaran, reviewer **wajib membaca seluruh isi `plan.md` baris demi baris** dan mengaudit setiap section terhadap checklist di bawah.
- **Reviewer wajib mengajukan minimal 3 pertanyaan kritis** setiap putaran. Semua pertanyaan **wajib menyertakan referensi eksplisit** ke bagian plan.md (nomor section atau kutipan baris), contoh: *"§4 Risiko: bagaimana handle X jika Y?"*.
- Jika masih ada kelemahan / hal yang belum jelas / ambiguitas / asumsi terselubup → **reviewer invoke `@planner`** untuk revisi `plan.md` dengan daftar pertanyaan eksplisit.
- **Reviewer hanya boleh menyatakan "sempurna"** jika SEMUA kriteria terpenuhi:
  - Semua section template terisi (atau `N/A` beralasan)
  - Tidak ada istilah ambigu yang belum didefinisikan
  - Setiap asumsi eksplisit ditulis, bukan tersirat
  - Setiap edge case yang masuk akal terdaftar di §4
  - Task breakdown granular & dapat dieksekusi (bukan terlalu abstrak)
  - Dependency lengkap dengan versi & tujuan
  - Acceptance criteria measurable & testable
  - Semua Open Questions terjawab / di-resolve
- Jika reviewer puas → kembalikan ke requester dengan ringkasan alasan keputusan (section apa saja yang sudah divalidasi).
- Maksimal 5 putaran untuk mencegah loop tak terbatas. Setelah itu, lanjut dengan catatan "toleransi" beserta daftar kelemahan tersisa.

### Checklist Review Detail (wajib diaudit per putaran)

| # | Aspek | Yang Dicek Reviewer |
|---|-------|---------------------|
| 1 | Tujuan | Apakah spesifik & measurable? Bukan kalimat kabur? |
| 2 | Scope | Boundary In/Out jelas? Tidak ada fitur agar-agar? |
| 3 | Pendekatan | Alternatif benar-benar dipertimbangkan atau formalitas? Ada trade-off eksplisit? |
| 4 | Risiko | Setiap risiko punya probabilitas + dampak + mitigasi konkret? Edge case di-list, bukan umum? |
| 5 | Dependency | Versi tertera? Lisensi cocok? Service punya fallback? |
| 6 | Task Breakdown | Bisa dieksekusi langsung? Estimasi effort? Dependency antar-task jelas? |
| 7 | Open Questions | Masih ada pertanyaan menggantung? Harus kosong untuk final. |
| 8 | Acceptance Criteria | Testable & measurable? Bisa di-verify objektif? |
| 9 | Konsistensi | Istilah konsisten? Tidak kontradiksi antar section? |
| 10 | Asumsi | Semua asumsi eksplisit tertulis? Tidak ada asumsi terselubung? |
| 11 | Kelengkapan | Tidak ada bagian kosong / TODO / placeholder? |
| 12 | Realistis | Apakah plan ini benar-benar bisa dijalankan dengan resource yang ada? |

## Konvensi File Planning

- Disimpan di `./planning/{feature-name}.md` (gunakan kebabab-case, contoh: `user-auth.md`)
- **Wajib** menggunakan template `./planning/_template.md` sebagai base — salin lalu isi.
- Struktur file wajib konsisten (section urut):
  1. Header metadata (Status, Versi, Tanggal, Author, Reviewer)
  2. `## 1. Tujuan`
  3. `## 2. Scope` (In Scope + Out of Scope)
  4. `## 3. Pendekatan` (Strategi Terpilih + Alternatif)
  5. `## 4. Risiko & Edge Case` (tabel + edge case)
  6. `## 5. Dependency` (Library, Service, Internal)
  7. `## 6. Task Breakdown` (checklist)
  8. `## 7. Open Questions`
  9. `## 8. Acceptance Criteria`
  10. `## 9. Referensi`
  11. `## Revisi History` (tabel log perubahan)
- Aturan:
  - Jangan hapus section yang ada; jika tidak relevan tulis `N/A` + alasan singkat.
  - Update `Status`, `Versi`, dan tambah baris di `Revisi History` setiap revisi.
  - Gunakan checklist `- [ ]` untuk task & acceptance criteria.
  - Format tanggal `YYYY-MM-DD`, versi semver (`1.0.0` → naik minor setiap revisi review).

## Session Navigation

- `Tab` - switch antara primary agents (requester)
- `@mention` - invoke subagent
- `<Leader>+Down` - masuk ke child session
- `Up` - kembali ke parent session
- `Right/Left` - cycle antar child sessions