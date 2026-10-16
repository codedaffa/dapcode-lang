//! Modul Mobile & PWA Bundler untuk DapCode
//!
//! Mengimplementasikan Pendekatan 2: WebAssembly + PWA / Hybrid WebView Bundler.
//! Menghasilkan bundel aplikasi mobile mandiri:
//! - Web App Manifest (manifest.webmanifest)
//! - Offline Service Worker (sw.js)
//! - Shell PWA responsif ramah layar sentuh (index.html)
//! - Ikon aplikasi SVG & PNG
//! - Proyek template Android WebView siap rakit APK (android/)
//! - Server pengembangan lokal dengan QR Code terminal & akses Wi-Fi (mobile serve)

use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::path::Path;
use std::time::Duration;
use colored::*;

/// Menghasilkan bundel lengkap aplikasi Mobile (PWA & Android WebView Project)
pub fn build_mobile_bundle(
    wasm_bytes: &[u8],
    app_title: &str,
    output_dir: &Path,
) -> io::Result<()> {
    fs::create_dir_all(output_dir)?;

    let app_slug = app_title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "_");

    // 1. Tulis biner WebAssembly
    let wasm_file = output_dir.join("app.wasm");
    fs::write(&wasm_file, wasm_bytes)?;

    // 2. Tulis Ikon Aplikasi (SVG & PNG)
    let icon_svg = generate_app_icon_svg(app_title);
    fs::write(output_dir.join("icon.svg"), icon_svg)?;

    let png_192 = generate_png_icon(192);
    let png_512 = generate_png_icon(512);
    fs::write(output_dir.join("icon-192.png"), &png_192)?;
    fs::write(output_dir.join("icon-512.png"), &png_512)?;

    // 3. Tulis Web App Manifest (PWA)
    let manifest_content = generate_web_manifest(app_title, &app_slug);
    fs::write(output_dir.join("manifest.webmanifest"), manifest_content)?;

    // 4. Tulis Offline Service Worker (sw.js)
    let sw_content = generate_service_worker(&app_slug);
    fs::write(output_dir.join("sw.js"), sw_content)?;

    // 5. Tulis Shell HTML PWA Responsif Mobile
    let html_content = generate_mobile_html(app_title, "app.wasm");
    fs::write(output_dir.join("index.html"), html_content)?;

    // 6. Tulis Konfigurasi Capacitor (untuk pengembang yang menggunakan Capacitor/Cordova)
    let cap_content = generate_capacitor_config(app_title, &app_slug);
    fs::write(output_dir.join("capacitor.config.json"), cap_content)?;

    // 7. Bangun Template Proyek Android WebView (android/)
    build_android_template(output_dir, app_title, &app_slug, wasm_bytes)?;

    println!(
        "{} Bundel aplikasi mobile berhasil dibuat di: {}",
        "sukses:".green().bold(),
        output_dir.display().to_string().bold()
    );
    println!("  ├── 📄 index.html          (PWA Shell dengan Touch Screen & Canvas 2D)");
    println!("  ├── ⚡ app.wasm            (Bytecode WebAssembly: {} byte)", wasm_bytes.len());
    println!("  ├── 📱 manifest.webmanifest(Konfigurasi PWA Layar Penuh/Standalone)");
    println!("  ├── 🛡️  sw.js               (Service Worker untuk mode Offline 100%)");
    println!("  ├── 🎨 icon-192.png / 512  (Ikon resolusi tinggi Android & iOS)");
    println!("  ├── 📦 capacitor.config    (Konfigurasi ekspor Hybrid)");
    println!("  └── 🤖 android/            (Template Proyek Android Studio / Gradle APK)");

    Ok(())
}

/// Menghasilkan berkas Web App Manifest standar PWA
fn generate_web_manifest(title: &str, slug: &str) -> String {
    format!(
        r###"{{
  "name": "{}",
  "short_name": "{}",
  "description": "Aplikasi Mobile bertenaga DapCode WebAssembly",
  "start_url": "./index.html",
  "scope": "./",
  "display": "standalone",
  "orientation": "any",
  "background_color": "#090d16",
  "theme_color": "#0284c7",
  "icons": [
    {{
      "src": "icon-192.png",
      "sizes": "192x192",
      "type": "image/png"
    }},
    {{
      "src": "icon-512.png",
      "sizes": "512x512",
      "type": "image/png"
    }},
    {{
      "src": "icon-512.png",
      "sizes": "512x512",
      "type": "image/png",
      "purpose": "maskable"
    }},
    {{
      "src": "icon.svg",
      "sizes": "any",
      "type": "image/svg+xml"
    }}
  ]
}}
"###,
        title, slug
    )
}

/// Menghasilkan Service Worker (sw.js) untuk caching offline
fn generate_service_worker(slug: &str) -> String {
    format!(
        r#"// DapCode Mobile PWA Offline Service Worker ({})
const CACHE_NAME = 'dapcode-pwa-{}';
const ASSETS_TO_CACHE = [
  './',
  './index.html',
  './app.wasm',
  './manifest.webmanifest',
  './icon-192.png',
  './icon-512.png',
  './icon.svg'
];

self.addEventListener('install', (event) => {{
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {{
      console.log('[DapCode PWA SW] Menyimpan aset ke cache offline...');
      return cache.addAll(ASSETS_TO_CACHE);
    }}).then(() => self.skipWaiting())
  );
}});

self.addEventListener('activate', (event) => {{
  event.waitUntil(
    caches.keys().then((keys) => {{
      return Promise.all(
        keys.map((key) => {{
          if (key !== CACHE_NAME) {{
            console.log('[DapCode PWA SW] Menghapus cache lama:', key);
            return caches.delete(key);
          }}
        }})
      );
    }}).then(() => self.clients.claim())
  );
}});

self.addEventListener('fetch', (event) => {{
  event.respondWith(
    caches.match(event.request).then((cachedResponse) => {{
      if (cachedResponse) {{
        return cachedResponse;
      }}
      return fetch(event.request).catch(() => {{
        if (event.request.mode === 'navigate') {{
          return caches.match('./index.html');
        }}
      }});
    }})
  );
}});
"#,
        slug, slug
    )
}

/// Menghasilkan berkas capacitor.config.json
fn generate_capacitor_config(title: &str, slug: &str) -> String {
    format!(
        r#"{{
  "appId": "com.dapcode.{}",
  "appName": "{}",
  "webDir": ".",
  "bundledWebRuntime": false
}}
"#,
        slug, title
    )
}

/// Menghasilkan Ikon Aplikasi SVG Beresolusi Vektor Tinggi
fn generate_app_icon_svg(title: &str) -> String {
    let initial = title.chars().next().unwrap_or('D').to_uppercase();
    format!(
        r###"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <linearGradient id="bgGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#0f172a" />
      <stop offset="100%" stop-color="#0284c7" />
    </linearGradient>
    <linearGradient id="circleGrad" x1="0%" y1="100%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#38bdf8" />
      <stop offset="100%" stop-color="#818cf8" />
    </linearGradient>
    <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="15" result="blur" />
      <feComposite in="SourceGraphic" in2="blur" operator="over" />
    </filter>
  </defs>
  <!-- Background Rounded Rect -->
  <rect width="512" height="512" rx="128" fill="url(#bgGrad)" />
  <!-- Glowing Rings -->
  <circle cx="256" cy="256" r="170" fill="none" stroke="url(#circleGrad)" stroke-width="16" filter="url(#glow)" opacity="0.8" />
  <circle cx="256" cy="256" r="140" fill="none" stroke="#ffffff" stroke-width="6" opacity="0.3" stroke-dasharray="12 12" />
  <!-- Monogram -->
  <text x="256" y="325" font-family="-apple-system, BlinkMacSystemFont, sans-serif" font-size="200" font-weight="900" text-anchor="middle" fill="#ffffff" filter="url(#glow)">{}</text>
  <!-- Mini DapCode Badge -->
  <rect x="176" y="390" width="160" height="34" rx="17" fill="rgba(15, 23, 42, 0.75)" stroke="#38bdf8" stroke-width="2" />
  <text x="256" y="413" font-family="-apple-system, BlinkMacSystemFont, sans-serif" font-size="16" font-weight="700" text-anchor="middle" fill="#38bdf8" letter-spacing="2">DAPCODE</text>
</svg>
"###,
        initial
    )
}

/// Menghasilkan biner PNG sederhana (RGBA murni)
fn generate_png_icon(size: u32) -> Vec<u8> {
    let mut raw_rgba = Vec::with_capacity((size * (size * 4 + 1)) as usize);

    for y in 0..size {
        raw_rgba.push(0); // PNG Filter byte (0 = None)
        for x in 0..size {
            let dx = (x as f32) - (size as f32 / 2.0);
            let dy = (y as f32) - (size as f32 / 2.0);
            let dist = (dx * dx + dy * dy).sqrt();
            let radius = size as f32 * 0.44;

            if dist < radius {
                // Gradient Cyan to Indigo
                let t = (x + y) as f32 / (size * 2) as f32;
                let r = (14.0 + t * 40.0) as u8;
                let g = (132.0 + t * 30.0) as u8;
                let b = (235.0 - t * 20.0) as u8;
                raw_rgba.extend_from_slice(&[r, g, b, 255]);
            } else if dist < radius + 4.0 {
                // Antialiased border
                raw_rgba.extend_from_slice(&[56, 189, 248, 180]);
            } else {
                // Background dark blue
                raw_rgba.extend_from_slice(&[9, 13, 22, 255]);
            }
        }
    }

    create_png_bytes(size, size, &raw_rgba)
}

/// Pembungkus PNG RFC 2083 zero-dependency
fn create_png_bytes(width: u32, height: u32, uncompressed_data: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    // 1. Signature
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // 2. IHDR Chunk
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type (RGBA)
    ihdr.push(0); // compression method (deflate)
    ihdr.push(0); // filter method (standard)
    ihdr.push(0); // interlace method (none)
    write_chunk(&mut png, b"IHDR", &ihdr);

    // 3. IDAT Chunk (zlib stream with uncompressed blocks)
    let mut idat = Vec::new();
    idat.push(0x78); // zlib header CMF
    idat.push(0x01); // zlib header FLG (no compression)

    let chunk_size = 65535;
    let chunks: Vec<&[u8]> = uncompressed_data.chunks(chunk_size).collect();
    for (i, c) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        idat.push(if is_last { 0x01 } else { 0x00 });
        let len = c.len() as u16;
        idat.extend_from_slice(&len.to_le_bytes());
        idat.extend_from_slice(&(!len).to_le_bytes());
        idat.extend_from_slice(c);
    }

    // Adler-32 Checksum
    let adler = compute_adler32(uncompressed_data);
    idat.extend_from_slice(&adler.to_be_bytes());
    write_chunk(&mut png, b"IDAT", &idat);

    // 4. IEND Chunk
    write_chunk(&mut png, b"IEND", &[]);

    png
}

fn write_chunk(dest: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    dest.extend_from_slice(&(data.len() as u32).to_be_bytes());
    dest.extend_from_slice(chunk_type);
    dest.extend_from_slice(data);

    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = compute_crc32(&crc_data);
    dest.extend_from_slice(&crc.to_be_bytes());
}

fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ 0xEDB88320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn compute_adler32(data: &[u8]) -> u32 {
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

/// Menghasilkan antarmuka PWA HTML ramah layar sentuh ponsel
fn generate_mobile_html(title: &str, wasm_filename: &str) -> String {
    format!(
        r###"<!DOCTYPE html>
<html lang="id">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover">
  <title>{}</title>

  <!-- PWA Meta Tags -->
  <link rel="manifest" href="manifest.webmanifest">
  <meta name="theme-color" content="#0284c7">
  <meta name="mobile-web-app-capable" content="yes">
  <meta name="apple-mobile-web-app-capable" content="yes">
  <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
  <meta name="apple-mobile-web-app-title" content="{}">
  <link rel="apple-touch-icon" href="icon-192.png">

  <style>
    * {{
      box-sizing: border-box;
      margin: 0;
      padding: 0;
      -webkit-touch-callout: none;
      -webkit-user-select: none;
      user-select: none;
      touch-action: none;
    }}
    html, body {{
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: #090d16;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      color: #f8fafc;
      display: flex;
      flex-direction: column;
    }}
    #header {{
      height: 48px;
      background: rgba(15, 23, 42, 0.9);
      backdrop-filter: blur(8px);
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 16px;
      z-index: 10;
    }}
    .app-title {{
      font-weight: 700;
      font-size: 1rem;
      color: #38bdf8;
      display: flex;
      align-items: center;
      gap: 8px;
    }}
    .app-title span.dot {{
      width: 8px;
      height: 8px;
      background: #4ade80;
      border-radius: 50%;
      display: inline-block;
      box-shadow: 0 0 8px #4ade80;
    }}
    .btn-action {{
      background: rgba(56, 189, 248, 0.15);
      color: #38bdf8;
      border: 1px solid rgba(56, 189, 248, 0.4);
      padding: 6px 12px;
      border-radius: 20px;
      font-size: 0.8rem;
      font-weight: 600;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 4px;
    }}
    #canvas-container {{
      flex: 1;
      width: 100%;
      height: 100%;
      position: relative;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #000;
    }}
    canvas {{
      display: block;
      width: 100%;
      height: 100%;
      object-fit: contain;
    }}
    #install-banner {{
      position: fixed;
      bottom: 20px;
      left: 50%;
      transform: translateX(-50%);
      background: linear-gradient(135deg, #0284c7, #2563eb);
      color: white;
      padding: 10px 20px;
      border-radius: 30px;
      font-weight: 700;
      font-size: 0.9rem;
      box-shadow: 0 10px 30px rgba(2, 132, 199, 0.5);
      cursor: pointer;
      display: none;
      align-items: center;
      gap: 8px;
      z-index: 100;
      border: 1px solid rgba(255, 255, 255, 0.3);
      animation: pulse 2s infinite;
    }}
    @keyframes pulse {{
      0%, 100% {{ transform: translateX(-50%) scale(1); }}
      50% {{ transform: translateX(-50%) scale(1.04); }}
    }}
    #fps-meter {{
      font-family: monospace;
      font-size: 0.75rem;
      color: #94a3b8;
    }}
  </style>
</head>
<body>
  <div id="header">
    <div class="app-title">
      <span class="dot"></span>
      <span>{}</span>
    </div>
    <div style="display: flex; gap: 8px; align-items: center;">
      <span id="fps-meter">60 FPS</span>
      <button id="btn-fullscreen" class="btn-action">⛶ Layar Penuh</button>
    </div>
  </div>

  <div id="canvas-container">
    <canvas id="canvas" width="800" height="600"></canvas>
  </div>

  <div id="install-banner">
    📲 Pasang Aplikasi (Install App)
  </div>

  <script>
    // 1. Registrasi PWA Service Worker
    if ('serviceWorker' in navigator) {{
      window.addEventListener('load', () => {{
        navigator.serviceWorker.register('./sw.js').then((reg) => {{
          console.log('[DapCode PWA] Service Worker terdaftar:', reg.scope);
        }}).catch((err) => {{
          console.warn('[DapCode PWA] Service Worker gagal:', err);
        }});
      }});
    }}

    // 2. Banner Pasang Aplikasi PWA
    let deferredPrompt = null;
    const installBanner = document.getElementById('install-banner');
    window.addEventListener('beforeinstallprompt', (e) => {{
      e.preventDefault();
      deferredPrompt = e;
      installBanner.style.display = 'flex';
    }});
    installBanner.addEventListener('click', async () => {{
      if (deferredPrompt) {{
        deferredPrompt.prompt();
        const {{ outcome }} = await deferredPrompt.userChoice;
        if (outcome === 'accepted') {{
          installBanner.style.display = 'none';
        }}
        deferredPrompt = null;
      }}
    }});

    // 3. Tombol Layar Penuh
    document.getElementById('btn-fullscreen').addEventListener('click', () => {{
      if (!document.fullscreenElement) {{
        document.documentElement.requestFullscreen().catch(() => {{}});
      }} else {{
        document.exitFullscreen().catch(() => {{}});
      }}
    }});

    // 4. Inisialisasi Kanvas & Sistem Input Sentuh (Touch)
    const canvas = document.getElementById('canvas');
    const ctx = canvas.getContext('2d');

    const dapInput = {{
      mouseX: 400.0,
      mouseY: 300.0,
      mouseClick: 0n,
      keys: new Set(),
      lastFrameTime: performance.now(),
      deltaTime: 0.016,
      fps: 60,
    }};

    function updateTouch(e, isClick) {{
      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;
      const touch = e.touches[0] || (e.changedTouches && e.changedTouches[0]);
      if (touch) {{
        dapInput.mouseX = (touch.clientX - rect.left) * scaleX;
        dapInput.mouseY = (touch.clientY - rect.top) * scaleY;
      }}
      if (isClick !== undefined) {{
        dapInput.mouseClick = isClick ? 1n : 0n;
      }}
      resumeAudio();
      e.preventDefault();
    }}

    canvas.addEventListener('touchstart', (e) => updateTouch(e, true), {{ passive: false }});
    canvas.addEventListener('touchmove', (e) => updateTouch(e), {{ passive: false }});
    canvas.addEventListener('touchend', (e) => updateTouch(e, false), {{ passive: false }});
    canvas.addEventListener('touchcancel', (e) => updateTouch(e, false), {{ passive: false }});

    canvas.addEventListener('mousemove', (e) => {{
      const rect = canvas.getBoundingClientRect();
      dapInput.mouseX = (e.clientX - rect.left) * (canvas.width / rect.width);
      dapInput.mouseY = (e.clientY - rect.top) * (canvas.height / rect.height);
    }});
    canvas.addEventListener('mousedown', () => {{ dapInput.mouseClick = 1n; resumeAudio(); }});
    canvas.addEventListener('mouseup', () => {{ dapInput.mouseClick = 0n; }});

    window.addEventListener('keydown', (e) => dapInput.keys.add(e.key.toLowerCase()));
    window.addEventListener('keyup', (e) => dapInput.keys.delete(e.key.toLowerCase()));

    // 5. Audio Web Synthesizer
    let audioCtx = null;
    function resumeAudio() {{
      if (!audioCtx) {{
        audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      }}
      if (audioCtx.state === 'suspended') {{
        audioCtx.resume();
      }}
    }}

    // 6. Muat & Jalankan Modul WebAssembly
    let wasmInstance = null;
    let wasmMemory = null;

    async function initMobileWasm() {{
      try {{
        const resp = await fetch('{}');
        const bytes = await resp.arrayBuffer();

        const importObject = {{
          env: {{
            cetak_u64: (val) => console.log('[DapCode]', val.toString()),
            cetak_f64: (val) => console.log('[DapCode]', val),
            cetak_teks: (ptr, len) => {{
              const arr = new Uint8Array(wasmMemory.buffer, ptr, len);
              console.log('[DapCode]', new TextDecoder().decode(arr));
            }},
            sin: Math.sin, cos: Math.cos, tan: Math.tan,
            asin: Math.asin, acos: Math.acos, atan: Math.atan, atan2: Math.atan2,
            akar_kubik: Math.cbrt, pangkat: Math.pow, log: Math.log, log10: Math.log10, log2: Math.log2,
            exp: Math.exp, tanda: Math.sign,
            kanvas_bersihkan: (r, g, b, a) => {{
              ctx.fillStyle = `rgba(${{Math.floor(r * 255)}}, ${{Math.floor(g * 255)}}, ${{Math.floor(b * 255)}}, ${{a}})`;
              ctx.fillRect(0, 0, canvas.width, canvas.height);
            }},
            kanvas_warna_isi: (r, g, b, a) => {{
              ctx.fillStyle = `rgba(${{Math.floor(r * 255)}}, ${{Math.floor(g * 255)}}, ${{Math.floor(b * 255)}}, ${{a}})`;
            }},
            kanvas_warna_garis: (r, g, b, a) => {{
              ctx.strokeStyle = `rgba(${{Math.floor(r * 255)}}, ${{Math.floor(g * 255)}}, ${{Math.floor(b * 255)}}, ${{a}})`;
            }},
            kanvas_tebal_garis: (tebal) => {{ ctx.lineWidth = tebal; }},
            kanvas_persegi: (x, y, w, h) => ctx.fillRect(x, y, w, h),
            kanvas_bingkai_persegi: (x, y, w, h) => ctx.strokeRect(x, y, w, h),
            kanvas_lingkaran: (x, y, rad) => {{
              ctx.beginPath(); ctx.arc(x, y, Math.max(0, rad), 0, Math.PI * 2); ctx.fill();
            }},
            kanvas_bingkai_lingkaran: (x, y, rad) => {{
              ctx.beginPath(); ctx.arc(x, y, Math.max(0, rad), 0, Math.PI * 2); ctx.stroke();
            }},
            kanvas_garis: (x1, y1, x2, y2) => {{
              ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
            }},
            kanvas_titik: (x, y) => ctx.fillRect(x, y, 1, 1),
            kanvas_teks: (ptr, len, x, y) => {{
              const arr = new Uint8Array(wasmMemory.buffer, ptr, len);
              const txt = new TextDecoder().decode(arr);
              ctx.font = '16px -apple-system, sans-serif';
              ctx.fillText(txt, x, y);
            }},
            kanvas_lebar: () => canvas.width,
            kanvas_tinggi: () => canvas.height,
            kanvas_mouse_x: () => dapInput.mouseX,
            kanvas_mouse_y: () => dapInput.mouseY,
            kanvas_mouse_klik: () => dapInput.mouseClick,
            kanvas_tombol_ditekan: (ptr, len) => {{
              const arr = new Uint8Array(wasmMemory.buffer, ptr, len);
              const k = new TextDecoder().decode(arr).toLowerCase();
              return dapInput.keys.has(k) ? 1n : 0n;
            }},
            kanvas_delta_waktu: () => dapInput.deltaTime,
            waktu_sekarang_ms: () => BigInt(Date.now()),
            audio_mainkan_nada: (freq, dur, vol) => {{
              try {{
                resumeAudio();
                const now = audioCtx.currentTime;
                const osc = audioCtx.createOscillator();
                const gain = audioCtx.createGain();
                osc.type = 'sine';
                osc.frequency.setValueAtTime(freq, now);
                gain.gain.setValueAtTime(0.0001, now);
                gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                osc.connect(gain); gain.connect(audioCtx.destination);
                osc.start(now); osc.stop(now + dur + 0.05);
              }} catch(e) {{}}
            }},
            audio_mainkan_gelombang: (tipe, freq, dur, vol) => {{
              try {{
                resumeAudio();
                const now = audioCtx.currentTime;
                const osc = audioCtx.createOscillator();
                const gain = audioCtx.createGain();
                const types = ['sine', 'square', 'sawtooth', 'triangle'];
                osc.type = types[Number(tipe) % 4] || 'sine';
                osc.frequency.setValueAtTime(freq, now);
                gain.gain.setValueAtTime(0.0001, now);
                gain.gain.exponentialRampToValueAtTime(Math.max(0.0001, Math.min(1.0, vol)), now + 0.02);
                gain.gain.exponentialRampToValueAtTime(0.0001, now + dur);
                osc.connect(gain); gain.connect(audioCtx.destination);
                osc.start(now); osc.stop(now + dur + 0.05);
              }} catch(e) {{}}
            }},
            audio_efek_suara: (tipe) => {{
              try {{
                resumeAudio();
                const now = audioCtx.currentTime;
                const osc = audioCtx.createOscillator();
                const gain = audioCtx.createGain();
                osc.connect(gain); gain.connect(audioCtx.destination);
                const t = Number(tipe);
                if (t === 0) {{
                  osc.type = 'sine'; osc.frequency.setValueAtTime(1000, now);
                  gain.gain.setValueAtTime(0.3, now); gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.08);
                  osc.start(now); osc.stop(now + 0.09);
                }} else if (t === 1) {{
                  osc.type = 'square'; osc.frequency.setValueAtTime(150, now);
                  osc.frequency.exponentialRampToValueAtTime(600, now + 0.15);
                  gain.gain.setValueAtTime(0.25, now); gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);
                  osc.start(now); osc.stop(now + 0.17);
                }} else if (t === 2) {{
                  osc.type = 'sine'; osc.frequency.setValueAtTime(987.77, now);
                  osc.frequency.setValueAtTime(1318.51, now + 0.08);
                  gain.gain.setValueAtTime(0.3, now); gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.35);
                  osc.start(now); osc.stop(now + 0.36);
                }} else if (t === 3) {{
                  osc.type = 'sawtooth'; osc.frequency.setValueAtTime(220, now);
                  osc.frequency.exponentialRampToValueAtTime(30, now + 0.35);
                  gain.gain.setValueAtTime(0.4, now); gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.38);
                  osc.start(now); osc.stop(now + 0.39);
                }}
              }} catch(e) {{}}
            }},
            audio_frekuensi_nada: (ptr, len) => 440.0,
            audio_set_volume: () => {{}},
            audio_hentikan_semua: () => {{ if (audioCtx) audioCtx.close(); audioCtx = null; }},
          }}
        }};

        const {{ instance }} = await WebAssembly.instantiate(bytes, importObject);
        wasmInstance = instance;
        wasmMemory = instance.exports.memory;

        console.log('[DapCode Mobile] WebAssembly siap dieksekusi.');

        if (instance.exports.utama) {{
          instance.exports.utama();
        }}

        // Cari fungsi siklus render animasi grafis
        const loopFn = instance.exports.render_game ||
                       instance.exports.gambar_tata_surya ||
                       instance.exports.perbarui_layar ||
                       instance.exports.render ||
                       instance.exports.gambar;

        let frameCount = 0;
        let lastFpsUpdate = performance.now();

        function mainLoop(now) {{
          dapInput.deltaTime = Math.min(0.1, (now - dapInput.lastFrameTime) / 1000);
          dapInput.lastFrameTime = now;

          frameCount++;
          if (now - lastFpsUpdate >= 1000) {{
            dapInput.fps = frameCount;
            document.getElementById('fps-meter').textContent = `${{frameCount}} FPS`;
            frameCount = 0;
            lastFpsUpdate = now;
          }}

          if (loopFn) {{
            loopFn();
          }}

          requestAnimationFrame(mainLoop);
        }}

        requestAnimationFrame(mainLoop);
      }} catch (err) {{
        console.error('[DapCode Mobile] Gagal menginisialisasi WASM:', err);
      }}
    }}

    initMobileWasm();
  </script>
</body>
</html>
"###,
        title, title, title, wasm_filename
    )
}

/// Membangun template proyek Android WebView mandiri (siap APK)
fn build_android_template(
    base_dir: &Path,
    title: &str,
    slug: &str,
    wasm_bytes: &[u8],
) -> io::Result<()> {
    let android_dir = base_dir.join("android");
    let app_dir = android_dir.join("app");
    let main_java_dir = app_dir.join("src").join("main").join("java").join("com").join("dapcode").join(slug);
    let assets_dir = app_dir.join("src").join("main").join("assets");

    fs::create_dir_all(&main_java_dir)?;
    fs::create_dir_all(&assets_dir)?;

    // Salin berkas WebAssembly & PWA ke folder assets Android
    fs::write(assets_dir.join("app.wasm"), wasm_bytes)?;
    fs::write(assets_dir.join("index.html"), generate_mobile_html(title, "app.wasm"))?;
    fs::write(assets_dir.join("manifest.webmanifest"), generate_web_manifest(title, slug))?;

    // 1. settings.gradle
    fs::write(
        android_dir.join("settings.gradle"),
        format!("rootProject.name = \"DapCode_{}\"\ninclude ':app'\n", slug),
    )?;

    // 2. build.gradle (Root)
    fs::write(
        android_dir.join("build.gradle"),
        r#"buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath 'com.android.tools.build:gradle:8.2.0'
    }
}
allprojects {
    repositories {
        google()
        mavenCentral()
    }
}
"#,
    )?;

    // 3. app/build.gradle
    fs::write(
        app_dir.join("build.gradle"),
        format!(
            r#"plugins {{
    id 'com.android.application'
}}

android {{
    namespace 'com.dapcode.{}'
    compileSdk 34

    defaultConfig {{
        applicationId "com.dapcode.{}"
        minSdk 21
        targetSdk 34
        versionCode 1
        versionName "1.0.0"
    }}

    buildTypes {{
        release {{
            minifyEnabled false
            proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
        }}
    }}
}}

dependencies {{
    implementation 'androidx.appcompat:appcompat:1.6.1'
}}
"#,
            slug, slug
        ),
    )?;

    // 4. AndroidManifest.xml
    let manifest_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <application
        android:allowBackup="true"
        android:label="{}"
        android:supportsRtl="true"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen"
        android:hardwareAccelerated="true">
        <activity
            android:name=".MainActivity"
            android:exported="true"
            android:configChanges="orientation|screenSize|keyboardHidden">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#,
        title
    );
    fs::write(app_dir.join("src").join("main").join("AndroidManifest.xml"), manifest_xml)?;

    // 5. MainActivity.java
    let main_activity = format!(
        r#"package com.dapcode.{};

import android.app.Activity;
import android.os.Bundle;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

public class MainActivity extends Activity {{
    private WebView webView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {{
        super.onCreate(savedInstanceState);

        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().setFlags(
            WindowManager.LayoutParams.FLAG_FULLSCREEN,
            WindowManager.LayoutParams.FLAG_FULLSCREEN
        );

        webView = new WebView(this);
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(true);
        settings.setAllowContentAccess(true);
        settings.setMediaPlaybackRequiresUserGesture(false);

        webView.setWebViewClient(new WebViewClient());
        webView.setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
            | View.SYSTEM_UI_FLAG_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
        );

        webView.loadUrl("file:///android_asset/index.html");
        setContentView(webView);
    }}

    @Override
    public void onBackPressed() {{
        if (webView != null && webView.canGoBack()) {{
            webView.goBack();
        }} else {{
            super.onBackPressed();
        }}
    }}
}}
"#,
        slug
    );
    fs::write(main_java_dir.join("MainActivity.java"), main_activity)?;

    // 6. README.md Panduan Perakitan APK
    let readme_content = format!(
        r#"# Proyek Android WebView DapCode - {}

Proyek ini dibuat secara otomatis oleh kompilator **DapCode Mobile Bundler**.

## Cara Membuat APK Android:

### Opsi 1: Menggunakan Android Studio (Paling Mudah)
1. Buka **Android Studio**.
2. Pilih **Open Project** dan arahkan ke folder ini (`{}`).
3. Tunggu Gradle Sync selesai.
4. Klik menu **Build > Build Bundle(s) / APK(s) > Build APK(s)**.
5. Berkas APK biner siap diinstal di ponsel Android Anda!

### Opsi 2: Menggunakan Terminal / Command Line
Jalankan perintah berikut di direktori ini:
```bash
./gradlew assembleDebug
```
Berkas APK akan berada di `app/build/outputs/apk/debug/app-debug.apk`.
"#,
        title, android_dir.display()
    );
    fs::write(android_dir.join("README.md"), readme_content)?;

    Ok(())
}

/// Mengambil alamat IP lokal (LAN) aktif
pub fn get_local_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Menampilkan QR Code versi 2 Alphanumeric/Byte di terminal secara langsung
pub fn print_terminal_qr(text: &str) {
    let matrix = generate_qr_matrix(text);
    let size = matrix.len();
    if size == 0 {
        return;
    }

    // Border atas
    print!("  ");
    for _ in 0..(size + 2) {
        print!("██");
    }
    println!();

    // Baris-baris modul
    for r in 0..size {
        print!("  ██");
        for c in 0..size {
            if matrix[r][c] {
                print!("  ");
            } else {
                print!("██");
            }
        }
        println!("██");
    }

    // Border bawah
    print!("  ");
    for _ in 0..(size + 2) {
        print!("██");
    }
    println!();
}

/// Generator QR Code sederhana (Versi 2: 25x25)
fn generate_qr_matrix(url: &str) -> Vec<Vec<bool>> {
    let size = 25;
    let mut grid = vec![vec![false; size]; size];
    let mut reserved = vec![vec![false; size]; size];

    // 1. Finder pattern 7x7
    let finders = [(0, 0), (size - 7, 0), (0, size - 7)];
    for &(fr, fc) in &finders {
        for r in 0..7 {
            for c in 0..7 {
                let is_black = r == 0 || r == 6 || c == 0 || c == 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
                grid[fr + r][fc + c] = is_black;
                reserved[fr + r][fc + c] = true;
            }
        }
        // Separators
        for r in 0..8 {
            for c in 0..8 {
                let gr = fr as i32 + r as i32;
                let gc = fc as i32 + c as i32;
                if gr >= 0 && gr < size as i32 && gc >= 0 && gc < size as i32 {
                    reserved[gr as usize][gc as usize] = true;
                }
            }
        }
    }

    // 2. Alignment pattern 5x5 at (18, 18) for Version 2
    let ar = 18;
    let ac = 18;
    for r in 0..5 {
        for c in 0..5 {
            let row = ar - 2 + r;
            let col = ac - 2 + c;
            let is_black = r == 0 || r == 4 || c == 0 || c == 4 || (r == 2 && c == 2);
            grid[row][col] = is_black;
            reserved[row][col] = true;
        }
    }

    // 3. Timing patterns
    for i in 8..(size - 8) {
        grid[6][i] = (i % 2) == 0;
        reserved[6][i] = true;
        grid[i][6] = (i % 2) == 0;
        reserved[i][6] = true;
    }

    // 4. Dark module
    grid[8][17] = true;
    reserved[8][17] = true;

    // 5. Format info dummy (Level L, Mask 0)
    let format_bits = 0b111011111000100u16;
    let mut f_idx = 0;
    for i in 0..9 {
        if i != 6 {
            let bit = ((format_bits >> f_idx) & 1) == 1;
            grid[8][i] = bit;
            grid[size - 1 - i][8] = bit;
            reserved[8][i] = true;
            reserved[size - 1 - i][8] = true;
            f_idx += 1;
        }
    }

    // 6. Data stream encode
    let mut bits = Vec::new();
    // Mode indicator: 8-bit byte (0100)
    bits.extend_from_slice(&[false, true, false, false]);
    // Character count (8 bits for V2)
    let len = url.len().min(32) as u8;
    for i in (0..8).rev() {
        bits.push(((len >> i) & 1) == 1);
    }
    // Data bytes
    for &b in &url.as_bytes()[..len as usize] {
        for i in (0..8).rev() {
            bits.push(((b >> i) & 1) == 1);
        }
    }
    // Terminator
    for _ in 0..4 {
        if bits.len() < 34 * 8 {
            bits.push(false);
        }
    }
    // Pad to multiple of 8
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    // Pad bytes 0xEC, 0x11
    let pad_bytes = [0xEC, 0x11];
    let mut pad_idx = 0;
    while bits.len() < 34 * 8 {
        let p = pad_bytes[pad_idx % 2];
        for i in (0..8).rev() {
            bits.push(((p >> i) & 1) == 1);
        }
        pad_idx += 1;
    }

    // Reed Solomon Error Correction (10 bytes)
    let mut data_bytes = Vec::with_capacity(34);
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for &bit in chunk {
            byte = (byte << 1) | (if bit { 1 } else { 0 });
        }
        data_bytes.push(byte);
    }
    let ec_bytes = compute_reed_solomon_10(&data_bytes);
    for &b in &ec_bytes {
        for i in (0..8).rev() {
            bits.push(((b >> i) & 1) == 1);
        }
    }

    // 7. Place data into grid (zigzag from bottom-right)
    let mut bit_idx = 0;
    let mut upward = true;
    let mut c = (size - 1) as i32;
    while c > 0 {
        if c == 6 {
            c -= 1; // Skip vertical timing pattern
        }
        let rows: Vec<usize> = if upward {
            (0..size).rev().collect()
        } else {
            (0..size).collect()
        };

        for r in rows {
            for col_offset in 0..2 {
                let curr_c = (c - col_offset) as usize;
                if !reserved[r][curr_c] {
                    let bit = if bit_idx < bits.len() {
                        bits[bit_idx]
                    } else {
                        false
                    };
                    bit_idx += 1;
                    // Apply Mask 0: (row + col) % 2 == 0
                    let masked_bit = if (r + curr_c) % 2 == 0 { !bit } else { bit };
                    grid[r][curr_c] = masked_bit;
                }
            }
        }
        upward = !upward;
        c -= 2;
    }

    grid
}

/// Perhitungan Reed-Solomon untuk 10 EC Codewords (GF(256))
fn compute_reed_solomon_10(data: &[u8]) -> Vec<u8> {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x = 1u16;
    for i in 0..255 {
        exp[i] = x as u8;
        exp[i + 255] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if (x & 0x100) != 0 {
            x ^= 0x11D; // Primitive polynomial
        }
    }

    // Generator polynomial for 10 EC codewords:
    // g(x) = (x - a^0)(x - a^1)...(x - a^9)
    let rs_gen = [0, 251, 67, 46, 61, 118, 70, 64, 94, 32, 45]; // log values
    let mut ec = vec![0u8; 10];

    for &b in data {
        let factor = b ^ ec[0];
        ec.copy_within(1..10, 0);
        ec[9] = 0;
        if factor != 0 {
            let log_f = log[factor as usize] as usize;
            for j in 0..10 {
                let log_g = rs_gen[j + 1] as usize;
                ec[j] ^= exp[(log_f + log_g) % 255];
            }
        }
    }

    ec
}

/// Menjalankan server lokal pengembangan mobile dengan QR Code & Wi-Fi
pub fn cmd_mobile_serve(file: Option<&Path>, port: u16, open: bool) -> io::Result<()> {
    let target = crate::cli::resolve_target_file(file)?;
    let mut sm = crate::span::SourceMap::new();
    let (file_id, mut program) = match crate::cli::load_program_and_modules(&target, &mut sm) {
        Some(res) => res,
        None => return Ok(()),
    };

    let mut comptime = crate::comptime::ComptimeEvaluator::new();
    let _ = comptime.fold_program(&mut program);

    let mut checker = crate::types::TypeChecker::new(file_id);
    if let Err(diag) = checker.check_program(&program) {
        eprint!("{}", diag.render(&sm));
        return Ok(());
    }

    let mut emitter = crate::wasm::WasmEmitter::new();
    let wasm_bytes = match emitter.compile_program(&program) {
        Ok(bytes) => bytes,
        Err(diag) => {
            eprint!("{}", diag.render(&sm));
            return Ok(());
        }
    };

    let app_stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("dapcode_app");
    let temp_mobile_dir = target.parent().unwrap_or(Path::new(".")).join(format!("{}_mobile", app_stem));

    build_mobile_bundle(&wasm_bytes, app_stem, &temp_mobile_dir)?;

    let local_ip = get_local_ip();
    let local_url = format!("http://localhost:{}", port);
    let lan_url = format!("http://{}:{}", local_ip, port);

    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║             📱 DAPCODE MOBILE LIVE DEV SERVER (PWA)                  ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║  💻 Komputer Desktop  : {:<44} ║", local_url);
    println!("║  📲 Ponsel HP (Wi-Fi) : {:<44} ║", lan_url.bold().green());
    println!("║  ⚡ Fitur PWA         : Standalone, Touch Screen, Offline ServiceWorker║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("  📷 {}", "Pindai QR Code di bawah dengan Kamera HP / Google Lens:".bold().cyan());
    println!();

    print_terminal_qr(&lan_url);

    println!();
    println!("💡 {}", "Tips Ponsel:".bold().yellow());
    println!("   • Di Chrome Android: Buka link lalu ketuk tombol 'Pasang Aplikasi' / titik 3 > 'Install App'.");
    println!("   • Di Safari iOS: Ketuk tombol 'Bagikan (Share)' lalu pilih 'Tambah ke Layar Utama'.");
    println!("   Tekan Ctrl+C untuk menghentikan server.\n");

    if open {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/c", "start", "", &local_url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&local_url).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&local_url).spawn();
    }

    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Gagal mengikat ke {}: {}", bind_addr, e)))?;

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };

        let req = match crate::net::parse_http_request(&buf[..n]) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let raw_path = if req.path == "/" { "/index.html" } else { &req.path };
        let rel_path = raw_path.trim_start_matches('/');
        let file_path = temp_mobile_dir.join(rel_path);

        if file_path.exists() && file_path.is_file() {
            if let Ok(content_bytes) = fs::read(&file_path) {
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let mime = match ext {
                    "html" => "text/html; charset=utf-8",
                    "js" => "application/javascript; charset=utf-8",
                    "wasm" => "application/wasm",
                    "webmanifest" | "json" => "application/manifest+json; charset=utf-8",
                    "png" => "image/png",
                    "svg" => "image/svg+xml",
                    _ => "application/octet-stream",
                };

                let resp = crate::net::HttpServerResponse {
                    status: 200,
                    content_type: mime.to_string(),
                    body: String::new(),
                    raw_body: Some(content_bytes),
                    headers: [
                        ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
                        ("Cache-Control".to_string(), "no-cache".to_string()),
                    ].into_iter().collect(),
                };

                let _ = stream.write_all(&crate::net::format_http_response(&resp));
                continue;
            }
        }

        let resp_404 = crate::net::HttpServerResponse {
            status: 404,
            content_type: "text/html; charset=utf-8".to_string(),
            body: "<h1>404 Tidak Ditemukan</h1><p>DapCode Mobile Server</p>".to_string(),
            raw_body: None,
            headers: std::collections::HashMap::new(),
        };
        let _ = stream.write_all(&crate::net::format_http_response(&resp_404));
    }

    Ok(())
}

/// Menginisialisasi proyek baru dengan template aplikasi mobile
pub fn cmd_mobile_init(name: Option<&str>) -> io::Result<()> {
    let app_name = name.unwrap_or("app_mobile");
    let dir = Path::new(app_name);
    fs::create_dir_all(dir.join("src"))?;

    // 1. DapCode.toml
    let toml_content = format!(
        r#"[paket]
nama = "{}"
versi = "0.1.0"
penulis = ["Pengembang DapCode"]
deskripsi = "Aplikasi Mobile Interaktif Layar Sentuh"

[target]
tipe = "mobile"
entry = "src/utama.dap"
"#,
        app_name
    );
    fs::write(dir.join("DapCode.toml"), toml_content)?;

    // 2. src/utama.dap
    let dap_template = r#"// Aplikasi Mobile Interaktif Layar Sentuh DapCode
// Mendukung sentuhan jari pada layar ponsel (touch screen) & akselerasi hardware

fungsi render() -> Unit {
    w := kanvas_lebar()
    h := kanvas_tinggi()

    // 1. Bersihkan latar belakang dengan tema gelap elegan
    kanvas_bersihkan(0.04, 0.06, 0.1, 1.0)

    // 2. Deteksi posisi sentuhan jari / kursor
    sx := kanvas_mouse_x()
    sy := kanvas_mouse_y()
    klik := kanvas_mouse_klik()

    // 3. Efek visual saat layar disentuh
    jika klik == 1 {
        // Layar sedang disentuh: Lingkaran pink menyala & efek suara koin
        kanvas_warna_isi(0.95, 0.35, 0.65, 0.9)
        kanvas_lingkaran(sx, sy, 70.0)
        kanvas_tebal_garis(3.0)
        kanvas_warna_garis(1.0, 1.0, 1.0, 0.8)
        kanvas_bingkai_lingkaran(sx, sy, 85.0)
        audio_efek_suara(2)
    } selain_itu {
        // Mode standby: Lingkaran cyan bercahaya
        kanvas_warna_isi(0.22, 0.74, 0.97, 0.8)
        kanvas_lingkaran(sx, sy, 50.0)
        kanvas_tebal_garis(2.0)
        kanvas_warna_garis(1.0, 1.0, 1.0, 0.5)
        kanvas_bingkai_lingkaran(sx, sy, 62.0)
    }

    // 4. Teks petunjuk mobile
    kanvas_warna_isi(1.0, 1.0, 1.0, 0.95)
    kanvas_teks("📱 Sentuh layar ponsel Anda!", (w / 2.0) - 100.0, 50.0, 14.0)
}

fungsi utama() -> u64 {
    render()
    kembali 1
}
"#;
    fs::write(dir.join("src").join("utama.dap"), dap_template)?;

    println!(
        "{} Proyek aplikasi mobile '{}' berhasil dibuat!",
        "sukses:".green().bold(),
        app_name.bold()
    );
    println!("Langkah selanjutnya:");
    println!("  1. cd {}", app_name);
    println!("  2. dapcodec mobile serve src/utama.dap   # Untuk uji langsung di HP via QR Code");
    println!("  3. dapcodec mobile build src/utama.dap   # Untuk mengekspor PWA & Android Bundle");

    Ok(())
}
