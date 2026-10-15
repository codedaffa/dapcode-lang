// Peluncur Node.js untuk Modul WebAssembly DapCode (uji_wasm.wasm)
const fs = require('fs');
const path = require('path');

const wasmBuffer = fs.readFileSync(path.join(__dirname, 'uji_wasm.wasm'));
let memory;

const importObject = {
    env: {
        cetak_u64: (val) => console.log(val.toString()),
        cetak_f64: (val) => console.log(val),
        cetak_teks: (ptr, len) => {
            const arr = new Uint8Array(memory.buffer, ptr, len);
            console.log(new TextDecoder().decode(arr));
        }
    }
};

WebAssembly.instantiate(wasmBuffer, importObject).then(({ instance }) => {
    memory = instance.exports.memory;
    console.log('[DapCode WASM] Modul uji_wasm berhasil dimuat.');
    if (instance.exports.utama) {
        const hasil = instance.exports.utama();
        if (hasil !== undefined) console.log('[DapCode WASM] Hasil utama():', hasil);
    }
}).catch(err => {
    console.error('[DapCode WASM] Gagal memuat modul:', err);
});
