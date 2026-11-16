$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$zip = Join-Path $dist 'VoxoraRust-1.0.0-win-x64.zip'
$out = Join-Path $dist 'VoxoraRust-1.0.0-setup.exe'
$target = Join-Path $root 'target\setup'

if (-not (Test-Path $zip)) { throw "Portable zip bulunamadi: $zip" }

cargo build --release --manifest-path (Join-Path $root 'crates\setup\Cargo.toml') --target-dir $target
if ($LASTEXITCODE -ne 0) { throw "Setup derlenemedi." }

Copy-Item -LiteralPath (Join-Path $target 'release\voxora-setup.exe') -Destination $out -Force
$item = Get-Item -LiteralPath $out
$hash = (Get-FileHash -LiteralPath $out -Algorithm SHA256).Hash
Write-Host ("SETUP OK  {0}  {1} bytes" -f $out, $item.Length)
Write-Host ("SHA256   {0}" -f $hash)
