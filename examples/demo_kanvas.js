// Peluncur Node.js untuk Modul WebAssembly DapCode (demo_kanvas.wasm)
const fs = require('fs');
const path = require('path');

const wasmBuffer = fs.readFileSync(path.join(__dirname, 'demo_kanvas.wasm'));
let memory;

const importObject = {
    env: {
        cetak_u64: (val) => console.log(val.toString()),
        cetak_f64: (val) => console.log(val),
        cetak_teks: (ptr, len) => {
            const arr = new Uint8Array(memory.buffer, ptr, len);
            console.log(new TextDecoder().decode(arr));
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

WebAssembly.instantiate(wasmBuffer, importObject).then(({ instance }) => {
    memory = instance.exports.memory;
    console.log('[DapCode WASM] Modul demo_kanvas berhasil dimuat.');
    if (instance.exports.utama) {
        const hasil = instance.exports.utama();
        if (hasil !== undefined) console.log('[DapCode WASM] Hasil utama():', hasil);
    }
}).catch(err => {
    console.error('[DapCode WASM] Gagal memuat modul:', err);
});
