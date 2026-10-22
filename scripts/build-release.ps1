param([switch]$SkipBrowserTests)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\windows-env.ps1"
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    npm ci
    if ($LASTEXITCODE) { throw 'npm install failed' }
    cargo fmt --all --check
    if ($LASTEXITCODE) { throw 'Rust formatting failed' }
    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE) { throw 'Rust lint failed' }
    cargo test --workspace
    if ($LASTEXITCODE) { throw 'Rust tests failed' }
    cargo build -p opentoken --example focus-target
    if ($LASTEXITCODE) { throw 'Native focus test helper build failed' }
    npm run check
    if ($LASTEXITCODE) { throw 'Frontend checks failed' }
    if (!$SkipBrowserTests) {
        npm run test:e2e
        if ($LASTEXITCODE) { throw 'Browser tests failed' }
    }
    npm run desktop:build
    if ($LASTEXITCODE) { throw 'Desktop build failed' }
    New-Item artifacts -ItemType Directory -Force | Out-Null
    Copy-Item target\release\opentoken.exe artifacts\OpenToken.exe -Force
    Get-ChildItem target\release\bundle\nsis\*.exe | Copy-Item -Destination artifacts -Force
    Copy-Item docs\THIRD-PARTY-LICENSES.txt artifacts\THIRD-PARTY-LICENSES.txt -Force
    Get-FileHash artifacts\*.exe | Format-Table
} finally { Pop-Location }
