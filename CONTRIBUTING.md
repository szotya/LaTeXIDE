# Fejlesztői belépés

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
cargo +1.99.1 test --workspace --locked
```

A jelenlegi Mac offline környezetben Cargo helyett `bash scripts/cargo-local.sh test --workspace --locked --offline`. Natív Mac/IPC/MCP részletes parancsok a handoffban. A távoli CI csak az általa ténylegesen futtatott köröket bizonyítja. Windows WinUI build és teljes compiler/PDF/agent workflow külön minősítés.

Új generált tervekhez a `docs/development/package-specs.json` a szerkesztendő forrás, majd `python3 scripts/build_development_packages.py` és a validator. A dátumozott audit régi bizonyítékait ne írd át aktuális tesztként.

## Adat és jogosultság

Ne commitolj credentialt, `.latexide` sessiont, helyi `.codex` metaadatot, `.local` toolchaint, `target`/`dist` terméket vagy felhasználói privát projektet. A nyers `evidence` helyi; csak a [pontos source/provenance-kivételek](evidence/README.md) verziózottak. Új eredményhez redaktált összegzés és reprodukálható tesztforrás kell, nyers log ne kerüljön automatikusan Gitbe.

## Licenc és közreműködés

A projekt [PolyForm Perimeter 1.0.1](LICENSE) licencű; a [licenc- és hozzáférési összefoglaló](LICENSING.md) a teljes szöveghez ad eligazítást. Saját hozzájárulásodat ugyanezen licenc alatt küldd be, a szükséges jogok birtokában. Eltérő licencű anyagot előre jelezz, a meglévő notice-okat őrizd meg. A szerzőséged megmarad; ez nem általános újralicencelési felhatalmazás.

A nyilvános olvasás, fork, issue vagy PR nem ad upstream push-/merge-jogot. Ehhez a tulajdonos külön jogosultságadása kell, és a main branchvédelmi szabályai akkor is érvényesek. Elfogadott commit miatt megjelenhetsz a contributor-listában; ez nem collaborator-meghívás.
