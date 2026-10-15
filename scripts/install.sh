#!/usr/bin/env bash
# Skrip Instalasi Otomatis DapCode untuk Linux & macOS
# Penggunaan: curl -fsSL https://raw.githubusercontent.com/dapcode/dapcode-lang/master/scripts/install.sh | bash

set -e

echo "=================================================="
echo " 🚀 Memulai Instalasi Otomatis Kompilator DapCode"
echo "=================================================="

INSTALL_DIR="$HOME/.dapcode/bin"
mkdir -p "$INSTALL_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    PLATFORM="linux"
    ;;
  Darwin)
    PLATFORM="macos"
    ;;
  *)
    echo "Kesalahan: Sistem operasi '$OS' tidak didukung oleh skrip ini."
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64)
    ARCH_NAME="x64"
    ;;
  arm64|aarch64)
    ARCH_NAME="arm64"
    ;;
  *)
    echo "Kesalahan: Arsitektur CPU '$ARCH' tidak didukung."
    exit 1
    ;;
esac

REPO="dapcode/dapcode-lang"
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"

echo "==> Mendeteksi platform: $PLATFORM-$ARCH_NAME..."

TARGET_FILE="dapcodec-$PLATFORM-$ARCH_NAME.tar.gz"
DOWNLOAD_URL=$(curl -sSL "$RELEASE_URL" | grep "browser_download_url" | grep "$PLATFORM" | grep "$ARCH_NAME" | cut -d '"' -f 4 | head -n 1 || true)

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

if [ -n "$DOWNLOAD_URL" ]; then
  echo "==> Mengunduh biner dari $DOWNLOAD_URL..."
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TARGET_FILE"
  tar -xzf "$TMP_DIR/$TARGET_FILE" -C "$TMP_DIR"
  [ -f "$TMP_DIR/dapcodec" ] && mv "$TMP_DIR/dapcodec" "$INSTALL_DIR/dapcodec"
  [ -f "$TMP_DIR/dappkg" ] && mv "$TMP_DIR/dappkg" "$INSTALL_DIR/dappkg"
else
  # Fallback: jika dijalankan di dalam repositori
  if [ -f "./target/release/dapcodec" ]; then
    echo "==> Menyalin biner lokal dari ./target/release..."
    cp "./target/release/dapcodec" "$INSTALL_DIR/dapcodec"
    [ -f "./target/release/dappkg" ] && cp "./target/release/dappkg" "$INSTALL_DIR/dappkg"
  elif command -v cargo >/dev/null 2>&1; then
    echo "(!) Mengompilasi dengan Cargo..."
    cargo build --release
    cp "./target/release/dapcodec" "$INSTALL_DIR/dapcodec"
    [ -f "./target/release/dappkg" ] && cp "./target/release/dappkg" "$INSTALL_DIR/dappkg"
  else
    echo "Kesalahan: Tidak dapat mengunduh biner rilis dan Cargo tidak ditemukan."
    exit 1
  fi
fi

chmod +x "$INSTALL_DIR/dapcodec"
[ -f "$INSTALL_DIR/dappkg" ] && chmod +x "$INSTALL_DIR/dappkg"
ln -sf "$INSTALL_DIR/dapcodec" "$INSTALL_DIR/dapcode"

# Konfigurasi PATH pada Shell Profile
SHELL_CONFIG=""
if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
  SHELL_CONFIG="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
  SHELL_CONFIG="$HOME/.bashrc"
elif [ -f "$HOME/.bash_profile" ]; then
  SHELL_CONFIG="$HOME/.bash_profile"
fi

if [ -n "$SHELL_CONFIG" ]; then
  if ! grep -q ".dapcode/bin" "$SHELL_CONFIG" 2>/dev/null; then
    echo "==> Menambahkan $INSTALL_DIR ke $SHELL_CONFIG..."
    echo '' >> "$SHELL_CONFIG"
    echo '# DapCode Programming Language' >> "$SHELL_CONFIG"
    echo 'export PATH="$HOME/.dapcode/bin:$PATH"' >> "$SHELL_CONFIG"
  fi
fi

echo ""
echo "🎉 Selamat! DapCode berhasil dipasang di komputer Anda."
echo "📁 Lokasi Biner: $INSTALL_DIR/dapcodec"
echo ""
echo "Jalankan perintah ini untuk memperbarui sesi terminal:"
if [ -n "$SHELL_CONFIG" ]; then
  echo "  source $SHELL_CONFIG"
fi
echo ""
echo "Lalu coba jalankan:"
echo "  dapcodec --version"
echo "  dapcodec --help"
echo "  dapcodec init proyek_pertama"
echo "=================================================="
