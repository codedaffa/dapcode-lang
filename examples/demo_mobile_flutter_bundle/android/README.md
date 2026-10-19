# Proyek Android WebView DapCode - demo_mobile_flutter

Proyek ini dibuat secara otomatis oleh kompilator **DapCode Mobile Bundler**.

## Cara Membuat APK Android:

### Opsi 1: Menggunakan Android Studio (Paling Mudah)
1. Buka **Android Studio**.
2. Pilih **Open Project** dan arahkan ke folder ini (`examples/demo_mobile_flutter_bundle\android`).
3. Tunggu Gradle Sync selesai.
4. Klik menu **Build > Build Bundle(s) / APK(s) > Build APK(s)**.
5. Berkas APK biner siap diinstal di ponsel Android Anda!

### Opsi 2: Menggunakan Terminal / Command Line
Jalankan perintah berikut di direktori ini:
```bash
./gradlew assembleDebug
```
Berkas APK akan berada di `app/build/outputs/apk/debug/app-debug.apk`.
