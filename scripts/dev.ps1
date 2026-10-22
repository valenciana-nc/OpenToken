$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\windows-env.ps1"
Push-Location (Split-Path $PSScriptRoot -Parent)
try { npm run desktop:dev } finally { Pop-Location }
