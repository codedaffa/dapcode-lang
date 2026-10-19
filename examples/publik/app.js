document.addEventListener("DOMContentLoaded", () => {
    const statusBox = document.getElementById("status-box");
    const btnPing = document.getElementById("btn-ping");
    const btnStatis = document.getElementById("btn-statis");

    function updateTerminal(msg) {
        statusBox.textContent = `[${new Date().toLocaleTimeString()}] ${msg}`;
    }

    async function checkApiStatus() {
        updateTerminal("Menghubungi /api/status ...");
        try {
            const resp = await fetch("/api/status");
            const data = await resp.json();
            updateTerminal(`Sukses! Respons server:\n${JSON.stringify(data, null, 2)}`);
        } catch (err) {
            updateTerminal(`Gagal menghubungi API: ${err.message}`);
        }
    }

    btnPing.addEventListener("click", checkApiStatus);

    btnStatis.addEventListener("click", async () => {
        updateTerminal("Memeriksa header /style.css ...");
        try {
            const resp = await fetch("/style.css");
            const contentType = resp.headers.get("content-type");
            updateTerminal(`Aset statis /style.css dimuat!\nStatus: ${resp.status} OK\nContent-Type: ${contentType}`);
        } catch (err) {
            updateTerminal(`Gagal memuat berkas statis: ${err.message}`);
        }
    });

    // Auto-ping on initial load
    checkApiStatus();

    // ==========================================
    // ⚡ WebAssembly (WASM 1.0 MVP) Client Integration
    // ==========================================
    const wasmBadge = document.getElementById("wasm-badge");
    const wasmStatusText = document.getElementById("wasm-status-text");
    const wasmBox = document.getElementById("wasm-box");
    const btnWasmFaktorial = document.getElementById("btn-wasm-faktorial");
    const btnWasmLingkaran = document.getElementById("btn-wasm-lingkaran");
    const btnWasmDeret = document.getElementById("btn-wasm-deret");

    let wasmInstance = null;
    let wasmMemory = null;

    function appendWasmLog(msg) {
        const time = new Date().toLocaleTimeString();
        const line = `[${time}] ${msg}`;
        if (wasmBox.textContent.includes("Menunggu inisialisasi")) {
            wasmBox.textContent = line;
        } else {
            wasmBox.textContent += `\n${line}`;
        }
        wasmBox.scrollTop = wasmBox.scrollHeight;
    }

    async function initWebAssembly() {
        try {
            appendWasmLog("Mengunduh dan memuat /uji_wasm.wasm ...");
            const resp = await fetch("/uji_wasm.wasm");
            if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
            const bytes = await resp.arrayBuffer();

            const importObject = {
                env: {
                    cetak_u64: (val) => appendWasmLog(`[WASM cetak] ${val.toString()}`),
                    cetak_f64: (val) => appendWasmLog(`[WASM cetak] ${val.toString()}`),
                    cetak_teks: (ptr, len) => {
                        const arr = new Uint8Array(wasmMemory.buffer, ptr, len);
                        appendWasmLog(`[WASM cetak] ${new TextDecoder().decode(arr)}`);
                    },
                    sin: Math.sin,
                    cos: Math.cos,
                    tan: Math.tan,
                    asin: Math.asin,
                    acos: Math.acos,
                    atan: Math.atan,
                    atan2: Math.atan2,
                    akar_kubik: Math.cbrt,
                    pangkat: Math.pow,
                    log: Math.log,
                    log10: Math.log10,
                    log2: Math.log2,
                    exp: Math.exp,
                    tanda: Math.sign,
                }
            };

            const { instance } = await WebAssembly.instantiate(bytes, importObject);
            wasmInstance = instance;
            wasmMemory = instance.exports.memory;

            if (wasmBadge) wasmBadge.classList.add("connected");
            if (wasmStatusText) wasmStatusText.textContent = "WASM Siap (Aktif)";
            appendWasmLog("Modul WebAssembly berhasil dimuat dan siap dieksekusi!");

            const exports = Object.keys(instance.exports).filter(k => k !== "memory");
            appendWasmLog(`Fungsi WASM terekspor: ${exports.join(", ")}`);
        } catch (err) {
            if (wasmStatusText) wasmStatusText.textContent = "WASM Gagal Dimuat";
            appendWasmLog(`Gagal memuat WebAssembly: ${err.message}`);
        }
    }

    if (btnWasmFaktorial) {
        btnWasmFaktorial.addEventListener("click", () => {
            if (!wasmInstance || !wasmInstance.exports.faktorial) {
                appendWasmLog("WASM belum siap!");
                return;
            }
            const start = performance.now();
            const hasil = wasmInstance.exports.faktorial(8n);
            const duration = (performance.now() - start).toFixed(4);
            appendWasmLog(`Hasil faktorial(8n) = ${hasil.toString()} (selesai dalam ${duration} ms)`);
        });
    }

    if (btnWasmLingkaran) {
        btnWasmLingkaran.addEventListener("click", () => {
            if (!wasmInstance || !wasmInstance.exports.luas_lingkaran) {
                appendWasmLog("WASM belum siap!");
                return;
            }
            const start = performance.now();
            const hasil = wasmInstance.exports.luas_lingkaran(14.0);
            const duration = (performance.now() - start).toFixed(4);
            appendWasmLog(`Hasil luas_lingkaran(14.0) = ${hasil.toFixed(4)} (selesai dalam ${duration} ms)`);
        });
    }

    if (btnWasmDeret) {
        btnWasmDeret.addEventListener("click", () => {
            if (!wasmInstance || !wasmInstance.exports.hitung_deret) {
                appendWasmLog("WASM belum siap!");
                return;
            }
            const start = performance.now();
            const hasil = wasmInstance.exports.hitung_deret(1000n);
            const duration = (performance.now() - start).toFixed(4);
            appendWasmLog(`Hasil hitung_deret(1000n) = ${hasil.toString()} (selesai dalam ${duration} ms)`);
        });
    }

    initWebAssembly();

    // ==========================================
    // 🎨 Web Playground & Interactive Canvas 2D
    // ==========================================
    const canvas = document.getElementById("dap-canvas");
    const ctx = canvas ? canvas.getContext("2d") : null;
    const canvasBadge = document.getElementById("canvas-badge");
    const canvasStatusText = document.getElementById("canvas-status-text");
    const canvasCompileTime = document.getElementById("canvas-compile-time");
    const canvasWasmSize = document.getElementById("canvas-wasm-size");
    const canvasRenderTime = document.getElementById("canvas-render-time");
    const canvasCmdCount = document.getElementById("canvas-cmd-count");
    const coordsOverlay = document.getElementById("coords-overlay");
    const btnCompileRun = document.getElementById("btn-compile-run");
    const btnResetCode = document.getElementById("btn-reset-code");
    const btnAnimasiKanvas = document.getElementById("btn-animasi-kanvas");
    const btnBersihkanKanvas = document.getElementById("btn-bersihkan-kanvas");
    const presetSelector = document.getElementById("preset-selector");
    const codeEditor = document.getElementById("code-editor");
    const lineNumbers = document.getElementById("line-numbers");
    const editorLineCol = document.getElementById("editor-line-col");
    const compileStatusDot = document.getElementById("compile-status-dot");
    const compileStatusMsg = document.getElementById("compile-status-msg");
    const currentFilename = document.getElementById("current-filename");
    const framePresetSelector = document.getElementById("frame-preset-selector");
    const frameDimBadge = document.getElementById("frame-dim-badge");
    const btnFlipOrientation = document.getElementById("btn-flip-orientation");
    const btnToggleBezel = document.getElementById("btn-toggle-bezel");
    const btnToggleWide = document.getElementById("btn-toggle-wide");
    const customResRow = document.getElementById("custom-res-row");
    const customWidth = document.getElementById("custom-width");
    const customHeight = document.getElementById("custom-height");
    const btnApplyCustom = document.getElementById("btn-apply-custom");
    const deviceFrameMockup = document.getElementById("device-frame-mockup");
    const canvasViewport = document.getElementById("canvas-viewport");
    const canvasWatermark = document.getElementById("canvas-watermark");
    const playgroundContainer = document.querySelector(".container");

    let canvasWasmInstance = null;
    let canvasWasmMemory = null;
    let totalCanvasCommands = 0;
    let isAnimating = false;
    let animationReqId = null;
    let currentDeltaTime = 0.0166;
    let lastAnimFrameTime = performance.now();

    // Koleksi Preset Kode Contoh Interaktif (Dinamis & Responsif terhadap Ukuran Frame)
    const KODE_PRESET = {
        tata_surya: `// ========================================================
// 🌌 Demo: Tata Surya Orbit 2D via WebAssembly
// Tampilan Adaptif Dinamis: Skala proporsional ukuran frame
// ========================================================

fungsi gambar_tata_surya() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0

    // Hitung dimensi minimum frame untuk skala proporsional
    dim_min := w
    jika h < w {
        dim_min = h
    }
    skala := dim_min / 480.0
    jika skala < 0.55 {
        skala = 0.55
    }

    // 1. Bersihkan kanvas dengan warna latar kosmik
    kanvas_bersihkan(0.04, 0.06, 0.12, 1.0)

    // 2. Bintang acak terdistribusi proporsional dalam batas frame
    kanvas_warna_isi(0.8, 0.9, 1.0, 0.7)
    kanvas_titik(w * 0.15, h * 0.18, 2.5 * skala)
    kanvas_titik(w * 0.85, h * 0.22, 2.0 * skala)
    kanvas_titik(w * 0.20, h * 0.82, 3.0 * skala)
    kanvas_titik(w * 0.80, h * 0.78, 2.0 * skala)
    kanvas_titik(w * 0.50, h * 0.12, 2.0 * skala)
    kanvas_titik(w * 0.35, h * 0.65, 2.5 * skala)
    kanvas_titik(w * 0.90, h * 0.50, 2.0 * skala)

    // 3. Garis radar sumbu kartesius dibatasi batas aman frame
    panjang_x := (w / 2.0) - 24.0
    panjang_y := (h / 2.0) - 24.0
    kanvas_warna_garis(0.2, 0.35, 0.55, 0.3)
    kanvas_tebal_garis(1.0)
    kanvas_garis(cx - panjang_x, cy, cx + panjang_x, cy)
    kanvas_garis(cx, cy - panjang_y, cx, cy + panjang_y)

    // 4. Orbit sirkular konsentris proporsional (tidak pernah terpotong)
    r1 := 48.0 * skala
    r2 := 90.0 * skala
    r3 := 138.0 * skala

    kanvas_warna_garis(0.25, 0.5, 0.8, 0.35)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_lingkaran(cx, cy, r1)
    kanvas_bingkai_lingkaran(cx, cy, r2)
    kanvas_bingkai_lingkaran(cx, cy, r3)

    // 5. Bintang pusat (Matahari Inti)
    kanvas_warna_isi(1.0, 0.8, 0.2, 0.3)
    kanvas_lingkaran(cx, cy, 28.0 * skala)
    kanvas_warna_isi(1.0, 0.5, 0.0, 0.9)
    kanvas_lingkaran(cx, cy, 18.0 * skala)
    kanvas_warna_isi(1.0, 0.95, 0.4, 1.0)
    kanvas_lingkaran(cx, cy, 10.0 * skala)

    // 6. Planet 1 (Biru Cyan)
    p1_x := cx + r1 * cos(0.8)
    p1_y := cy + r1 * sin(0.8)
    kanvas_warna_isi(0.2, 0.85, 0.95, 1.0)
    kanvas_lingkaran(p1_x, p1_y, 6.0 * skala)

    // 7. Planet 2 (Hijau Emerald & Cincin)
    p2_x := cx + r2 * cos(2.3)
    p2_y := cy + r2 * sin(2.3)
    kanvas_warna_isi(0.2, 0.9, 0.5, 1.0)
    kanvas_lingkaran(p2_x, p2_y, 9.0 * skala)
    kanvas_warna_garis(0.4, 1.0, 0.7, 0.6)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_lingkaran(p2_x, p2_y, 14.0 * skala)

    // 8. Planet 3 (Ungu Violet)
    p3_x := cx + r3 * cos(4.2)
    p3_y := cy + r3 * sin(4.2)
    kanvas_warna_isi(0.7, 0.3, 0.95, 1.0)
    kanvas_lingkaran(p3_x, p3_y, 13.0 * skala)

    // 9. Kartu Status HUD Dinamis
    hud_w := 200.0 * skala
    jika hud_w > w * 0.45 {
        hud_w = w * 0.45
    }
    jika hud_w < 120.0 {
        hud_w = 120.0
    }
    hud_h := 54.0 * skala
    jika hud_h < 38.0 {
        hud_h = 38.0
    }

    kanvas_warna_isi(0.08, 0.12, 0.22, 0.85)
    kanvas_persegi(15.0, 15.0, hud_w, hud_h)
    kanvas_warna_garis(0.22, 0.65, 0.95, 0.75)
    kanvas_tebal_garis(1.2)
    kanvas_bingkai_persegi(15.0, 15.0, hud_w, hud_h)

    // 10. Teks HUD Informasi
    font_s1 := 12.0 * skala
    jika font_s1 < 9.0 { font_s1 = 9.0 }
    font_s2 := 10.0 * skala
    jika font_s2 < 8.0 { font_s2 = 8.0 }

    kanvas_warna_isi(0.95, 0.98, 1.0, 1.0)
    kanvas_teks("DapCode 2D Canvas", 22.0, 15.0 + (hud_h * 0.42), font_s1)
    kanvas_warna_isi(0.4, 0.85, 1.0, 0.9)
    kanvas_teks("Target: WebAssembly", 22.0, 15.0 + (hud_h * 0.80), font_s2)

    cetak("Tata Surya 2D dinamis berhasil dirender!")
}

fungsi utama() -> u64 {
    gambar_tata_surya()
    kembali 0
}`,
        spiral_archimedes: `// ========================================================
// 🌀 Demo: Pola Spiral Geometri Archimedes
// Tampilan Adaptif Dinamis: Menyesuaikan jari-jari frame
// ========================================================

fungsi gambar_spiral() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0

    dim_min := w
    jika h < w {
        dim_min = h
    }

    // 1. Bersihkan kanvas
    kanvas_bersihkan(0.02, 0.04, 0.08, 1.0)
    kanvas_tebal_garis(1.5)

    // 2. Skala radius spiral agar pas dengan frame tanpa terpotong
    max_radius := (dim_min / 2.0) - 35.0
    jika max_radius < 45.0 {
        max_radius = 45.0
    }
    b := max_radius / 26.0
    a := 2.0

    theta := 0.0
    selama theta < 25.0 {
        r := a + b * theta
        x := cx + r * cos(theta)
        y := cy + r * sin(theta)

        // Variasi warna gradasi berbasis sudut rotasi
        red := 0.5 + 0.5 * sin(theta * 0.5)
        green := 0.5 + 0.5 * cos(theta * 0.3)
        blue := 0.95

        dot_size := 3.0 + (theta * 0.12 * (dim_min / 480.0))
        kanvas_warna_isi(red, green, blue, 0.85)
        kanvas_lingkaran(x, y, dot_size)

        theta = theta + 0.18
    }

    // 3. Label HUD Dinamis
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.95)
    kanvas_teks("Pola Spiral Archimedes", 20.0, 35.0, 14.0)
    kanvas_warna_isi(0.4, 0.8, 1.0, 0.8)
    kanvas_teks("r = a + b * theta via WebAssembly", 20.0, 55.0, 11.0)

    cetak("Pola Spiral Archimedes dinamis berhasil dirender!")
}

fungsi utama() -> u64 {
    gambar_spiral()
    kembali 0
}`,
        partikel_pantul: `// ========================================================
// ⚡ Demo: Simulasi Partikel Pantul & Medan Energi 2D
// Tampilan Adaptif Dinamis: Arena & koordinat proporsional
// ========================================================

fungsi render_partikel() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()

    kanvas_bersihkan(0.05, 0.08, 0.14, 1.0)

    pad := 18.0
    arena_w := w - (pad * 2.0)
    arena_h := h - (pad * 2.0)

    dim_min := w
    jika h < w {
        dim_min = h
    }
    skala := dim_min / 480.0
    jika skala < 0.6 { skala = 0.6 }

    // 1. Bingkai pembatas arena fisika dinamis
    kanvas_warna_garis(0.2, 0.4, 0.7, 0.5)
    kanvas_tebal_garis(2.5)
    kanvas_bingkai_persegi(pad, pad, arena_w, arena_h)

    // 2. Partikel 1: Inti Plasma Biru (~24% X, ~35% Y)
    p1_x := pad + arena_w * 0.24
    p1_y := pad + arena_h * 0.35
    r1 := 26.0 * skala
    kanvas_warna_isi(0.1, 0.7, 1.0, 0.85)
    kanvas_lingkaran(p1_x, p1_y, r1)
    kanvas_warna_garis(0.4, 0.9, 1.0, 0.9)
    kanvas_tebal_garis(2.0)
    kanvas_bingkai_lingkaran(p1_x, p1_y, r1 + 6.0 * skala)

    // 3. Partikel 2: Bola Energi Neon Jingga (~74% X, ~58% Y)
    p2_x := pad + arena_w * 0.74
    p2_y := pad + arena_h * 0.58
    r2 := 32.0 * skala
    kanvas_warna_isi(1.0, 0.4, 0.1, 0.9)
    kanvas_lingkaran(p2_x, p2_y, r2)
    kanvas_warna_garis(1.0, 0.7, 0.2, 0.8)
    kanvas_bingkai_lingkaran(p2_x, p2_y, r2 + 8.0 * skala)

    // 4. Partikel 3: Satelit Hijau Lime (~48% X, ~80% Y)
    p3_x := pad + arena_w * 0.48
    p3_y := pad + arena_h * 0.80
    r3 := 18.0 * skala
    kanvas_warna_isi(0.2, 0.9, 0.4, 0.85)
    kanvas_lingkaran(p3_x, p3_y, r3)

    // 5. Garis gaya medan vektor energi antar-partikel
    kanvas_warna_garis(1.0, 0.9, 0.2, 0.6)
    kanvas_tebal_garis(1.5)
    kanvas_garis(p1_x, p1_y, p2_x, p2_y)
    kanvas_garis(p2_x, p2_y, p3_x, p3_y)

    // 6. Teks judul dinamis
    font_s := 13.0 * skala
    jika font_s < 10.0 { font_s = 10.0 }
    kanvas_warna_isi(0.9, 0.95, 1.0, 1.0)
    kanvas_teks("Simulasi Fisika Partikel 2D", pad + 15.0, pad + 24.0, font_s)
    kanvas_warna_isi(0.5, 0.7, 0.9, 0.8)
    kanvas_teks("Medan Vektor Energi Dinamis", pad + 15.0, pad + 42.0, font_s * 0.82)

    cetak("Partikel 2D dinamis berhasil disimulasikan!")
}

fungsi utama() -> u64 {
    render_partikel()
    kembali 0
}`,
        grafik_batang: `// ========================================================
// 📊 Demo: Diagram Batang Statistik Interaktif
// Tampilan Adaptif Dinamis: Sumbu, batang & label proporsional
// ========================================================

fungsi gambar_grafik() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()

    kanvas_bersihkan(0.04, 0.07, 0.12, 1.0)

    // Margin & sumbu kartesius dinamis proporsional
    margin_kiri := w * 0.12
    jika margin_kiri < 45.0 { margin_kiri = 45.0 }
    margin_kanan := w * 0.08
    jika margin_kanan < 25.0 { margin_kanan = 25.0 }

    sumbu_x_awal := margin_kiri
    sumbu_x_akhir := w - margin_kanan
    area_grafik_w := sumbu_x_akhir - sumbu_x_awal

    sumbu_y_bawah := h * 0.82
    sumbu_y_atas := h * 0.20
    tinggi_maks := sumbu_y_bawah - sumbu_y_atas

    dim_min := w
    jika h < w { dim_min = h }
    skala := dim_min / 480.0
    jika skala < 0.6 { skala = 0.6 }

    // 1. Garis bantu grid horizontal tipis (50%, 100%)
    kanvas_warna_garis(0.2, 0.3, 0.45, 0.3)
    kanvas_tebal_garis(1.0)
    kanvas_garis(sumbu_x_awal, sumbu_y_bawah - tinggi_maks * 0.5, sumbu_x_akhir, sumbu_y_bawah - tinggi_maks * 0.5)
    kanvas_garis(sumbu_x_awal, sumbu_y_bawah - tinggi_maks, sumbu_x_akhir, sumbu_y_bawah - tinggi_maks)

    // 2. Sumbu X dan Y utama
    kanvas_warna_garis(0.3, 0.45, 0.65, 0.7)
    kanvas_tebal_garis(2.0)
    kanvas_garis(sumbu_x_awal, sumbu_y_bawah, sumbu_x_akhir, sumbu_y_bawah)
    kanvas_garis(sumbu_x_awal, sumbu_y_atas - 10.0, sumbu_x_awal, sumbu_y_bawah)

    // 3. Batang Statistik Proporsional (4 Batang Terdistribusi)
    slot_w := area_grafik_w / 4.0
    bar_w := slot_w * 0.60
    jika bar_w > 90.0 { bar_w = 90.0 }
    pad_bar := (slot_w - bar_w) / 2.0

    font_persen := 11.0 * skala
    jika font_persen < 9.0 { font_persen = 9.0 }
    font_label := 10.0 * skala
    jika font_label < 8.0 { font_label = 8.0 }

    // Batang 1: DapCode Native (92%)
    b1_x := sumbu_x_awal + pad_bar
    b1_h := tinggi_maks * 0.92
    kanvas_warna_isi(0.2, 0.75, 1.0, 0.9)
    kanvas_persegi(b1_x, sumbu_y_bawah - b1_h, bar_w, b1_h)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("92%", b1_x + (bar_w * 0.15), sumbu_y_bawah - b1_h - 8.0, font_persen)
    kanvas_teks("DapCode", b1_x + (bar_w * 0.05), sumbu_y_bawah + 18.0, font_label)

    // Batang 2: WebAssembly (85%)
    b2_x := sumbu_x_awal + slot_w + pad_bar
    b2_h := tinggi_maks * 0.85
    kanvas_warna_isi(0.6, 0.3, 0.95, 0.9)
    kanvas_persegi(b2_x, sumbu_y_bawah - b2_h, bar_w, b2_h)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("85%", b2_x + (bar_w * 0.15), sumbu_y_bawah - b2_h - 8.0, font_persen)
    kanvas_teks("WASM", b2_x + (bar_w * 0.15), sumbu_y_bawah + 18.0, font_label)

    // Batang 3: C11 Native (96%)
    b3_x := sumbu_x_awal + (slot_w * 2.0) + pad_bar
    b3_h := tinggi_maks * 0.96
    kanvas_warna_isi(0.1, 0.85, 0.5, 0.9)
    kanvas_persegi(b3_x, sumbu_y_bawah - b3_h, bar_w, b3_h)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("96%", b3_x + (bar_w * 0.15), sumbu_y_bawah - b3_h - 8.0, font_persen)
    kanvas_teks("C11", b3_x + (bar_w * 0.2), sumbu_y_bawah + 18.0, font_label)

    // Batang 4: JIT Engine (74%)
    b4_x := sumbu_x_awal + (slot_w * 3.0) + pad_bar
    b4_h := tinggi_maks * 0.74
    kanvas_warna_isi(1.0, 0.65, 0.1, 0.9)
    kanvas_persegi(b4_x, sumbu_y_bawah - b4_h, bar_w, b4_h)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("74%", b4_x + (bar_w * 0.15), sumbu_y_bawah - b4_h - 8.0, font_persen)
    kanvas_teks("JIT", b4_x + (bar_w * 0.2), sumbu_y_bawah + 18.0, font_label)

    // 4. Judul Grafik Dinamis
    font_judul := 13.0 * skala
    jika font_judul < 11.0 { font_judul = 11.0 }
    kanvas_warna_isi(0.95, 0.98, 1.0, 1.0)
    kanvas_teks("Efisiensi & Performa Komputasi (%)", sumbu_x_awal, sumbu_y_atas - 22.0, font_judul)

    cetak("Grafik Batang Statistik dinamis berhasil digambar!")
}

fungsi utama() -> u64 {
    gambar_grafik()
    kembali 0
}`,
        game_pesawat: `// ========================================================
// 🎮 Demo Game: Pesawat Antariksa & Laser Interaktif 2D
// Tampilan Adaptif Dinamis: Batas terbang & laser sesuai frame
// Kontrol: Gerakkan Mouse / Sentuh Layar & Klik Tembak
// ========================================================

fungsi render_game() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()

    // 1. Latar antariksa dengan bintang proporsional batas w & h
    kanvas_bersihkan(0.02, 0.03, 0.07, 1.0)
    kanvas_warna_isi(0.8, 0.9, 1.0, 0.5)
    kanvas_titik(w * 0.10, h * 0.15, 2.0)
    kanvas_titik(w * 0.28, h * 0.25, 2.5)
    kanvas_titik(w * 0.58, h * 0.18, 1.5)
    kanvas_titik(w * 0.85, h * 0.30, 2.0)
    kanvas_titik(w * 0.18, h * 0.70, 2.0)
    kanvas_titik(w * 0.48, h * 0.88, 3.0)
    kanvas_titik(w * 0.75, h * 0.65, 1.5)
    kanvas_titik(w * 0.90, h * 0.85, 2.5)

    // 2. Batasi posisi pesawat agar tidak terpotong keluar frame (Clamping)
    px := mx
    py := my
    batas_x := 32.0
    batas_y := 24.0
    jika px < batas_x { px = batas_x }
    jika px > w - batas_x { px = w - batas_x }
    jika py < batas_y { py = batas_y }
    jika py > h - batas_y { py = h - batas_y }

    // 3. Render Pesawat Antariksa
    // Api Pendorong Reaktor (Thruster)
    kanvas_warna_isi(1.0, 0.45, 0.1, 0.9)
    kanvas_lingkaran(px - 22.0, py, 7.0)
    kanvas_warna_isi(1.0, 0.85, 0.2, 0.95)
    kanvas_lingkaran(px - 17.0, py, 4.0)

    // Lambung & Sayap
    kanvas_warna_isi(0.18, 0.45, 0.85, 1.0)
    kanvas_garis(px + 28.0, py, px - 18.0, py - 18.0)
    kanvas_garis(px - 18.0, py - 18.0, px - 10.0, py)
    kanvas_garis(px - 10.0, py, px - 18.0, py + 18.0)
    kanvas_garis(px - 18.0, py + 18.0, px + 28.0, py)

    kanvas_warna_isi(0.25, 0.65, 1.0, 0.9)
    kanvas_persegi(px - 10.0, py - 6.0, 22.0, 12.0)

    // Kokpit Kaca Neon
    kanvas_warna_isi(0.4, 0.95, 1.0, 0.95)
    kanvas_lingkaran(px + 6.0, py, 5.0)

    // 4. Laser Energi Ganda (Ditembakkan hingga ujung batas frame w)
    jika klik == 1 {
        kanvas_warna_garis(0.2, 1.0, 0.5, 0.95)
        kanvas_tebal_garis(3.0)
        kanvas_garis(px + 20.0, py - 12.0, w, py - 12.0)
        kanvas_garis(px + 20.0, py + 12.0, w, py + 12.0)

        kanvas_warna_isi(1.0, 1.0, 1.0, 0.9)
        kanvas_lingkaran(px + 22.0, py - 12.0, 4.0)
        kanvas_lingkaran(px + 22.0, py + 12.0, 4.0)
    }

    // 5. HUD Target Reticle di sekeliling posisi kursor
    kanvas_warna_garis(0.2, 0.8, 1.0, 0.5)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_lingkaran(px, py, 32.0)
    kanvas_garis(px - 40.0, py, px + 40.0, py)
    kanvas_garis(px, py - 40.0, px, py + 40.0)

    // 6. Status HUD Banner Adaptif
    dim_min := w
    jika h < w { dim_min = h }
    skala := dim_min / 480.0
    jika skala < 0.65 { skala = 0.65 }

    hud_w := 280.0 * skala
    jika hud_w > w - 24.0 { hud_w = w - 24.0 }
    hud_h := 65.0 * skala
    jika hud_h < 46.0 { hud_h = 46.0 }

    kanvas_warna_isi(0.06, 0.1, 0.18, 0.85)
    kanvas_persegi(12.0, 12.0, hud_w, hud_h)
    kanvas_warna_garis(0.3, 0.7, 1.0, 0.6)
    kanvas_tebal_garis(1.2)
    kanvas_bingkai_persegi(12.0, 12.0, hud_w, hud_h)

    font_h1 := 12.0 * skala
    jika font_h1 < 9.0 { font_h1 = 9.0 }
    font_h2 := 10.0 * skala
    jika font_h2 < 8.0 { font_h2 = 8.0 }

    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("🎮 SIMULATOR PESAWAT 2D", 20.0, 12.0 + hud_h * 0.35, font_h1)

    jika klik == 1 {
        kanvas_warna_isi(0.2, 1.0, 0.5, 1.0)
        kanvas_teks("STATUS: ⚡ LASER AKTIF!", 20.0, 12.0 + hud_h * 0.75, font_h2)
    } selain_itu {
        kanvas_warna_isi(0.4, 0.9, 1.0, 0.85)
        kanvas_teks("STATUS: Jelajah Orbit (Klik Tembak)", 20.0, 12.0 + hud_h * 0.75, font_h2)
    }
}

fungsi utama() -> u64 {
    render_game()
    kembali 1
}`
    };

    function updateLineNumbers() {
        if (!codeEditor || !lineNumbers) return;
        const lineCount = codeEditor.value.split("\n").length;
        let nums = "";
        for (let i = 1; i <= lineCount; i++) {
            nums += `${i}\n`;
        }
        lineNumbers.textContent = nums;
    }

    function updateCursorPos() {
        if (!codeEditor || !editorLineCol) return;
        const text = codeEditor.value.substr(0, codeEditor.selectionStart);
        const lines = text.split("\n");
        const line = lines.length;
        const col = lines[lines.length - 1].length + 1;
        editorLineCol.textContent = `Baris: ${line}, Kol: ${col}`;
    }

    if (codeEditor) {
        codeEditor.value = KODE_PRESET.tata_surya;
        updateLineNumbers();

        codeEditor.addEventListener("input", () => {
            updateLineNumbers();
            updateCursorPos();
        });

        codeEditor.addEventListener("scroll", () => {
            if (lineNumbers) lineNumbers.scrollTop = codeEditor.scrollTop;
        });

        codeEditor.addEventListener("click", updateCursorPos);
        codeEditor.addEventListener("keyup", updateCursorPos);

        // Tab key handling (4 spasi)
        codeEditor.addEventListener("keydown", (e) => {
            if (e.key === "Tab") {
                e.preventDefault();
                const start = codeEditor.selectionStart;
                const end = codeEditor.selectionEnd;
                codeEditor.value = codeEditor.value.substring(0, start) + "    " + codeEditor.value.substring(end);
                codeEditor.selectionStart = codeEditor.selectionEnd = start + 4;
                updateLineNumbers();
                updateCursorPos();
            } else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
                e.preventDefault();
                compileAndRun();
            }
        });
    }

    if (presetSelector) {
        presetSelector.addEventListener("change", (e) => {
            const key = e.target.value;
            if (KODE_PRESET[key] && codeEditor) {
                codeEditor.value = KODE_PRESET[key];
                if (currentFilename) {
                    currentFilename.textContent = `${key}.dap`;
                }
                updateLineNumbers();
                updateCursorPos();
                compileAndRun();
            }
        });
    }

    if (btnResetCode) {
        btnResetCode.addEventListener("click", () => {
            const key = presetSelector ? presetSelector.value : "tata_surya";
            if (KODE_PRESET[key] && codeEditor) {
                codeEditor.value = KODE_PRESET[key];
                updateLineNumbers();
                updateCursorPos();
                compileAndRun();
            }
        });
    }

    // Host Functions untuk WebAssembly Canvas 2D
    function createCanvasImports(getMemory) {
        return {
            kanvas_bersihkan: (r, g, b, a) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.fillStyle = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
                ctx.fillRect(0, 0, canvas.width, canvas.height);
            },
            kanvas_warna_isi: (r, g, b, a) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.fillStyle = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
            },
            kanvas_warna_garis: (r, g, b, a) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.strokeStyle = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
            },
            kanvas_tebal_garis: (tebal) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.lineWidth = tebal;
            },
            kanvas_persegi: (x, y, w, h) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.fillRect(x, y, w, h);
            },
            kanvas_bingkai_persegi: (x, y, w, h) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.strokeRect(x, y, w, h);
            },
            kanvas_persegi_bulat: (x, y, w, h, r) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                if (ctx.roundRect) {
                    ctx.roundRect(x, y, w, h, r);
                } else {
                    ctx.rect(x, y, w, h);
                }
                ctx.fill();
            },
            kanvas_bingkai_persegi_bulat: (x, y, w, h, r) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                if (ctx.roundRect) {
                    ctx.roundRect(x, y, w, h, r);
                } else {
                    ctx.strokeRect(x, y, w, h);
                }
                ctx.stroke();
            },
            kanvas_lingkaran: (x, y, radius) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                ctx.arc(x, y, radius, 0, Math.PI * 2);
                ctx.fill();
            },
            kanvas_bingkai_lingkaran: (x, y, radius) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                ctx.arc(x, y, radius, 0, Math.PI * 2);
                ctx.stroke();
            },
            kanvas_garis: (x1, y1, x2, y2) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                ctx.moveTo(x1, y1);
                ctx.lineTo(x2, y2);
                ctx.stroke();
            },
            kanvas_titik: (x, y, ukuran) => {
                totalCanvasCommands++;
                if (!ctx) return;
                ctx.beginPath();
                ctx.arc(x, y, ukuran / 2, 0, Math.PI * 2);
                ctx.fill();
            },
            kanvas_teks: (ptr, len, x, y, ukuran) => {
                totalCanvasCommands++;
                if (!ctx) return;
                const mem = getMemory();
                const text = mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : "";
                ctx.font = `${Math.round(ukuran)}px sans-serif`;
                ctx.fillText(text, x, y);
            },
            kanvas_teks_tengah: (ptr, len, cx, cy, ukuran) => {
                totalCanvasCommands++;
                if (!ctx) return;
                const mem = getMemory();
                const text = mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : "";
                ctx.font = `${Math.round(ukuran)}px sans-serif`;
                ctx.textAlign = 'center';
                ctx.textBaseline = 'middle';
                ctx.fillText(text, cx, cy);
                ctx.textAlign = 'left';
                ctx.textBaseline = 'alphabetic';
            },
            kanvas_lebar: () => (canvas ? canvas.width : 800.0),
            kanvas_tinggi: () => (canvas ? canvas.height : 480.0),
            kanvas_mouse_x: () => canvasMouseX,
            kanvas_mouse_y: () => canvasMouseY,
            kanvas_mouse_klik: () => (canvasMouseDown ? 1n : 0n),
            kanvas_tombol_ditekan: (kode) => (canvasKeysPressed.has(Number(kode)) ? 1n : 0n),
            kanvas_delta_waktu: () => currentDeltaTime,
            waktu_sekarang_ms: () => BigInt(Date.now()),
            audio_mainkan_nada: (freq, dur, vol) => {
                try {
                    const audioCtx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                    if (audioCtx.state === 'suspended') audioCtx.resume();
                    const osc = audioCtx.createOscillator();
                    const gain = audioCtx.createGain();
                    osc.type = 'sine';
                    osc.frequency.setValueAtTime(freq, audioCtx.currentTime);
                    const now = audioCtx.currentTime;
                    gain.gain.setValueAtTime(0.0001, now);
                    gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                    gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                    osc.connect(gain);
                    gain.connect(audioCtx.destination);
                    osc.start(now);
                    osc.stop(now + dur + 0.05);
                } catch(e) { console.error('Audio error:', e); }
            },
            audio_mainkan_gelombang: (tipe, freq, dur, vol) => {
                try {
                    const audioCtx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                    if (audioCtx.state === 'suspended') audioCtx.resume();
                    const types = ['sine', 'square', 'sawtooth', 'triangle'];
                    const osc = audioCtx.createOscillator();
                    const gain = audioCtx.createGain();
                    osc.type = types[Number(tipe) % 4] || 'sine';
                    osc.frequency.setValueAtTime(freq, audioCtx.currentTime);
                    const now = audioCtx.currentTime;
                    gain.gain.setValueAtTime(0.0001, now);
                    gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                    gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                    osc.connect(gain);
                    gain.connect(audioCtx.destination);
                    osc.start(now);
                    osc.stop(now + dur + 0.05);
                } catch(e) { console.error('Audio error:', e); }
            },
            audio_efek_suara: (tipe) => {
                try {
                    const audioCtx = window.__dapAudioCtx || (window.__dapAudioCtx = new (window.AudioContext || window.webkitAudioContext)());
                    if (audioCtx.state === 'suspended') audioCtx.resume();
                    const now = audioCtx.currentTime;
                    const osc = audioCtx.createOscillator();
                    const gain = audioCtx.createGain();
                    osc.connect(gain);
                    gain.connect(audioCtx.destination);
                    const t = Number(tipe);
                    if (t === 0) { // Bip
                        osc.type = 'sine';
                        osc.frequency.setValueAtTime(1000, now);
                        gain.gain.setValueAtTime(0.3, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.08);
                        osc.start(now); osc.stop(now + 0.09);
                    } else if (t === 1) { // Lompat
                        osc.type = 'square';
                        osc.frequency.setValueAtTime(150, now);
                        osc.frequency.exponentialRampToValueAtTime(600, now + 0.15);
                        gain.gain.setValueAtTime(0.25, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);
                        osc.start(now); osc.stop(now + 0.17);
                    } else if (t === 2) { // Koin
                        osc.type = 'sine';
                        osc.frequency.setValueAtTime(987.77, now);
                        osc.frequency.setValueAtTime(1318.51, now + 0.08);
                        gain.gain.setValueAtTime(0.3, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.35);
                        osc.start(now); osc.stop(now + 0.36);
                    } else if (t === 3) { // Ledakan
                        osc.type = 'sawtooth';
                        osc.frequency.setValueAtTime(220, now);
                        osc.frequency.exponentialRampToValueAtTime(30, now + 0.35);
                        gain.gain.setValueAtTime(0.4, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.38);
                        osc.start(now); osc.stop(now + 0.39);
                    } else if (t === 4) { // Kalah
                        osc.type = 'triangle';
                        osc.frequency.setValueAtTime(330, now);
                        osc.frequency.setValueAtTime(293, now + 0.12);
                        osc.frequency.setValueAtTime(261, now + 0.24);
                        osc.frequency.setValueAtTime(196, now + 0.36);
                        gain.gain.setValueAtTime(0.3, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);
                        osc.start(now); osc.stop(now + 0.56);
                    } else if (t === 5) { // Menang
                        osc.type = 'triangle';
                        osc.frequency.setValueAtTime(261, now);
                        osc.frequency.setValueAtTime(329, now + 0.1);
                        osc.frequency.setValueAtTime(392, now + 0.2);
                        osc.frequency.setValueAtTime(523, now + 0.3);
                        gain.gain.setValueAtTime(0.3, now);
                        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);
                        osc.start(now); osc.stop(now + 0.56);
                    }
                } catch(e) { console.error('Audio SFX error:', e); }
            },
            audio_frekuensi_nada: (ptr, len) => {
                try {
                    const mem = getMemory();
                    if (!mem) return 440.0;
                    const arr = new Uint8Array(mem.buffer, ptr, len);
                    const note = new TextDecoder().decode(arr).trim().toUpperCase();
                    const map = {
                        'C0': 16.35, 'C#0': 17.32, 'DB0': 17.32, 'D0': 18.35, 'D#0': 19.45, 'EB0': 19.45, 'E0': 20.60, 'F0': 21.83, 'F#0': 23.12, 'GB0': 23.12, 'G0': 24.50, 'G#0': 25.96, 'AB0': 25.96, 'A0': 27.50, 'A#0': 29.14, 'BB0': 29.14, 'B0': 30.87,
                        'C1': 32.70, 'C#1': 34.65, 'DB1': 34.65, 'D1': 36.71, 'D#1': 38.89, 'EB1': 38.89, 'E1': 41.20, 'F1': 43.65, 'F#1': 46.25, 'GB1': 46.25, 'G1': 49.00, 'G#1': 51.91, 'AB1': 51.91, 'A1': 55.00, 'A#1': 58.27, 'BB1': 58.27, 'B1': 61.74,
                        'C2': 65.41, 'C#2': 69.30, 'DB2': 69.30, 'D2': 73.42, 'D#2': 77.78, 'EB2': 77.78, 'E2': 82.41, 'F2': 87.31, 'F#2': 92.50, 'GB2': 92.50, 'G2': 98.00, 'G#2': 103.83, 'AB2': 103.83, 'A2': 110.00, 'A#2': 116.54, 'BB2': 116.54, 'B2': 123.47,
                        'C3': 130.81, 'C#3': 138.59, 'DB3': 138.59, 'D3': 146.83, 'D#3': 155.56, 'EB3': 155.56, 'E3': 164.81, 'F3': 174.61, 'F#3': 185.00, 'GB3': 185.00, 'G3': 196.00, 'G#3': 207.65, 'AB3': 207.65, 'A3': 220.00, 'A#3': 233.08, 'BB3': 233.08, 'B3': 246.94,
                        'C4': 261.63, 'C#4': 277.18, 'DB4': 277.18, 'D4': 293.66, 'D#4': 311.13, 'EB4': 311.13, 'E4': 329.63, 'F4': 349.23, 'F#4': 369.99, 'GB4': 369.99, 'G4': 392.00, 'G#4': 415.30, 'AB4': 415.30, 'A4': 440.00, 'A#4': 466.16, 'BB4': 466.16, 'B4': 493.88,
                        'C5': 523.25, 'C#5': 554.37, 'DB5': 554.37, 'D5': 587.33, 'D#5': 622.25, 'EB5': 622.25, 'E5': 659.25, 'F5': 698.46, 'F#5': 739.99, 'GB5': 739.99, 'G5': 783.99, 'G#5': 830.61, 'AB5': 830.61, 'A5': 880.00, 'A#5': 932.33, 'BB5': 932.33, 'B5': 987.77,
                        'C6': 1046.50, 'C#6': 1108.73, 'DB6': 1108.73, 'D6': 1174.66, 'D#6': 1244.51, 'EB6': 1244.51, 'E6': 1318.51, 'F6': 1396.91, 'F#6': 1479.98, 'GB6': 1479.98, 'G6': 1567.98, 'G#6': 1661.22, 'AB6': 1661.22, 'A6': 1760.00, 'A#6': 1864.66, 'BB6': 1864.66, 'B6': 1975.53
                    };
                    return map[note] || 440.0;
                } catch(e) { return 440.0; }
            },
            audio_set_volume: (vol) => {},
            audio_hentikan_semua: () => {
                try {
                    if (window.__dapAudioCtx) {
                        window.__dapAudioCtx.close();
                        window.__dapAudioCtx = null;
                    }
                } catch(e) {}
            },
        };
    }

    // Melacak posisi mouse & interaksi tombol di atas Canvas
    let canvasMouseX = 400.0;
    let canvasMouseY = 240.0;
    let canvasMouseDown = false;
    const canvasKeysPressed = new Set();

    // ==========================================
    // 📐 Manajemen Ukuran Frame Layar Dinamis
    // ==========================================
    const FRAME_PRESETS = {
        desktop_std: { nama: "Desktop Standar", w: 800, h: 480, ratio: "5:3", isDevice: false },
        desktop_hd: { nama: "Desktop HD", w: 960, h: 540, ratio: "16:9", isDevice: false },
        mobile_portrait: { nama: "Ponsel Potret (360×640)", w: 360, h: 640, ratio: "9:16", isDevice: true },
        mobile_iphone: { nama: "Ponsel Modern (390×844)", w: 390, h: 844, ratio: "9:19.5", isDevice: true },
        mobile_landscape: { nama: "Ponsel Lanskap (640×360)", w: 640, h: 360, ratio: "16:9", isDevice: true },
        tablet_portrait: { nama: "Tablet Potret (768×1024)", w: 768, h: 1024, ratio: "3:4", isDevice: true },
        tablet_landscape: { nama: "Tablet Lanskap (768×576)", w: 768, h: 576, ratio: "4:3", isDevice: true },
        persegi: { nama: "Persegi 1:1 (500×500)", w: 500, h: 500, ratio: "1:1", isDevice: false },
        auto_fit: { nama: "Dinamis Wadah", w: 800, h: 480, ratio: "Auto", isAuto: true },
        custom: { nama: "Kustom", w: 800, h: 480, ratio: "Kustom", isCustom: true }
    };

    let currentFrameKey = "desktop_std";
    let isBezelEnabled = false;
    let isWideMode = false;

    function applyCanvasResolution(w, h, ratioLabel, shouldUpdateBezel = true) {
        if (!canvas) return;
        const clampedW = Math.max(200, Math.min(2560, Math.round(w)));
        const clampedH = Math.max(200, Math.min(2560, Math.round(h)));

        canvas.width = clampedW;
        canvas.height = clampedH;
        canvas.style.aspectRatio = `${clampedW} / ${clampedH}`;

        const gcd = (a, b) => (b === 0 ? a : gcd(b, a % b));
        const divisor = gcd(clampedW, clampedH);
        const autoRatio = `${Math.round(clampedW / divisor)}:${Math.round(clampedH / divisor)}`;
        const displayRatio = ratioLabel || autoRatio;

        if (frameDimBadge) {
            frameDimBadge.textContent = `${clampedW} × ${clampedH} px (${displayRatio})`;
        }
        if (canvasWatermark) {
            canvasWatermark.textContent = `${clampedW} × ${clampedH}`;
        }
        if (customWidth) customWidth.value = clampedW;
        if (customHeight) customHeight.value = clampedH;

        if (shouldUpdateBezel && deviceFrameMockup) {
            const preset = FRAME_PRESETS[currentFrameKey];
            if (preset && preset.isDevice) {
                deviceFrameMockup.classList.add("with-bezel");
                isBezelEnabled = true;
                if (btnToggleBezel) btnToggleBezel.classList.add("highlight");
            } else if (!isBezelEnabled) {
                deviceFrameMockup.classList.remove("with-bezel");
                if (btnToggleBezel) btnToggleBezel.classList.remove("highlight");
            }
        }

        // Render ulang grafis kanvas agar komputasi w := kanvas_lebar() terbarui
        renderCanvasWasm();
    }

    function setFramePreset(key) {
        currentFrameKey = key;
        const preset = FRAME_PRESETS[key] || FRAME_PRESETS.desktop_std;

        if (customResRow) {
            customResRow.style.display = key === "custom" ? "flex" : "none";
        }

        if (preset.isAuto) {
            updateAutoFitDimensions();
        } else {
            applyCanvasResolution(preset.w, preset.h, preset.ratio, true);
        }
    }

    function updateAutoFitDimensions() {
        if (currentFrameKey !== "auto_fit" || !canvasViewport) return;
        const viewportRect = canvasViewport.getBoundingClientRect();
        const availableW = Math.max(280, Math.floor(viewportRect.width - 40));
        const availableH = Math.max(240, Math.min(640, Math.floor(availableW * 0.6)));
        applyCanvasResolution(availableW, availableH, "Auto-Fit", false);
    }

    if (framePresetSelector) {
        framePresetSelector.addEventListener("change", (e) => {
            setFramePreset(e.target.value);
        });
    }

    if (btnFlipOrientation) {
        btnFlipOrientation.addEventListener("click", () => {
            if (!canvas) return;
            const newW = canvas.height;
            const newH = canvas.width;
            applyCanvasResolution(newW, newH, null, false);
        });
    }

    if (btnToggleBezel && deviceFrameMockup) {
        btnToggleBezel.addEventListener("click", () => {
            isBezelEnabled = !isBezelEnabled;
            deviceFrameMockup.classList.toggle("with-bezel", isBezelEnabled);
            btnToggleBezel.classList.toggle("highlight", isBezelEnabled);
        });
    }

    if (btnToggleWide && playgroundContainer) {
        btnToggleWide.addEventListener("click", () => {
            isWideMode = !isWideMode;
            playgroundContainer.classList.toggle("wide-mode", isWideMode);
            btnToggleWide.textContent = isWideMode ? "⛶ Lebar Normal" : "⛶ Lebar Penuh";
            btnToggleWide.classList.toggle("highlight", isWideMode);
            if (currentFrameKey === "auto_fit") {
                setTimeout(updateAutoFitDimensions, 250);
            }
        });
    }

    if (btnApplyCustom) {
        btnApplyCustom.addEventListener("click", () => {
            const w = parseInt(customWidth.value, 10) || 800;
            const h = parseInt(customHeight.value, 10) || 480;
            applyCanvasResolution(w, h, "Kustom", false);
        });
    }

    window.addEventListener("resize", () => {
        if (currentFrameKey === "auto_fit") {
            updateAutoFitDimensions();
        }
    });

    if (window.ResizeObserver && canvasViewport) {
        const ro = new ResizeObserver(() => {
            if (currentFrameKey === "auto_fit") {
                updateAutoFitDimensions();
            }
        });
        ro.observe(canvasViewport);
    }

    function updateCanvasPointer(clientX, clientY) {
        if (!canvas) return;
        const rect = canvas.getBoundingClientRect();
        if (rect.width <= 0 || rect.height <= 0) return;
        const scaleX = canvas.width / rect.width;
        const scaleY = canvas.height / rect.height;
        canvasMouseX = Math.max(0, Math.min(canvas.width, (clientX - rect.left) * scaleX));
        canvasMouseY = Math.max(0, Math.min(canvas.height, (clientY - rect.top) * scaleY));
        if (coordsOverlay) {
            coordsOverlay.textContent = `X: ${Math.round(canvasMouseX)}, Y: ${Math.round(canvasMouseY)}`;
        }
    }

    if (canvas) {
        canvas.addEventListener("mousemove", (e) => {
            updateCanvasPointer(e.clientX, e.clientY);
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });

        canvas.addEventListener("mousedown", (e) => {
            canvasMouseDown = true;
            updateCanvasPointer(e.clientX, e.clientY);
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });

        canvas.addEventListener("mouseup", () => {
            canvasMouseDown = false;
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });

        canvas.addEventListener("mouseleave", () => {
            canvasMouseDown = false;
        });

        // Dukungan Sentuh Layar Dinamis (Touch Events untuk Layar Mobile & Tablet)
        canvas.addEventListener("touchstart", (e) => {
            const touch = e.touches[0];
            if (touch) {
                canvasMouseDown = true;
                updateCanvasPointer(touch.clientX, touch.clientY);
                if (canvasWasmInstance) renderCanvasWasm();
            }
            e.preventDefault();
        }, { passive: false });

        canvas.addEventListener("touchmove", (e) => {
            const touch = e.touches[0];
            if (touch) {
                updateCanvasPointer(touch.clientX, touch.clientY);
                if (canvasWasmInstance) renderCanvasWasm();
            }
            e.preventDefault();
        }, { passive: false });

        canvas.addEventListener("touchend", (e) => {
            canvasMouseDown = false;
            if (canvasWasmInstance) renderCanvasWasm();
            e.preventDefault();
        }, { passive: false });

        canvas.addEventListener("touchcancel", (e) => {
            canvasMouseDown = false;
            e.preventDefault();
        }, { passive: false });
    }

    window.addEventListener("keydown", (e) => {
        canvasKeysPressed.add(e.keyCode || e.which);
        if (canvasWasmInstance) {
            renderCanvasWasm();
        }
    });

    window.addEventListener("keyup", (e) => {
        canvasKeysPressed.delete(e.keyCode || e.which);
        if (canvasWasmInstance) {
            renderCanvasWasm();
        }
    });

    // Kompilasi dan Jalankan Kode dari Editor
    async function compileAndRun() {
        if (!codeEditor) return;
        const code = codeEditor.value;

        if (compileStatusDot) {
            compileStatusDot.className = "status-indicator compiling";
        }
        if (compileStatusMsg) {
            compileStatusMsg.textContent = "Mengompilasi kode sumber ke WebAssembly...";
        }

        const startCompile = performance.now();
        try {
            const resp = await fetch("/api/kompilasi", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ kode: code }),
            });

            const data = await resp.json();
            const durationCompile = (performance.now() - startCompile).toFixed(2);
            if (canvasCompileTime) canvasCompileTime.textContent = `${durationCompile} ms`;

            if (!data.sukses) {
                if (compileStatusDot) compileStatusDot.className = "status-indicator error";
                if (compileStatusMsg) compileStatusMsg.textContent = `Galat: ${data.galat}`;
                appendWasmLog(`[Galat Kompilasi] ${data.galat}`);
                return;
            }

            // Sukses kompilasi: decode Base64
            const raw = atob(data.wasm_base64);
            const bytes = new Uint8Array(raw.length);
            for (let i = 0; i < raw.length; i++) {
                bytes[i] = raw.charCodeAt(i);
            }

            if (canvasWasmSize) {
                canvasWasmSize.textContent = `${bytes.length} byte`;
            }

            const canvasImports = {
                env: {
                    cetak_u64: (val) => appendWasmLog(`[WASM cetak] ${val}`),
                    cetak_f64: (val) => appendWasmLog(`[WASM cetak] ${val}`),
                    cetak_teks: (ptr, len) => {
                        const arr = new Uint8Array(canvasWasmMemory.buffer, ptr, len);
                        appendWasmLog(`[Kanvas WASM] ${new TextDecoder().decode(arr)}`);
                    },
                    sin: Math.sin,
                    cos: Math.cos,
                    tan: Math.tan,
                    asin: Math.asin,
                    acos: Math.acos,
                    atan: Math.atan,
                    atan2: Math.atan2,
                    akar_kubik: Math.cbrt,
                    pangkat: Math.pow,
                    log: Math.log,
                    log10: Math.log10,
                    log2: Math.log2,
                    exp: Math.exp,
                    tanda: Math.sign,
                    ...createCanvasImports(() => canvasWasmMemory),
                },
            };
            canvasImports.env = new Proxy(canvasImports.env, {
                get(target, prop) {
                    if (prop in target) return target[prop];
                    return () => 0;
                }
            });

            const { instance } = await WebAssembly.instantiate(bytes, canvasImports);
            canvasWasmInstance = instance;
            canvasWasmMemory = instance.exports.memory;

            if (compileStatusDot) compileStatusDot.className = "status-indicator ready";
            if (compileStatusMsg) compileStatusMsg.textContent = `Kompilasi sukses (${bytes.length} byte)`;
            if (canvasStatusText) canvasStatusText.textContent = "Playground Aktif";

            // Jalankan fungsi
            renderCanvasWasm();
        } catch (err) {
            // Jika /api/kompilasi offline, gunakan fallback demo_kanvas.wasm lokal
            console.warn("API kompilasi offline, memuat /demo_kanvas.wasm fallback:", err);
            initFallbackWasm();
        }
    }

    async function initFallbackWasm() {
        try {
            const resp = await fetch("/demo_kanvas.wasm");
            if (!resp.ok) return;
            const bytes = await resp.arrayBuffer();
            if (canvasWasmSize) canvasWasmSize.textContent = `${bytes.byteLength} byte`;

            const canvasImports = {
                env: {
                    cetak_u64: () => {},
                    cetak_f64: () => {},
                    cetak_teks: (ptr, len) => {
                        const arr = new Uint8Array(canvasWasmMemory.buffer, ptr, len);
                        appendWasmLog(`[Kanvas WASM] ${new TextDecoder().decode(arr)}`);
                    },
                    sin: Math.sin,
                    cos: Math.cos,
                    ...createCanvasImports(() => canvasWasmMemory),
                },
            };
            const { instance } = await WebAssembly.instantiate(bytes, canvasImports);
            canvasWasmInstance = instance;
            canvasWasmMemory = instance.exports.memory;
            if (compileStatusDot) compileStatusDot.className = "status-indicator ready";
            if (compileStatusMsg) compileStatusMsg.textContent = "Modul statis siap dieksekusi";
            renderCanvasWasm();
        } catch (e) {
            console.error("Gagal fallback WASM:", e);
        }
    }

    function renderCanvasWasm() {
        if (!canvasWasmInstance) return;
        totalCanvasCommands = 0;
        const start = performance.now();
        if (canvasWasmInstance.exports.render_game) {
            canvasWasmInstance.exports.render_game();
        } else if (canvasWasmInstance.exports.gambar_tata_surya) {
            canvasWasmInstance.exports.gambar_tata_surya();
        } else if (canvasWasmInstance.exports.gambar_spiral) {
            canvasWasmInstance.exports.gambar_spiral();
        } else if (canvasWasmInstance.exports.render_partikel) {
            canvasWasmInstance.exports.render_partikel();
        } else if (canvasWasmInstance.exports.gambar_grafik) {
            canvasWasmInstance.exports.gambar_grafik();
        } else if (canvasWasmInstance.exports.utama) {
            canvasWasmInstance.exports.utama();
        }
        const duration = (performance.now() - start).toFixed(2);
        if (canvasRenderTime) canvasRenderTime.textContent = `${duration} ms`;
        if (canvasCmdCount) canvasCmdCount.textContent = `${totalCanvasCommands} panggilan`;
    }

    if (btnCompileRun) {
        btnCompileRun.addEventListener("click", compileAndRun);
    }

    if (btnAnimasiKanvas) {
        function updateDeltaTime() {
            const now = performance.now();
            currentDeltaTime = Math.min((now - lastAnimFrameTime) / 1000.0, 0.1);
            if (currentDeltaTime <= 0) currentDeltaTime = 0.0166;
            lastAnimFrameTime = now;
        }

        function animateLoop() {
            if (!isAnimating) return;
            updateDeltaTime();
            renderCanvasWasm();
            animationReqId = requestAnimationFrame(animateLoop);
        }

        btnAnimasiKanvas.addEventListener("click", () => {
            isAnimating = !isAnimating;
            if (isAnimating) {
                lastAnimFrameTime = performance.now();
                currentDeltaTime = 0.0166;
                btnAnimasiKanvas.textContent = "⏸ Jeda Animasi Loop";
                btnAnimasiKanvas.classList.remove("secondary");
                btnAnimasiKanvas.classList.add("danger");
                animateLoop();
            } else {
                btnAnimasiKanvas.textContent = "▶ Mulai Animasi Loop (60 FPS)";
                btnAnimasiKanvas.classList.remove("danger");
                btnAnimasiKanvas.classList.add("secondary");
                if (animationReqId) cancelAnimationFrame(animationReqId);
            }
        });
    }

    if (btnBersihkanKanvas) {
        btnBersihkanKanvas.addEventListener("click", () => {
            if (ctx && canvas) {
                ctx.clearRect(0, 0, canvas.width, canvas.height);
                totalCanvasCommands = 0;
                if (canvasCmdCount) canvasCmdCount.textContent = "0 panggilan";
                if (canvasRenderTime) canvasRenderTime.textContent = "0.00 ms";
            }
        });
    }

    // Inisialisasi awal frame dinamis dan kompilasi kode
    setFramePreset("desktop_std");
    compileAndRun();

    // ==========================================
    // 🔌 WebSocket Real-Time Client Implementation
    // ==========================================
    const wsBadge = document.getElementById("ws-badge");
    const wsStatusText = document.getElementById("ws-status-text");
    const wsEndpoint = document.getElementById("ws-endpoint");
    const wsLatency = document.getElementById("ws-latency");
    const wsPktCount = document.getElementById("ws-pkt-count");
    const wsBox = document.getElementById("ws-box");
    const wsInput = document.getElementById("ws-input");
    const btnWsSend = document.getElementById("btn-ws-send");
    const btnWsPing = document.getElementById("btn-ws-ping");
    const btnWsReconnect = document.getElementById("btn-ws-reconnect");

    let socket = null;
    let packetCounter = 0;
    let pingSentTimestamp = 0;

    function appendWsLog(prefix, msg) {
        const time = new Date().toLocaleTimeString();
        const line = `[${time}] ${prefix} ${msg}`;
        if (wsBox.textContent.includes("Menunggu jabat tangan")) {
            wsBox.textContent = line;
        } else {
            wsBox.textContent += `\n${line}`;
        }
        wsBox.scrollTop = wsBox.scrollHeight;
    }

    function initWebSocket() {
        if (socket) {
            try {
                socket.close();
            } catch (_) {}
        }

        const host = window.location.host || "localhost:8080";
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        const wsUrl = `${protocol}//${host}/ws`;

        wsEndpoint.textContent = wsUrl;
        wsBadge.className = "ws-status-badge";
        wsStatusText.textContent = "Menghubungkan...";
        appendWsLog("🔄", `Membuka koneksi WebSocket ke ${wsUrl} ...`);

        try {
            socket = new WebSocket(wsUrl);
        } catch (e) {
            appendWsLog("❌", `Inisialisasi WebSocket gagal: ${e.message}`);
            wsStatusText.textContent = "Gagal Terhubung";
            return;
        }

        socket.onopen = () => {
            wsBadge.className = "ws-status-badge connected";
            wsStatusText.textContent = "Terhubung (RFC 6455)";
            appendWsLog("✅", "Jabat tangan (handshake) 101 Switching Protocols berhasil! Koneksi siap.");
        };

        socket.onmessage = (event) => {
            packetCounter++;
            wsPktCount.textContent = packetCounter;

            const text = event.data;
            // Deteksi respon latency ping
            if (text.includes("PING:")) {
                const parts = text.split("PING:");
                if (parts.length > 1) {
                    const sentAt = parseInt(parts[1].trim(), 10);
                    if (!isNaN(sentAt)) {
                        const rtt = Date.now() - sentAt;
                        wsLatency.textContent = `${rtt} ms`;
                    }
                }
            }

            appendWsLog("📥", text);
        };

        socket.onerror = (err) => {
            appendWsLog("⚠️", "Kesalahan koneksi WebSocket terdeteksi.");
        };

        socket.onclose = (event) => {
            wsBadge.className = "ws-status-badge";
            wsStatusText.textContent = "Terputus";
            appendWsLog("🔌", `Koneksi ditutup (Kode: ${event.code}${event.reason ? ", Alasan: " + event.reason : ""})`);
        };
    }

    function sendWsMessage() {
        if (!socket || socket.readyState !== WebSocket.OPEN) {
            appendWsLog("❌", "Gagal kirim: WebSocket belum terhubung!");
            return;
        }
        const text = wsInput.value.trim();
        if (!text) return;

        appendWsLog("📤", text);
        socket.send(text);
    }

    function sendWsPing() {
        if (!socket || socket.readyState !== WebSocket.OPEN) {
            appendWsLog("❌", "Gagal ping: WebSocket belum terhubung!");
            return;
        }
        pingSentTimestamp = Date.now();
        const pingPayload = `PING:${pingSentTimestamp}`;
        appendWsLog("⚡", `Mengirim probe latency: ${pingPayload}`);
        socket.send(pingPayload);
    }

    btnWsSend.addEventListener("click", sendWsMessage);
    btnWsPing.addEventListener("click", sendWsPing);
    btnWsReconnect.addEventListener("click", initWebSocket);

    wsInput.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
            sendWsMessage();
        }
    });

    // Mulai koneksi WebSocket otomatis
    initWebSocket();
});

