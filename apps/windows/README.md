# Windows fejlesztői munkatér — P21

Cél: **Windows 11 x64, Intel és AMD**, C++/WinRT + WinUI 3. Windows ARM64 és x86 nem támogatási cél. A meglévő forrás még nem igazolt natív build vagy kiadás.

Kezdőág: `szotya/p21-windows`; issue: [#22](https://github.com/szotya/LaTeXIDE/issues/22). Előbb olvasd a [közös platformbelépőt](../../docs/development/PLATFORM_DEVELOPMENT.md), a [handoffot](../../docs/DEVELOPER_HANDOFF.md), a [projekt-skillt](../../skills/latexide-platform-development/SKILL.md), majd a [P21 tervet](../../docs/superpowers/plans/2026-10-31-parallel/p21-windows.md).

## Mappák és módosítási határ

- A teljes `apps/windows/` P21-é: a meglévő `MainWindow*`, `NativeCore*`, XAML, `.sln`/`.vcxproj`, NuGet, téma- és editoradapterek javíthatók itt, kis PR-okban.
- `Features/PlatformParity/`: a célgépi kvalifikáció saját kódja; nem újabb dokumentummodell.
- `tests/`: meglévő strukturális és hordozható C++ helper tesztek. A célgépes workflow új tesztjei a [tests/windows_native](../../tests/windows_native/README.md) alatt készülnek.
- P21 módosíthatja a `scripts/build_windows.ps1`, `tools/windows-nuget-lock.json`, `tests/windows_text_adapter.cpp`, `docs/windows-validation.md`, `docs/windows-ui-port.md`, saját manifest és [eredménynapló](../../docs/development/results/P21.md) fájlokat is.
- Közös Rust/ABI/IPC/MCP, root Cargo/lock/toolchain, paletta-JSON, workflow, központi dokumentáció és skill: saját manifestes igény + P00-integráció. A másik platform fájlját ne módosítsd a saját ágadban.

## Első valódi build

Követelmény: Windows x64 VS 2022 C++ developer PowerShell, MSVC/Windows SDK, NuGet és hivatalos Rust 1.99.1 MSVC. A pontos csomagpin és TeX-feltétel a [Windows-validációban](../../docs/windows-validation.md), a [Windows-skillben](../../skills/latexide-platform-development/references/windows.md) és a [TeX Live útmutatóban](../../tools/texlive/README.md) szerepel.

Ha a PowerShell futtatási házirendje blokkolja a buildscriptet, előbb ellenőrizd az effektív beállítást és a scope-okat:

```powershell
Get-ExecutionPolicy
Get-ExecutionPolicy -List
```

Ha a helyi szabályzat megengedi az ellenőrzött checkout helyi scriptjét, az aktuális munkamenetre használható:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy RemoteSigned
Get-ExecutionPolicy
```

A Process beállítás a munkamenet végén megszűnik; a Group Policy MachinePolicy/UserPolicy szabálya felülírhatja. Ilyen tiltásnál a szervezeti eljárást kövesd. [Microsoft: execution policy és scope](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies?view=powershell-5.1).

```powershell
cargo +1.99.1 test --workspace --locked
cargo +1.99.1 check --workspace --target x86_64-pc-windows-msvc --locked
.\scripts\build_windows.ps1 -Configuration Debug
.\dist\windows-x64\LaTeXIDE.exe
```

A parancsokat a repository gyökeréből futtasd. A buildscript külön ellenőrzött `.local/texlive-windows-x64` fát vár és csomagol. Hiányzó SDK/compiler valódi környezetblokk, nem PASS. A script eddig nem futott igazolt Windows buildként, ezért az első célgépes kör javításokat fedhet fel. Mac `.a`, TeX-bináris vagy helper PASS nem helyettesíti a Windows linket/futtatást.

Először reprodukálható restore/build/link/launch, majd a közös edit → agent → Save → undo → Save → build → friss PDF menet készüljön el. Utána completion, Ctrl+kattintás, natív Codex-panel, PDF- és felületi paritás. A P21 tervben szereplő `tests/windows_native/qualification.ps1` még létrehozandó teszt; az előkészítés nem tartalmaz helyette mindig zöld stubot.

## Átadás

PR cél: `main`, `Refs #22`. Az [eredménynaplóba](../../docs/development/results/P21.md) írd a Mac-referenciát, base/head SHA-t, tényleges Windows/CPU/toolverziót, parancsokat, teszt-/skip-számot, hibákat és unrun köröket. Az [induló paritásmátrix](../../docs/development/PLATFORM_PARITY.md) eltéréseit konkrét teszttel zárd. A minősítési DTO-ban megadott `launched=true` nem alkalmazásindítási bizonyíték.
