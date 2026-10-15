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
});
