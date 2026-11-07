# Cookie Launcher — Windows paketi: dist\cookie-launcher-<version>-windows-x64.zip
#
# Kullanım (Windows'ta, PowerShell):
#   .\packaging\build-windows.ps1
#   .\packaging\build-windows.ps1 -SkipBuild      # derlemeyi atla, var olan exe'yi paketle
#
# Zip içeriği: cookie-launcher.exe + README.txt + ikon.
# Launcher tek dosya çalışır; Java'yı JAVA_HOME veya PATH üzerinden bulur.
[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$CargoToml = Join-Path $Root "Cargo.toml"
$Version = (Select-String -Path $CargoToml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
if (-not $Version) { throw "Sürüm Cargo.toml'dan okunamadı" }

$Name = "cookie-launcher-$Version-windows-x64"
$Dist = Join-Path $Root "dist"
$Stage = Join-Path $Dist $Name
$Exe = Join-Path $Root "target\release\cookie-launcher-gpui.exe"

if (-not $SkipBuild) {
    Write-Host "→ derleniyor (release)"
    Push-Location $Root
    try { cargo build --release }
    finally { Pop-Location }
}

if (-not (Test-Path $Exe)) { throw "Derlenmiş exe bulunamadı: $Exe" }

Write-Host "→ paketleniyor: dist\$Name.zip"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item $Exe (Join-Path $Stage "cookie-launcher.exe")
$Icon = Join-Path $Root "assets\cookie-logo.png"
if (Test-Path $Icon) { Copy-Item $Icon (Join-Path $Stage "cookie-launcher.png") }

@"
Cookie Launcher $Version — Windows (x64)
========================================

Çalıştırma: cookie-launcher.exe (kurulum gerekmez).

Gereksinimler:
  - Windows 10/11 x64, DirectX 11/12 uyumlu ekran kartı sürücüsü
  - Microsoft Visual C++ 2015-2022 Redistributable (x64) — exe dinamik CRT kullanır
  - Java 8-21 (JAVA_HOME veya PATH'te). Launcher sürüme uygun JVM'i kendisi seçer.
  - İnternet (ilk çalıştırmada sürüm manifesti ve oyun dosyaları indirilir).

Veriler: %USERPROFILE%\.minecraft ve %USERPROFILE%\.cookie_launcher
(COOKIE_MINECRAFT_DIR / COOKIE_LAUNCHER_DATA_DIR ile değiştirilebilir).

Minecraft, Mojang Synergies AB'nin ticari markasıdır; bu proje Mojang/Microsoft
ile bağlantılı veya onaylı değildir.
"@ | Set-Content -Encoding UTF8 (Join-Path $Stage "README.txt")

$Zip = Join-Path $Dist "$Name.zip"
# Sürüm sayfasına doğrudan indirilebilen tek dosyalık exe de bırakılır.
$Loose = Join-Path $Dist "$Name.exe"
Copy-Item $Exe $Loose -Force
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -CompressionLevel Optimal

Write-Host "✓ $Zip"
Write-Host "✓ $Loose"
Get-Item $Zip | ForEach-Object { Write-Host ("  {0:N1} MB" -f ($_.Length / 1MB)) }
