# Optional user-local MSVC extraction used on this machine, or a normal VS Developer shell.
$ErrorActionPreference = 'Stop'
$cargo = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargo) { $env:PATH = "$cargo;$env:PATH" }
$portable = Join-Path $env:LOCALAPPDATA 'OpenToken-build\msvc\unpack'
if (Test-Path $portable) {
    $vc = Get-ChildItem "$portable\VC\Tools\MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $sdk = "$portable\Windows Kits\10"
    $version = (Get-ChildItem "$sdk\Lib" -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
    $env:PATH = "$($vc.FullName)\bin\Hostx64\x64;$sdk\bin\$version\x64;$env:PATH"
    $env:LIB = "$($vc.FullName)\lib\x64;$sdk\Lib\$version\um\x64;$sdk\Lib\$version\ucrt\x64"
    $env:INCLUDE = "$($vc.FullName)\include;$sdk\Include\$version\ucrt;$sdk\Include\$version\shared;$sdk\Include\$version\um;$sdk\Include\$version\winrt"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = "$($vc.FullName)\bin\Hostx64\x64\link.exe"
}
