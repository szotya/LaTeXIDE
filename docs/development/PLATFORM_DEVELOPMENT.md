# Windows és Linux fejlesztői belépés — Mac által vezetett közös termék

Ez a dokumentum a P21/P22 fejlesztői munkaterét és módosítási rendjét írja le. A natív Windows és Linux termékelfogadás továbbra **planned/unrun**. Az előkészített branch, mappa vagy DTO nem működő port.

## Irányadó termék és közös állapot

A Mac az elfogadott felhasználói működés referenciája. A [termékirány](../IDE_PRODUCT_DIRECTION.md), a [handoff](../DEVELOPER_HANDOFF.md), majd az adott funkció friss Mac-ellenőrzése határozza meg az elvárt viselkedést. A Swift/AppKit kód és a Mac saját útvonalai platformpéldák; a Windows és Linux saját natív adaptert készít. Egy reprodukált Mac-hiba közös javítandó hiba, nem követendő termékszabály.

A közös Rust-mag birtokolja a dokumentumot, revíziót, szerkesztési történetet és a szolgáltatási szerződéseket. A natív felület ugyanahhoz a runtime-hoz kapcsolódik, mint az MCP. Kötelező a nulla alapú sor/UTF-16-oszlop, BOM/CRLF megőrzése, atomi revízióellenőrzés, közvetlen agent-edit, explicit Save és Save utáni undo/redo. A natív widget saját undo-ja nem válhat második dokumentumtörténetté.

## Branch és az első checkout

| Munka | Közzétett kezdőág | Követelménygazda / issue | Belépés |
| --- | --- | --- | --- |
| Windows 11 x64, Intel/AMD | `szotya/p21-windows` | [P21 / #22](https://github.com/szotya/LaTeXIDE/issues/22) | [Windows README](../../apps/windows/README.md) |
| Linux platformdöntés és port | `szotya/p22-linux` | [P22 / #23](https://github.com/szotya/LaTeXIDE/issues/23) | [Linux README](../../apps/linux/README.md) |

A két kezdőág azonos főági alapból indul. Az előkészítés PR-jának elfogadása után a gazda csak fast-forwarddal emeli őket a dokumentált előkészítési commitra; az aktuális SHA-t a GitHub branch mutatja. Ezek munkakezdő ágak, nem külön termék- vagy kiadási főágak. A közös integrációs alap továbbra `main`.

Windows fejlesztői PowerShellben, tiszta clone-ban:

```powershell
git clone https://github.com/szotya/LaTeXIDE.git
Set-Location LaTeXIDE
git fetch origin
git switch --track origin/szotya/p21-windows
python -B scripts/validate_development_packages.py
python -B integration/registry/generate_contracts.py --check
```

Linuxon, tiszta clone-ban:

```sh
git clone https://github.com/szotya/LaTeXIDE.git
cd LaTeXIDE
git fetch origin
git switch --track origin/szotya/p22-linux
python3 -B scripts/validate_development_packages.py
python3 -B integration/registry/generate_contracts.py --check
```

Az első kis feladat használhatja a kezdőágat. Újabb feladat külön, rövid `szotya/p21-<feladat>` vagy `szotya/p22-<feladat>` ágról induljon az aktuális `origin/main` alapján; a PR célja `main`, hivatkozása `Refs #22` vagy `Refs #23`. Egy munkabranchnek egy aktív szerzője legyen. Módosított közös ágat ne resetelj vagy force-pusholj; a kezdőágat új szinkronizálás előtt egyeztesd a gazdájával. Ha a branch már mainbe került és eltér az új főágtól, a következő feladathoz új ágat készíts.

## Ki mit írhat?

A [package-specs.json](package-specs.json) a szerkesztendő forrás; a [packages.json](packages.json) és az [owners.v1.json](../../integration/registry/owners.v1.json) generált nézet. A következő határok ezekkel együtt érvényesek:

| Gazda | Önállóan módosítható | P00-val integrálandó |
| --- | --- | --- |
| P21 Windows | `apps/windows/` teljes natív fája, beleértve a meglévő XAML/C++/C++-WinRT/NuGet/projektfájlokat; `scripts/build_windows.ps1`; `tests/windows_native/`; `tests/windows_text_adapter.cpp`; `tools/windows-nuget-lock.json`; `docs/windows-validation.md`; `docs/windows-ui-port.md`; saját `integration/requests/P21.json` és `docs/development/results/P21.md` | Közös Rust/FFI/IPC/C ABI/MCP, közös paletták, root lock/toolchain, CI/workflow, központi handoff/ledger/skill |
| P22 Linux | `apps/linux/` teljes natív fája, saját frontend/probe/önálló Cargo-manifest és lockfile; `tests/linux_native/`; a később létrehozandó `scripts/build_linux.sh`; saját `integration/requests/P22.json` és `docs/development/results/P22.md` | Meglévő `session.rs` Linux/XDG bekötése, közös Rust/FFI/IPC/C ABI/MCP, root workspace/lock/toolchain, CI/workflow, központi handoff/ledger/skill |
| P00 integrátor | A gépi jegyzékben felsorolt közös bekötési fájlok és központi dokumentáció | Érintett platformgazda felülvizsgálata, kompatibilitási döntés és regresszió szükséges |

A közös paletták `shared/ide-theme/` fája és a compiler OS-folyamatadaptere (`crates/latexide-build/src/process.rs`) P00 integrációs tulajdon. Más Rust-szolgáltatás saját P-csomagjának gazdájával P00 koordinálja a módosítást; a platformgazda nem ír idegen szolgáltatásfájlt.

A P21 natív fájltulajdona 2026-11-03-tól felváltja a régi szabályt, amely minden meglévő WinUI-fájlt P00-hoz irányított. P22 a teljes Linux-appfát birtokolja, nem csak a `platform_probe` mappát. Más csomag platformfunkciója az adaptergazdával egyeztetett patchként kerül ide; két csomag nem írhatja párhuzamosan ugyanazt a fájlt.

A tulajdon csapat- és integrációs szerződés, nem GitHub fájlszintű hozzáférési jogosultság. A manifest validátora a megadott útvonalakat ellenőrzi; nem bizonyítja automatikusan a teljes PR-diff tulajdonhelyességét. A reviewer a tényleges diffet is egyeztesse a jegyzékkel. Egy branch neve nem ad új push-/merge-jogot.

## Közös módosítás átadása

1. A platformgazda saját fájában reprodukálja a problémát és készíti az adaptert/tesztet. Közös változtatás igényét saját eredménynaplójában rögzíti: pontos célfájl, szükséges viselkedés és kompatibilitási következmény.
2. A valódi implementáció elkészültekor létrehozza saját `integration/requests/P21.json` vagy `P22.json` kérelmét a [v1 manifest-szerződés](contracts/v1.md) szerint: `schema`, `package_id`, `contract_version`, `requirement_ids`, `source_paths`, `hook_targets`, `test_commands`, `compatibility`. Csak létező normál forrásfájlt soroljon fel. Az előkészítő README-khez nincs kész szolgáltatást állító manifest.
3. P00 külön integrálja a közös hookot és a szükséges workspace-/CI-változást; a platformgazda ellenőrzi a célrendszeren. A queue ellenőrző, `not_applied` állapota nem automatikus patch, tesztfuttatás vagy termékelfogadás.
4. Meglévő v1 wire-mezőt, bytearrayt, kvótát vagy C ABI-t nem módosíthat önállóan egy platformport. Új vagy változó függőséghez rögzített verzió/lock és indoklás kell. A gyökér lockfile-t P00 kezeli; a P22 saját, külön workspace-ének lockfile-ja P22-é.

## Paritás és Mac-baseline

A [platformmátrix](PLATFORM_PARITY.md) a mostani portolási kiindulás. Új funkciónál a csomag eredménynaplója tartalmazza:

- az alapul vett `main` commitot és a Mac-funkció konkrét dokumentumát/tesztjét;
- az elvárt felhasználói műveletsort, a saját forrás-SHA-t, OS/toolchain/CPU-adatokat;
- külön a source, type-check, linkelt build, teszt, launch és tényleges natív workflow eredményét;
- a közös API eltérését vagy hiányát, a provider/mock szerepét, a kihagyott és unrun részeket.

A közös elfogadási menet: projektmegnyitás → Unicode/IME szerkesztés → közvetlen MCP-edit → explicit Save → GUI-undo/redo → újramentés → fordítás → friss PDF. Külön ellenőrizendő a prompt/kereső saját undo-ja, a forrásfókusz, panelváltás, tartós beállítások, el nem küldött draft, stale revízió, hibás fordítás és folyamatleállítás.

Mac-funkció továbbfejleszthető a platformport elkészülte előtt, ha a közös kompatibilitás és az előírt regresszió zöld. A hiányzó Windows/Linux natív képesség `planned`, `partial`, `unavailable` vagy `unrun`; megjelenő ikon vagy közös Rust PASS nem teszi késznek. P21/P22 a saját eredménynaplóját írja, P00 ebből frissíti a központi mátrixot, handoffot, ledgert és skill-reference-et.

## CI, review és az első feladatok

A meglévő, pontosan `Planning and Python contracts` nevű required kapu megmarad. A forrásváltozásokhoz kiválasztott Mac integrated és Windows/Ubuntu shared eredményét továbbra is kéri; platformág miatt nincs skip vagy kapulazítás. A [main-protection](BRANCH_PROTECTION.md) szerinti PR, friss alapág, lezárt beszélgetések és squash/rebase kötelező. A PR-sablonba írd a csomag-ID-t, forrás-SHA-t, manifestet, paritáseltérést és tényleges bizonyítékot. Közös viselkedés vagy a Mac által kijelölt termékirány változtatását a termékgazda/integrátor ellenőrizze.

P21 első érdemi szállítása a rögzített függőségekkel valódi Windows restore/MSBuild/link, majd Windows 11 x64 launch és az edit/save/undo folyamat. A [Windows natív teszttér](../../tests/windows_native/README.md) rögzíti a bizonyítékot. A jelenlegi hosted `windows-2025` shared CI Windows Server, nem Windows 11 GUI-minősítés. A natív buildworkflow külön P00-integrációs feladat, ebben az előkészítésben nem készült el.

P22 először kijelölt Linux/disztribúció/architektúra mellett XDG-runtime/IPC-próbát és összehasonlítható GTK/Qt döntési kísérletet készít. A toolkit még nincs kiválasztva. Ezután saját editor/PDF/clipboard/IME/AX/process adapter következik; Wayland és X11 külön scope. A [Linux natív teszttér](../../tests/linux_native/README.md) írja le a kezdő ellenőrzéseket. A Mac bootstrap és `/private/tmp` nem Linux-implementáció.

Kezdeti referencia: a `dd14b4e047a5a13e5a4b2b30942895864bd809dd` főág termékforrása; a legutóbbi teljes [CI41634714704](https://github.com/szotya/LaTeXIDE/actions/runs/41634714704) a `3c0d032b347fd8a816dce724067487cc86aaab68` commitra futott. A köztes főági változás dokumentációs, saját runtime-köre skipped volt. Ez történeti induló referencia; minden későbbi PR a saját aktuális alapját és futását rögzítse.
