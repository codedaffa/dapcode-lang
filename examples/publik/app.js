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

    let canvasWasmInstance = null;
    let canvasWasmMemory = null;
    let totalCanvasCommands = 0;
    let isAnimating = false;
    let animationReqId = null;
    let currentDeltaTime = 0.0166;
    let lastAnimFrameTime = performance.now();

    // Koleksi Preset Kode Contoh Interaktif
    const KODE_PRESET = {
        tata_surya: `// ========================================================
// 🌌 Demo: Tata Surya Orbit 2D via WebAssembly
// ========================================================

fungsi gambar_tata_surya() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0

    // 1. Bersihkan kanvas dengan warna latar kosmik
    kanvas_bersihkan(0.04, 0.06, 0.12, 1.0)

    // 2. Bintang acak / titik konstelasi
    kanvas_warna_isi(0.8, 0.9, 1.0, 0.7)
    kanvas_titik(cx - 200.0, cy - 120.0, 3.0)
    kanvas_titik(cx + 180.0, cy - 100.0, 2.5)
    kanvas_titik(cx - 150.0, cy + 130.0, 3.5)
    kanvas_titik(cx + 220.0, cy + 90.0, 2.0)
    kanvas_titik(cx - 80.0, cy - 160.0, 2.0)
    kanvas_titik(cx + 100.0, cy + 150.0, 3.0)

    // 3. Garis radar sumbu kartesius tipis
    kanvas_warna_garis(0.2, 0.35, 0.55, 0.3)
    kanvas_tebal_garis(1.0)
    kanvas_garis(cx - 260.0, cy, cx + 260.0, cy)
    kanvas_garis(cx, cy - 180.0, cx, cy + 180.0)

    // 4. Orbit sirkular konsentris
    kanvas_warna_garis(0.25, 0.5, 0.8, 0.4)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_lingkaran(cx, cy, 60.0)
    kanvas_bingkai_lingkaran(cx, cy, 110.0)
    kanvas_bingkai_lingkaran(cx, cy, 170.0)

    // 5. Bintang pusat (Matahari Inti)
    kanvas_warna_isi(1.0, 0.8, 0.2, 0.3)
    kanvas_lingkaran(cx, cy, 32.0)
    kanvas_warna_isi(1.0, 0.5, 0.0, 0.9)
    kanvas_lingkaran(cx, cy, 22.0)
    kanvas_warna_isi(1.0, 0.95, 0.4, 1.0)
    kanvas_lingkaran(cx, cy, 12.0)

    // 6. Planet 1 (Biru Cyan)
    p1_x := cx + 60.0 * cos(0.8)
    p1_y := cy + 60.0 * sin(0.8)
    kanvas_warna_isi(0.2, 0.85, 0.95, 1.0)
    kanvas_lingkaran(p1_x, p1_y, 7.0)

    // 7. Planet 2 (Hijau Emerald & Cincin)
    p2_x := cx + 110.0 * cos(2.3)
    p2_y := cy + 110.0 * sin(2.3)
    kanvas_warna_isi(0.2, 0.9, 0.5, 1.0)
    kanvas_lingkaran(p2_x, p2_y, 11.0)
    kanvas_warna_garis(0.4, 1.0, 0.7, 0.6)
    kanvas_tebal_garis(2.0)
    kanvas_bingkai_lingkaran(p2_x, p2_y, 17.0)

    // 8. Planet 3 (Ungu Violet)
    p3_x := cx + 170.0 * cos(4.2)
    p3_y := cy + 170.0 * sin(4.2)
    kanvas_warna_isi(0.7, 0.3, 0.95, 1.0)
    kanvas_lingkaran(p3_x, p3_y, 16.0)

    // 9. Kartu Status HUD
    kanvas_warna_isi(0.08, 0.12, 0.22, 0.85)
    kanvas_persegi(20.0, 20.0, 220.0, 70.0)
    kanvas_warna_garis(0.22, 0.65, 0.95, 0.75)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_persegi(20.0, 20.0, 220.0, 70.0)

    // 10. Teks HUD Informasi
    kanvas_warna_isi(0.95, 0.98, 1.0, 1.0)
    kanvas_teks("DapCode 2D Canvas Engine", 32.0, 44.0, 14.0)
    kanvas_warna_isi(0.4, 0.85, 1.0, 0.9)
    kanvas_teks("Target: WebAssembly Native", 32.0, 68.0, 12.0)

    cetak("Grafis HTML5 Canvas 2D berhasil dirender dari WebAssembly!")
}

fungsi utama() -> u64 {
    gambar_tata_surya()
    kembali 0
}`,
        spiral_archimedes: `// ========================================================
// 🌀 Demo: Pola Spiral Geometri Archimedes
// ========================================================

fungsi gambar_spiral() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    cx := w / 2.0
    cy := h / 2.0

    // 1. Bersihkan kanvas
    kanvas_bersihkan(0.02, 0.04, 0.08, 1.0)
    kanvas_tebal_garis(2.0)

    // 2. Gambar deret titik spiral trigonometrik
    theta := 0.0
    a := 2.5
    b := 1.8

    selama theta < 25.0 {
        r := a + b * theta
        x := cx + r * cos(theta)
        y := cy + r * sin(theta)

        // Variasi warna gradasi berbasis sudut rotasi
        red := 0.5 + 0.5 * sin(theta * 0.5)
        green := 0.5 + 0.5 * cos(theta * 0.3)
        blue := 0.95

        kanvas_warna_isi(red, green, blue, 0.8)
        kanvas_lingkaran(x, y, 4.0 + (theta * 0.15))

        theta = theta + 0.15
    }

    // 3. Label HUD
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.95)
    kanvas_teks("Pola Spiral Archimedes", 25.0, 40.0, 15.0)
    kanvas_warna_isi(0.4, 0.8, 1.0, 0.8)
    kanvas_teks("r = a + b * theta via WebAssembly", 25.0, 65.0, 12.0)

    cetak("Pola Spiral Archimedes berhasil dirender!")
}

fungsi utama() -> u64 {
    gambar_spiral()
    kembali 0
}`,
        partikel_pantul: `// ========================================================
// ⚡ Demo: Simulasi Partikel Pantul & Medan Energi 2D
// ========================================================

fungsi render_partikel() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()

    kanvas_bersihkan(0.05, 0.08, 0.14, 1.0)

    // 1. Bingkai pembatas arena fisika
    kanvas_warna_garis(0.2, 0.4, 0.7, 0.5)
    kanvas_tebal_garis(3.0)
    kanvas_bingkai_persegi(20.0, 20.0, w - 40.0, h - 40.0)

    // 2. Partikel 1: Inti Plasma Biru
    kanvas_warna_isi(0.1, 0.7, 1.0, 0.85)
    kanvas_lingkaran(180.0, 140.0, 28.0)
    kanvas_warna_garis(0.4, 0.9, 1.0, 0.9)
    kanvas_tebal_garis(2.0)
    kanvas_bingkai_lingkaran(180.0, 140.0, 34.0)

    // 3. Partikel 2: Bola Energi Neon Jingga
    kanvas_warna_isi(1.0, 0.4, 0.1, 0.9)
    kanvas_lingkaran(520.0, 280.0, 36.0)
    kanvas_warna_garis(1.0, 0.7, 0.2, 0.8)
    kanvas_bingkai_lingkaran(520.0, 280.0, 44.0)

    // 4. Partikel 3: Satelit Hijau Lime
    kanvas_warna_isi(0.2, 0.9, 0.4, 0.85)
    kanvas_lingkaran(340.0, 360.0, 20.0)

    // 5. Garis gaya medan vektor energi
    kanvas_warna_garis(1.0, 0.9, 0.2, 0.6)
    kanvas_tebal_garis(1.5)
    kanvas_garis(180.0, 140.0, 520.0, 280.0)
    kanvas_garis(520.0, 280.0, 340.0, 360.0)

    // 6. Teks judul
    kanvas_warna_isi(0.9, 0.95, 1.0, 1.0)
    kanvas_teks("Simulasi Fisika Partikel 2D", 35.0, 50.0, 15.0)
    kanvas_warna_isi(0.5, 0.7, 0.9, 0.8)
    kanvas_teks("Vektor Tumbukan & Medan Gaya", 35.0, 75.0, 12.0)

    cetak("Partikel 2D berhasil disimulasikan!")
}

fungsi utama() -> u64 {
    render_partikel()
    kembali 0
}`,
        grafik_batang: `// ========================================================
// 📊 Demo: Diagram Batang Statistik Interaktif
// ========================================================

fungsi gambar_grafik() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()

    kanvas_bersihkan(0.04, 0.07, 0.12, 1.0)

    // 1. Sumbu X dan Y
    kanvas_warna_garis(0.3, 0.45, 0.65, 0.6)
    kanvas_tebal_garis(2.0)
    kanvas_garis(80.0, 380.0, 740.0, 380.0)
    kanvas_garis(80.0, 80.0, 80.0, 380.0)

    // 2. Batang 1: DapCode Native (92%)
    kanvas_warna_isi(0.2, 0.75, 1.0, 0.9)
    kanvas_persegi(120.0, 380.0 - 276.0, 80.0, 276.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("92%", 140.0, 380.0 - 286.0, 13.0)
    kanvas_teks("DapCode", 125.0, 405.0, 12.0)

    // 3. Batang 2: WebAssembly MVP (85%)
    kanvas_warna_isi(0.6, 0.3, 0.95, 0.9)
    kanvas_persegi(240.0, 380.0 - 255.0, 80.0, 255.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("85%", 260.0, 380.0 - 265.0, 13.0)
    kanvas_teks("WASM MVP", 240.0, 405.0, 12.0)

    // 4. Batang 3: C11 Native (96%)
    kanvas_warna_isi(0.1, 0.85, 0.5, 0.9)
    kanvas_persegi(360.0, 380.0 - 288.0, 80.0, 288.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("96%", 380.0, 380.0 - 298.0, 13.0)
    kanvas_teks("C11 Native", 365.0, 405.0, 12.0)

    // 5. Batang 4: JIT Engine (74%)
    kanvas_warna_isi(1.0, 0.65, 0.1, 0.9)
    kanvas_persegi(480.0, 380.0 - 222.0, 80.0, 222.0)
    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("74%", 500.0, 380.0 - 232.0, 13.0)
    kanvas_teks("JIT Engine", 485.0, 405.0, 12.0)

    // 6. Judul Grafik
    kanvas_warna_isi(0.95, 0.98, 1.0, 1.0)
    kanvas_teks("Performa Komputasi & Efisiensi Memori (%)", 90.0, 55.0, 16.0)

    cetak("Grafik Batang Statistik berhasil digambar!")
}

fungsi utama() -> u64 {
    gambar_grafik()
    kembali 0
}`,
        game_pesawat: `// ========================================================
// 🎮 Demo Game: Pesawat Antariksa & Laser Interaktif 2D
// Kontrol:
// - Gerakkan Mouse: Mengarahkan posisi pesawat & target
// - Klik Mouse: Menembakkan laser energi ganda
// - Tombol [W / A / S / D] / [Panah]: Manuver reaktor
// ========================================================

fungsi gambar_bintang(w: f64, h: f64) -> Unit {
    kanvas_bersihkan(0.02, 0.03, 0.07, 1.0)
    kanvas_warna_isi(0.8, 0.9, 1.0, 0.5)
    kanvas_titik(80.0, 60.0, 2.0)
    kanvas_titik(220.0, 110.0, 2.5)
    kanvas_titik(450.0, 80.0, 1.5)
    kanvas_titik(670.0, 140.0, 2.0)
    kanvas_titik(140.0, 320.0, 2.0)
    kanvas_titik(380.0, 420.0, 3.0)
    kanvas_titik(590.0, 310.0, 1.5)
    kanvas_titik(720.0, 410.0, 2.5)
}

fungsi render_pesawat(px: f64, py: f64, klik: u64) -> Unit {
    // 1. Api Pendorong Reaktor (Thruster)
    kanvas_warna_isi(1.0, 0.45, 0.1, 0.9)
    kanvas_lingkaran(px - 22.0, py, 7.0)
    kanvas_warna_isi(1.0, 0.85, 0.2, 0.95)
    kanvas_lingkaran(px - 17.0, py, 4.0)

    // 2. Sayap & Lambung Pesawat
    kanvas_warna_isi(0.18, 0.45, 0.85, 1.0)
    kanvas_garis(px + 28.0, py, px - 18.0, py - 18.0)
    kanvas_garis(px - 18.0, py - 18.0, px - 10.0, py)
    kanvas_garis(px - 10.0, py, px - 18.0, py + 18.0)
    kanvas_garis(px - 18.0, py + 18.0, px + 28.0, py)

    kanvas_warna_isi(0.25, 0.65, 1.0, 0.9)
    kanvas_persegi(px - 10.0, py - 6.0, 22.0, 12.0)

    // 3. Kokpit Kaca Neon
    kanvas_warna_isi(0.4, 0.95, 1.0, 0.95)
    kanvas_lingkaran(px + 6.0, py, 5.0)

    // 4. Efek Laser Tembakan saat Mouse Diklik
    jika klik == 1 {
        kanvas_warna_garis(0.2, 1.0, 0.5, 0.95)
        kanvas_tebal_garis(3.0)
        kanvas_garis(px + 20.0, py - 12.0, 800.0, py - 12.0)
        kanvas_garis(px + 20.0, py + 12.0, 800.0, py + 12.0)

        kanvas_warna_isi(1.0, 1.0, 1.0, 0.9)
        kanvas_lingkaran(px + 22.0, py - 12.0, 4.0)
        kanvas_lingkaran(px + 22.0, py + 12.0, 4.0)
    }
}

fungsi render_game() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()
    mx := kanvas_mouse_x()
    my := kanvas_mouse_y()
    klik := kanvas_mouse_klik()

    // 1. Latar antariksa
    gambar_bintang(w, h)

    // 2. Pesawat Interaktif
    render_pesawat(mx, my, klik)

    // 3. HUD Target Reticle
    kanvas_warna_garis(0.2, 0.8, 1.0, 0.5)
    kanvas_tebal_garis(1.0)
    kanvas_bingkai_lingkaran(mx, my, 35.0)
    kanvas_garis(mx - 45.0, my, mx + 45.0, my)
    kanvas_garis(mx, my - 45.0, mx, my + 45.0)

    // 4. Status HUD Banner
    kanvas_warna_isi(0.06, 0.1, 0.18, 0.85)
    kanvas_persegi(15.0, 15.0, 300.0, 75.0)
    kanvas_warna_garis(0.3, 0.7, 1.0, 0.6)
    kanvas_tebal_garis(1.5)
    kanvas_bingkai_persegi(15.0, 15.0, 300.0, 75.0)

    kanvas_warna_isi(1.0, 1.0, 1.0, 1.0)
    kanvas_teks("🎮 SIMULATOR PESAWAT ANTARIKSA", 26.0, 38.0, 13.0)

    kanvas_warna_isi(0.4, 0.9, 1.0, 0.9)
    kanvas_teks("Kontrol: Gerakkan Mouse & Klik Tembak", 26.0, 58.0, 11.0)

    jika klik == 1 {
        kanvas_warna_isi(0.2, 1.0, 0.5, 1.0)
        kanvas_teks("STATUS: ⚡ LASER DIAKTIFKAN!", 26.0, 76.0, 11.0)
    } selain_itu {
        kanvas_warna_isi(0.8, 0.8, 0.8, 0.7)
        kanvas_teks("STATUS: Menjelajah Orbit Antariksa", 26.0, 76.0, 11.0)
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
            kanvas_lebar: () => (canvas ? canvas.width : 800.0),
            kanvas_tinggi: () => (canvas ? canvas.height : 480.0),
            kanvas_mouse_x: () => canvasMouseX,
            kanvas_mouse_y: () => canvasMouseY,
            kanvas_mouse_klik: () => (canvasMouseDown ? 1n : 0n),
            kanvas_tombol_ditekan: (kode) => (canvasKeysPressed.has(Number(kode)) ? 1n : 0n),
            kanvas_delta_waktu: () => currentDeltaTime,
            waktu_sekarang_ms: () => BigInt(Date.now()),
        };
    }

    // Melacak posisi mouse & interaksi tombol di atas Canvas
    let canvasMouseX = 400.0;
    let canvasMouseY = 240.0;
    let canvasMouseDown = false;
    const canvasKeysPressed = new Set();

    if (canvas) {
        canvas.addEventListener("mousemove", (e) => {
            const rect = canvas.getBoundingClientRect();
            const scaleX = canvas.width / rect.width;
            const scaleY = canvas.height / rect.height;
            canvasMouseX = (e.clientX - rect.left) * scaleX;
            canvasMouseY = (e.clientY - rect.top) * scaleY;
            if (coordsOverlay) {
                coordsOverlay.textContent = `X: ${Math.round(canvasMouseX)}, Y: ${Math.round(canvasMouseY)}`;
            }
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });

        canvas.addEventListener("mousedown", (e) => {
            canvasMouseDown = true;
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });

        canvas.addEventListener("mouseup", (e) => {
            canvasMouseDown = false;
            if (canvasWasmInstance) {
                renderCanvasWasm();
            }
        });
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

    // Inisialisasi awal: coba kompilasi kode awal atau muat wasm lokal
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

