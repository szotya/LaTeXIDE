# Fejlesztői belépés

**Tesztelőként hibát, hiányzó funkciót vagy új ötletet jelentenél?** Kezdd a [magyar tesztelői útmutatóval](docs/TESTING.hu.md), majd válassz [issue-űrlapot](https://github.com/szotya/LaTeXIDE/issues/new/choose). Ehhez nem kell kódot módosítanod vagy az alábbi fejlesztési eljárást követned.

Olvasd az [AGENTS.md](AGENTS.md), a [handoff](docs/DEVELOPER_HANDOFF.md), a [projekt-skill](skills/latexide-platform-development/SKILL.md) és a [termékirány](docs/IDE_PRODUCT_DIRECTION.md) dokumentumait. Platformonként különítsd el a forrás/check/build/teszt/indítás/workflow bizonyítékot.

## Párhuzamos csomag végrehajtása

1. Válassz egyet a [44 csomag](docs/development/README.md) közül; P00 v1 szerződése után saját fixture-rel indulj. Az U-csomagok külön választhatók.
2. A csomagtervben rögzített branch, szükség esetén saját worktree. Windows/Linux induláshoz `szotya/p21-windows` / `szotya/p22-linux`, további kis feladatokhoz `szotya/p21-<feladat>` / `szotya/p22-<feladat>` az aktuális mainből; [pontos belépés és tulajdon](docs/development/PLATFORM_DEVELOPMENT.md). A jelenleg futó app dirty munkáját ne dobd el és ne indítsd újra automatikusan.
3. Csak a saját `owned_paths` írhatók párhuzamosan. A [közös szerződés](docs/superpowers/specs/2026-10-31-parallel-development-design.md) leírja a közös fájlok P00 általi bekötését.
4. Minden követelményhez saját reprodukáló teszt, kis implementáció, GREEN és eredménynapló. Mock és valódi provider eredménye külön szerepeljen.
5. PR: követelmény-ID-k, contract-verzió, eredménynapló, integration-manifest és tényleges parancsok. P00 a bekötés után frissíti a handoffot, ledgert és skill-reference-et.

## Ellenőrzések

```sh
python3 scripts/validate_development_packages.py
python3 -m unittest discover -s tests -p 'test_*.py'
python3 integration/registry/verify.py --profile shared
```

A teljes workspace tesztjeinek támogatott belépése a `shared` verifier (Windowson `python` a `python3` helyett). Ez friss, kizárólagos fixture-gyökeret és forráshoz/futtatáshoz kötött nonce-ot ad a kötelező `P02_RUNTIME_FIXTURES`, `P02_WORKSPACE_FIXTURES`, `P02_RUN_NONCE` változókhoz, majd ellenőrzi a tesztprogramokat és a bizonyítékokat. Minden ilyen build előtt takarítja a saját target workspace-csomagjait; a gyorsítótárból kapott P02 tesztprogram nem válik friss bizonyítékká.

A korábbi platformútmutatók közvetlen `cargo test --workspace --locked` parancsa és a Mac `scripts/cargo-local.sh` wrapper önmagában nem állítja elő ezeket a változókat: hiányuk szándékos teszthiba. A teljes workspace-hez a verifier fenti belépését használd; a nyers tesztprogram célzott fejlesztői futtatásához a hívó biztosít mindhárom friss, saját gyökérhez kötött értéket, ami önmagában nem CI-minősítés. Offline környezetben a verifier `--offline` kapcsolója és a már ellenőrzött helyi Rust-környezet használható; toolchain-telepítés külön feladat. Natív Mac/IPC/MCP részletes parancsok a handoffban. A távoli CI csak az általa ténylegesen futtatott köröket bizonyítja. Windows WinUI build és teljes compiler/PDF/agent workflow külön minősítés.

Új generált tervekhez a `docs/development/package-specs.json` a szerkesztendő forrás, majd `python3 scripts/build_development_packages.py` és a validator. A dátumozott audit régi bizonyítékait ne írd át aktuális tesztként.

## Adat és jogosultság

Ne commitolj credentialt, `.latexide` sessiont, helyi `.codex` metaadatot, `.local` toolchaint, `target`/`dist` terméket vagy felhasználói privát projektet. A nyers `evidence` helyi; csak a [pontos source/provenance-kivételek](evidence/README.md) verziózottak. Új eredményhez redaktált összegzés és reprodukálható tesztforrás kell, nyers log ne kerüljön automatikusan Gitbe.

## Licenc és közreműködés

A projekt [PolyForm Perimeter 1.0.1](LICENSE) licencű; a [licenc- és hozzáférési összefoglaló](LICENSING.md) a teljes szöveghez ad eligazítást. Saját hozzájárulásodat ugyanezen licenc alatt küldd be, a szükséges jogok birtokában. Eltérő licencű anyagot előre jelezz, a meglévő notice-okat őrizd meg. A szerzőséged megmarad; ez nem általános újralicencelési felhatalmazás.

A nyilvános olvasás, fork, issue vagy PR nem ad upstream push-/merge-jogot. Ehhez a tulajdonos külön jogosultságadása kell, és a main branchvédelmi szabályai akkor is érvényesek. Elfogadott commit miatt megjelenhetsz a contributor-listában; ez nem collaborator-meghívás.
