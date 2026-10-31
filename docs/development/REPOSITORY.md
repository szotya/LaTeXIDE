# Privát GitHub-fejlesztési alap

Kért hely: [szotya/LaTeXIDE](https://github.com/szotya/LaTeXIDE), **private**. Alapág: `main`. A távoli létrehozás és ellenőrzés tényleges eredményét a handoff és a ledger rögzíti; ez az oldal az új clone használati szerződése.

A `main` ágon [aktív branchvédelmi ruleset](BRANCH_PROTECTION.md) működik: saját branch és PR, kötelező összesítő check aktuális alapággal, lezárt beszélgetések és squash/rebase merge; nincs force push, törlés vagy adminbypass. Jelenleg 0 kötelező approval az egyetlen közreműködő miatt. A `Planning and Python contracts` összesítő a tervezési/Python mátrix mellett a forrásváltozásokhoz kiválasztott Rust/Mac kör sikerét is megköveteli.

## Mi van verziózva?

- Hat Rust crate és lockfile, Mac SwiftPM és Windows WinUI forrás, közös C header és paletták.
- Beszerzési/buildscriptek, hivatalos toolchain URL-/hash-rögzítések, TeX Live profilok és eredetmetaadat.
- Automatizált tesztek és kontrollált fixture-ök; a történeti helyi eredmények tartalom helyett [hashjegyzékkel](LOCAL_EVIDENCE.md).
- Audit, 44 terv, 226 soros kiosztás, handoff, ledger és hordozható projekt-skill.
- CI, PR-/issue-sablonok, CODEOWNERS és együttműködési útmutató.

A `.local` toolchain/TeX Live, `target`, `dist`, Swift-cache, helyi `.codex` metaadat, fordított tesztexecutable, letöltött installer/payload és élő `.latexide` session kimarad. Nyers történeti evidence, screenshot, futási log és másolt pillanatkép szintén nem feltöltött. Csak három auditsegéd-forrás és hat nyilvános TeX Live aláírás/checksum/publikuskulcs-metaadat verziózott az `evidence` alatt; [pontos határ](../../evidence/README.md). A történeti dokumentum helyi output-hivatkozása nem új letöltési vagy reprodukálási garancia.

## Tiszta clone

```sh
git clone https://github.com/szotya/LaTeXIDE.git
cd LaTeXIDE
python3 scripts/validate_development_packages.py
python3 -m unittest discover -s tests -p 'test_*.py'
```

A privát clone-hoz GitHub-hozzáférés kell. Credential nem része a repónak.

## Rust és Mac

Meglévő rustup esetén `rustup toolchain install 1.99.1 --profile minimal`, majd `cargo +1.99.1 test --workspace --locked`. A közös tesztek külön réteget minősítenek, nem teljes natív alkalmazást.

A Mac projekt saját helyi toolchain-eljárása: `python3 scripts/acquire_tools.py --install`. Ez a rögzített hivatalos payloadokat `.local` alá tölti/ellenőrzi/telepíti; hálózatot használ, nem globális telepítés. Kövesd a [TeX Live útmutatót](../../tools/texlive/README.md) a compilerhez; több lépés és külön offline smoke szükséges. A runtime nincs a Gitben.

Natív teszthez előbb az FFI/MCP buildje, majd SwiftPM:

```sh
cargo +1.99.1 build -p latexide-ffi -p latexide-mcp --locked
swift test --package-path apps/macos --disable-sandbox
python3 tests/ipc_contract.py
python3 tests/mcp_contract.py
```

Teljes Mac bundle a saját helyi toolchainnel és telepített TeX Live-val: `bash scripts/build_macos.sh`. A futó felhasználói appot a repositorybehozatal nem indítja újra. A buildscript ad-hoc aláírása nem Developer ID/notarizáció.

## Windows és Linux

Windows 11 x64, VS/MSVC/Windows SDK és rögzített NuGet-függőségek: [eljárás és korlát](../windows-validation.md), [P21 terv](../superpowers/plans/2026-10-31-parallel/p21-windows.md). A CI Windows Rust-tesztje nem WinUI MSBuild/link/launch vagy Intel+AMD gépmátrix. A teljes native bizonyítékot P21 adja.

Linuxon a CI közös Rust-réteget ellenőrizhet, de nincs kész natív felület vagy támogatott desktopkiadás. A [P22 terv](../superpowers/plans/2026-10-31-parallel/p22-linux.md) külön kezeli az XDG/runtime-dir és UI/platformdöntést.

## CI és források

A `Source and contract checks` workflow main-push/PR/manual eseményre Ubuntu és Windows tervezési/Python ellenőrzést és állandó nevű összesítőt futtat. A Git-diff alapján szükséges `Rust and Mac checks` újrahasználható workflow Macen az integrált profilt, Ubuntu/Windowson a közös Rust-profilt futtatja; ismert docs-only változás kihagyhatja ezt a mátrixot. Hiányzó Git-bázis nem adhat lazább kaput; kézi indítás teljes kört kér. Nincs online modellkérés, compilertelepítés, aláírt release vagy deployment. A workflow végrehajtott eredménye a GitHub Actions lapon olvasható; a workflowfájl létezése még nem PASS. Az új profilok és report parancsai a [v1 útmutatóban](contracts/v1.md), tényleges eredményei a [P00 naplóban](results/P00.md) találhatók.

A kontrollált asszisztens-alfolyamat forrása `tests/fixtures/assistant/assistant_fixture.py`; ezt a natív teszt maga csomagolja privát ideiglenes fixture-ré, valódi modell vagy credential nélkül. Fixture-változás is indítja a natív CI-t. A négy opt-in skip külön marad: telepített Codex handshake és három render. A történeti `WorkspaceRenderTests` opt-in kör a helyi korábbi demo-build/evidence inputot igényli; ez a nyers input nincs Gitben, és ez a kör nem minősített tiszta-clone teszt. Új önálló renderfixture és teljes vizuális kvalifikáció P23/QA-07 feladata.

Runnercímkék az aktuális [GitHub runner-dokumentációból](https://docs.github.com/en/actions/reference/runners/github-hosted-runners); a használt `macos-15` arm64. A [checkout](https://github.com/actions/checkout) és [setup-python](https://github.com/actions/setup-python) v7 hivatalos tagjának commitja van rögzítve, nem lebegő tag. A privát repo runnerhasználatának elérhetőségét/kvótáját a GitHub-fiók adja; a workflow nem állít be új fizetős keretet.

`python3 scripts/check_repository_content.py --staged` a tényleges indexben ellenőrzi a tiltott helyi útvonalakat, ismert credentialmintákat, nagy fájlokat és executable fejléceket. Ez korlátos higiene-ellenőrzés, nem teljes titokdetektor vagy malware-/biztonsági minősítés.

A létrehozott repository végső [ellenőrzött CI-eredménye](REMOTE_VERIFICATION.json) és [átadási összegzése](IMPORT_VERIFICATION.md) külön rögzíti a pontos commitot, tesztszámokat és unrun határokat. A tervekhez [44 saját GitHub-issue](github-issues.json) tartozik.

## Tulajdon és licenc

A CODEOWNERS `@szotya` gazdát jelöl; nem hív meg közreműködőt. A repó privát, projektlicenc nincs kiválasztva. Harmadik felek Rust/Swift/NuGet/TeX Live komponenseinek saját licencei és terjesztési feltételei külön érvényesek; kiadási SBOM/notice/minősítés P18/P20 feladata. Új projektlicencet külön döntés nélkül ne adj hozzá.
