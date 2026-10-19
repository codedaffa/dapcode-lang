/* ===========================================================================
 * DapCode Direct GPU Canvas Engine (Flutter-style Architecture)
 * Antarmuka Render Grafis 2D Terakselerasi GPU untuk Android NDK, iOS & Desktop
 * =========================================================================== */

#ifndef DAPCODE_ENGINE_H
#define DAPCODE_ENGINE_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef f64
typedef double f64;
#endif

/* Status Kanvas & Input Pengguna */
extern f64 dap_kanvas_width;
extern f64 dap_kanvas_height;
extern f64 dap_kanvas_mouse_pos_x;
extern f64 dap_kanvas_mouse_pos_y;
extern uint64_t dap_kanvas_mouse_is_down;
extern uint64_t dap_kanvas_last_key;
extern f64 dap_kanvas_dt;

/* Fungsi Primitif Kanvas DapCode (Direct GPU Rendering) */
void dap_kanvas_bersihkan(f64 r, f64 g, f64 b, f64 a);
void dap_kanvas_warna_isi(f64 r, f64 g, f64 b, f64 a);
void dap_kanvas_warna_garis(f64 r, f64 g, f64 b, f64 a);
void dap_kanvas_tebal_garis(f64 tebal);
void dap_kanvas_persegi(f64 x, f64 y, f64 lebar, f64 tinggi);
void dap_kanvas_bingkai_persegi(f64 x, f64 y, f64 lebar, f64 tinggi);
void dap_kanvas_lingkaran(f64 x, f64 y, f64 radius);
void dap_kanvas_bingkai_lingkaran(f64 x, f64 y, f64 radius);
void dap_kanvas_garis(f64 x1, f64 y1, f64 x2, f64 y2);
void dap_kanvas_titik(f64 x, f64 y, f64 ukuran);
void dap_kanvas_teks(const char* teks, f64 x, f64 y, f64 ukuran);
f64 dap_kanvas_lebar(void);
f64 dap_kanvas_tinggi(void);
f64 dap_kanvas_mouse_x(void);
f64 dap_kanvas_mouse_y(void);
uint64_t dap_kanvas_mouse_klik(void);
uint64_t dap_kanvas_tombol_ditekan(uint64_t kode);
f64 dap_kanvas_delta_waktu(void);
void dap_kanvas_simpan_teks(const char* kunci, const char* nilai);
const char* dap_kanvas_ambil_teks(const char* kunci);
void dap_kanvas_simpan_angka(const char* kunci, f64 nilai);
f64 dap_kanvas_ambil_angka(const char* kunci);
void dap_kanvas_simpan_u64(const char* kunci, uint64_t nilai);
uint64_t dap_kanvas_ambil_u64(const char* kunci);

/* Siklus Hidup Mesin Grafis GPU Langsung */
void dap_engine_init(int width, int height);
void dap_engine_resize(int width, int height);
void dap_engine_step(float dt);
void dap_engine_on_touch(float x, float y, int action);
void dap_engine_on_key(int key_code, int is_down);
void dap_engine_destroy(void);

#ifdef __cplusplus
}
#endif

#endif /* DAPCODE_ENGINE_H */
