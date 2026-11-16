param(
    [switch]$FromTemp,
    [string]$Dest = ''
)

$ErrorActionPreference = 'SilentlyContinue'

if (-not $FromTemp) {
    $dest = Split-Path -Parent $PSCommandPath
    $tmp = Join-Path $env:TEMP ('voxora-uninstall-' + [guid]::NewGuid().ToString('N') + '.ps1')
    Copy-Item -LiteralPath $PSCommandPath -Destination $tmp -Force
    Start-Process -FilePath 'powershell.exe' -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $tmp + '"'),
        '-FromTemp', '-Dest', ('"' + $dest + '"')
    ) -WindowStyle Hidden
    exit
}

if (-not $Dest) { exit 1 }
$dest = $Dest

Get-Process -Name voxora -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -like (Join-Path $dest '*') } |
    Stop-Process -Force -ErrorAction SilentlyContinue

$startLnk = Join-Path (Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs') 'Voxora Rust.lnk'
Remove-Item -LiteralPath $startLnk -Force -ErrorAction SilentlyContinue

$desktop = [Environment]::GetFolderPath('Desktop')
if ($desktop) {
    Remove-Item -LiteralPath (Join-Path $desktop 'Voxora Rust.lnk') -Force -ErrorAction SilentlyContinue
}

Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\VoxoraRust' -Recurse -Force -ErrorAction SilentlyContinue

Set-Location $env:TEMP
for ($i = 0; $i -lt 20; $i++) {
    Remove-Item -LiteralPath $dest -Recurse -Force -ErrorAction SilentlyContinue
    if (-not (Test-Path $dest)) { break }
    Start-Sleep -Milliseconds 300
}

Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
