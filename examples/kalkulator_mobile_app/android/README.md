# Proyek Android NDK Native GPU - kalkulator (Direct Rendering Engine)

Proyek ini dibangun menggunakan **Pendekatan ala Flutter** (Direct GPU Rendering):
- **100% Native ARM/ARM64 C11** tanpa WebView, tanpa DOM HTML.
- Render langsung via **OpenGL ES 2.0 / 3.0** hardware-accelerated.
- Kecepatan **60-120 FPS** dengan latensi sentuhan (touch latency) 0ms.

## Cara Perakitan APK Native:
1. Buka folder ini di **Android Studio**.
2. Pastikan **Android NDK & CMake** terpasang via SDK Manager.
3. Jalankan di Emulator atau Ponsel Android asli via tombol **Run** (Shift+F10).
4. Atau rakit lewat terminal:
   ```bash
   ./gradlew assembleDebug
   ```
