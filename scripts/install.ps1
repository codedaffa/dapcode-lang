# Skrip Instalasi Otomatis DapCode untuk Windows PowerShell
# Penggunaan: irm https://raw.githubusercontent.com/dapcode/dapcode-lang/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host " [>>] Memulai Instalasi Otomatis Kompilator DapCode" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

$InstallDir = "$env:USERPROFILE\.dapcode\bin"
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$Repo = "dapcode/dapcode-lang"
$ReleaseApiUrl = "https://api.github.com/repos/$Repo/releases/latest"

Write-Host "==> Mencari versi rilis terbaru dari GitHub..." -ForegroundColor Yellow

$DownloadUrl = $null
$Tag = "latest"

try {
    $ReleaseInfo = Invoke-RestMethod -Uri $ReleaseApiUrl -Headers @{ "User-Agent" = "DapCode-Installer" }
    $Tag = $ReleaseInfo.tag_name
    $Asset = $ReleaseInfo.assets | Where-Object { $_.name -like "*windows-x64.zip" } | Select-Object -First 1
    if ($Asset) {
        $DownloadUrl = $Asset.browser_download_url
    }
} catch {
    Write-Host "(!) Catatan: Tidak dapat menjangkau GitHub Releases API, beralih ke fallback kompilasi/lokal." -ForegroundColor DarkYellow
}

$ZipPath = "$env:TEMP\dapcodec-windows.zip"
$TargetExe = "$InstallDir\dapcodec.exe"
$TargetPkg = "$InstallDir\dappkg.exe"

if ($DownloadUrl) {
    Write-Host "==> Mengunduh DapCode ($Tag) dari $DownloadUrl..." -ForegroundColor Green
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
    Expand-Archive -Path $ZipPath -DestinationPath "$env:TEMP\dapcodec_extracted" -Force
    
    $ExtractedExes = Get-ChildItem -Path "$env:TEMP\dapcodec_extracted" -Filter "*.exe" -Recurse
    foreach ($exe in $ExtractedExes) {
        Copy-Item $exe.FullName -Destination "$InstallDir\$($exe.Name)" -Force
    }
    Remove-Item -Path $ZipPath -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$env:TEMP\dapcodec_extracted" -Recurse -Force -ErrorAction SilentlyContinue
} else {
    # Fallback to local copy if run within repository
    if (Test-Path "target\release\dapcodec.exe") {
        Write-Host "==> Menyalin biner lokal dari target\release..." -ForegroundColor Green
        Copy-Item "target\release\dapcodec.exe" -Destination $TargetExe -Force
        if (Test-Path "target\release\dappkg.exe") {
            Copy-Item "target\release\dappkg.exe" -Destination $TargetPkg -Force
        }
    } else {
        Write-Host "(!) Mengompilasi dengan Cargo..." -ForegroundColor Yellow
        cargo build --release
        Copy-Item "target\release\dapcodec.exe" -Destination $TargetExe -Force
        if (Test-Path "target\release\dappkg.exe") {
            Copy-Item "target\release\dappkg.exe" -Destination $TargetPkg -Force
        }
    }
}

# Tambahkan ke User PATH jika belum ada
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "==> Menambahkan $InstallDir ke Windows User PATH..." -ForegroundColor Green
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstallDir", "User")
    $env:PATH = "$env:PATH;$InstallDir"
}

Write-Host ""
Write-Host "[OK] Selamat! DapCode & dappkg berhasil dipasang di komputer Anda." -ForegroundColor Green
Write-Host "Lokasi Kompilator: $TargetExe" -ForegroundColor Gray
Write-Host "Lokasi Manajer Paket: $TargetPkg" -ForegroundColor Gray
Write-Host ""
Write-Host "Coba jalankan perintah berikut di terminal baru Anda:" -ForegroundColor White
Write-Host "  dapcodec --version" -ForegroundColor Cyan
Write-Host "  dappkg --help" -ForegroundColor Cyan
Write-Host "  dapcodec init proyek_pertama" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
