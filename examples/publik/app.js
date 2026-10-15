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

