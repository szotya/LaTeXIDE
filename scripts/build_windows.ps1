param([ValidateSet('Debug','Release')][string]$Configuration = 'Debug', [string]$NuGet = 'nuget')
$ErrorActionPreference = 'Stop'
$TaskRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $IsWindows -and $PSVersionTable.PSEdition -eq 'Core') { throw 'Run this on Windows 11 x64.' }
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'Use an x64 developer shell on Intel/AMD Windows.' }
Push-Location $TaskRoot
try {
    $version = (& rustc --version) -join ''
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '^rustc 1\.99\.1 ') { throw 'Install official Rust 1.99.1 x64 MSVC first; see developer handoff.' }
    & cargo build --release --locked --target x86_64-pc-windows-msvc -p latexide-ffi -p latexide-mcp
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
    & $NuGet restore apps/windows/LaTeXIDE.sln -PackagesDirectory .local/windows-packages -Source https://api.nuget.org/v3/index.json -NonInteractive
    if ($LASTEXITCODE -ne 0) { throw 'Pinned NuGet restore failed.' }
    & msbuild apps/windows/LaTeXIDE.sln /m /p:Configuration=$Configuration /p:Platform=x64
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' }
    $output = Join-Path $TaskRoot 'dist/windows-x64'
    Copy-Item 'target/x86_64-pc-windows-msvc/release/latexide-mcp.exe' $output
    # Runtime loads only this staged asset beside the exe, never the checkout.
    Copy-Item 'shared/ide-theme/Sources/IDEThemeAssets/Resources/editor-themes.json' (Join-Path $output 'editor-themes.json')
    $compiler = Join-Path $TaskRoot '.local/texlive-windows-x64/bin/windows/latexmk.exe'
    if (-not (Test-Path $compiler)) { throw 'Verified Windows TeX Live installation missing. Follow tools/texlive/README.md; the Mac tree cannot be copied as Windows binaries.' }
    & robocopy (Join-Path $TaskRoot '.local/texlive-windows-x64') (Join-Path $output 'texlive') /E /NFL /NDL /NJH /NJS
    if ($LASTEXITCODE -ge 8) { throw 'Compiler staging failed.' }
    Write-Host "Built locally: $output/LaTeXIDE.exe. Run native acceptance before treating it as qualified."
} finally { Pop-Location }
