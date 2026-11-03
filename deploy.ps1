$ErrorActionPreference = 'Stop'
$srcApp = 'D:\Projects\capri-tauri\src-tauri\target\release\capri.exe'
$srcLauncher = 'D:\Projects\capri-tauri\launcher\target\release\capri-launcher.exe'
$srcNsis = 'D:\Projects\capri-tauri\src-tauri\target\release\bundle\nsis\Capri_0.5.0_x64-setup.exe'
$portableDir = 'D:\Projects\capri-tauri\release-portable'
$binDir = Join-Path $env:LOCALAPPDATA 'Capri\bin'

Write-Host "1. Stopping existing Capri processes..."
Get-Process -Name Capri*, capri* -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

Write-Host "2. Copying to release-portable..."
Copy-Item -Force $srcApp (Join-Path $portableDir 'CapriCore.exe')
Copy-Item -Force $srcLauncher (Join-Path $portableDir 'Capri.exe')
if (Test-Path $srcNsis) {
    Copy-Item -Force $srcNsis (Join-Path $portableDir 'Capri_0.5.0_Setup.exe')
}

Write-Host "3. Updating local Capri installation in AppData..."
Copy-Item -Force $srcApp (Join-Path $binDir 'CapriCore.exe')
Copy-Item -Force $srcLauncher (Join-Path $binDir 'Capri.exe')

Write-Host "4. Launching updated CapriCore..."
$coreExe = Join-Path $binDir 'CapriCore.exe'
Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{CommandLine = "`"$coreExe`""; CurrentDirectory = $binDir}
Start-Sleep -Seconds 2

Get-Process -Name CapriCore -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, StartTime

Write-Host "5. Creating updated portable zip..."
Start-Sleep -Seconds 2
$zipPath = Join-Path $portableDir 'Capri-Portable.zip'
if (Test-Path $zipPath) {
    Remove-Item -Force $zipPath
}
$zipFiles = @(
    (Join-Path $portableDir 'Capri.exe'),
    (Join-Path $portableDir 'CapriCore.exe'),
    (Join-Path $portableDir 'WebView2Loader.dll')
)
try {
    Compress-Archive -Path $zipFiles -DestinationPath $zipPath
    Write-Host "Zip created successfully."
} catch {
    Write-Host "Zip creation deferred: $_"
}
