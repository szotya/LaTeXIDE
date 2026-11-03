# Linux fejlesztői munkatér — P22

Kezdőág: `szotya/p22-linux`; issue: [#23](https://github.com/szotya/LaTeXIDE/issues/23). A teljes `apps/linux/` P22-é, így a platformpróba után a kiválasztott frontend is itt készülhet. **Natív Linux-alkalmazás, kiválasztott toolkit és igazolt desktop-támogatás jelenleg nincs.**

Előbb olvasd a [közös platformbelépőt](../../docs/development/PLATFORM_DEVELOPMENT.md), a [handoffot](../../docs/DEVELOPER_HANDOFF.md), a [Linux-skillt](../../skills/latexide-platform-development/references/linux.md) és a [P22 tervet](../../docs/superpowers/plans/2026-10-31-parallel/p22-linux.md). A Mac a működési referencia; SwiftUI/WinUI és Mac-bootstrap nem Linux-felület vagy Linux-toolchain.

## Első munka és könyvtárak

1. Válassz dokumentált Linux/disztribúció/architektúra tesztcélt; készíts kicsi GTK/Qt összehasonlítást editor-, PDF-, IME-, AX-, memória-/csomagolási és licenckövetkezményekkel. A megvalósítás előtt a toolkit-döntést a termékgazda fogadja el.
2. `platform_probe/`: saját Linux runtime-directory/Unix socket próba. A tényleges fájlrendszer tulajdonosát, módját, symlinkjeit és a socketnév korlátját vizsgáld; caller által bemondott UID/mode nem OS-bizonyíték.
3. [tests/linux_native](../../tests/linux_native/README.md): a Linuxhoz tartozó tényleges próba és későbbi GUI/workflow tesztek. A tervbeli `test_runtime_dir.py` még létrehozandó.
4. A kiválasztott frontend saját moduljai `apps/linux/` alatt készüljenek. Saját, külön `[workspace]`-es Cargo-projekt és lockfile megengedett; root workspace-regisztrációt vagy root `Cargo.lock`-ot P00 integrál. Most nincs döntést megelőző üres Cargo/build/GUI-stub.
5. A később szükséges `scripts/build_linux.sh` P22-é. Saját [eredménynapló](../../docs/development/results/P22.md) és `integration/requests/P22.json` a P22 átadási pontja; manifest csak létező implementációhoz készüljön.

## Futtatható közös indulási ellenőrzés

A repository gyökeréből, Linuxra telepített hivatalos Rust 1.99.1 és Python mellett:

```sh
python3 -B scripts/validate_development_packages.py
python3 -B integration/registry/generate_contracts.py --check
cargo +1.99.1 test --workspace --locked
```

Ez közös forrás-/Rust-ellenőrzés, nem Linux desktop-, IPC- vagy TeX-minősítés. A `scripts/acquire_tools.py` Mac payloadját ne futtasd Linux-installerként. Saját Linux TeX Live platform, package-rootok és Unicode/multifile/offline próba szükséges.

## Közös integráció és elfogadás

A jelenlegi `crates/latexide-ffi/src/session.rs` minden Unixra `/private/tmp`-t választ. Linuxon saját XDG-runtime eljárás kell, tényleges owner/mode/symlink ellenőrzéssel; a meglévő közös fájlt P00 módosítja a P22-próba és manifest alapján. Hiányzó/hibás XDG esetére legyen explicit, dokumentált és tesztelt viselkedés; automatikus Mac-path fallback nincs.

Az editor UTF-8/scalar/widget pozícióit tudatosan konvertáld a közös UTF-16 szerződésre. A dokumentum, revízió és undo Rustban marad. Wayland és X11, clipboard/IME/fókusz, PDF, process-tree stop, MCP-edit/Save/undo saját Linux-próbát kap. PR cél: `main`, `Refs #23`; a Mac-funkció konkrét forrását, base/head SHA-t, disztribúciót, toolkit/toolverziót, parancsot és unrun részeket az eredménynapló rögzítse.
