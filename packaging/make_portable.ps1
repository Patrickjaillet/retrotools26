# Builds the portable distribution of Retro Tools 2026: the release
# binaries + bundled resources/ tools + docs, with a `portable.txt` marker
# so retrotools_common::config::is_portable_mode() stores settings/cache
# next to the exe instead of the per-user profile. Zips the result.
#
# Usage (from the repo root):
#   cargo build --workspace --release
#   powershell -File packaging\make_portable.ps1

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$version = "0.1.3"
$stageDir = Join-Path $repoRoot "packaging\output\RetroTools2026-Portable-$version"
$zipPath = Join-Path $repoRoot "packaging\output\RetroTools2026-Portable-$version.zip"

if (Test-Path $stageDir) { Remove-Item -Recurse -Force $stageDir }
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "resources") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "docs") | Out-Null

Copy-Item (Join-Path $repoRoot "target\release\retrotools2026.exe") $stageDir
Copy-Item (Join-Path $repoRoot "target\release\retrotools-cli.exe") $stageDir
Copy-Item (Join-Path $repoRoot "resources\*.exe") (Join-Path $stageDir "resources")
Copy-Item (Join-Path $repoRoot "README.md") $stageDir
Copy-Item (Join-Path $repoRoot "LICENSE") $stageDir
Copy-Item (Join-Path $repoRoot "docs\CHANGELOG.md") (Join-Path $stageDir "docs")

# The portable-mode marker: its mere presence next to the exe is what
# switches retrotools_common::config to exe-relative paths.
Set-Content -Path (Join-Path $stageDir "portable.txt") -Value "Delete this file to switch back to per-user settings storage." -NoNewline

if (Test-Path $zipPath) { Remove-Item $zipPath }
Compress-Archive -Path "$stageDir\*" -DestinationPath $zipPath

Write-Output "Portable package: $zipPath"
