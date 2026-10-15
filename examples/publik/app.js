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
                    }
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

