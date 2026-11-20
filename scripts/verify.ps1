# One command to verify the workspace: tests, format, and all parity harnesses.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path $cargo)) { Write-Error "cargo not found; see vendor/tools/NOTES.md"; exit 127 }

$pyCmd = (Get-Command py -ErrorAction SilentlyContinue).Source
if (-not $pyCmd) { $pyCmd = (Get-Command python -ErrorAction SilentlyContinue).Source }
if (-not $pyCmd) { Write-Error "python (py) not found"; exit 127 }

$failed = 0

Write-Host "== cargo test --workspace =="
& $cargo test --workspace
if ($LASTEXITCODE -ne 0) { $failed = 1 }

Write-Host "== cargo fmt --all -- --check =="
& $cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { $failed = 1 }

Write-Host "== parity harnesses =="
& $pyCmd (Join-Path $root 'tools\parity.py')
if ($LASTEXITCODE -ne 0) { $failed = 1 }

if ($failed -eq 0) { Write-Host "`nALL OK" } else { Write-Host "`nFAILED" }
exit $failed
