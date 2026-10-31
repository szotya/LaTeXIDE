# Független fejlesztési csomagok — közös szerződés

2026-10-31. A felhasználó az IDE-auditból párhuzamosan végezhető fejlesztési terveket és teljes privát GitHub-repót kért: **szotya/LaTeXIDE**. Ez a specifikáció a munkabontást rögzíti; nem jelenti a tervezett termékfunkciók megvalósítását.

[Csomagjegyzék](../../development/README.md) · [Audit](../../IDE_AUDIT_2026-10-31.md) · [Handoff](../../DEVELOPER_HANDOFF.md) · [Termékirány](../../IDE_PRODUCT_DIRECTION.md)

## Cél és teljesség

Az audit 226 követelménye pontosan egy gazdát kap: 206 LaTeX/kutatói/fejlesztői követelmény 24 fő csomagban; 20 széles technológiai vertikum 20 külön választható csomagban. A P00 koordinátor maga is fő csomag. A relatív S/M/L/XL becslések az auditból származnak, nem személynapok és nem vállalt határidők.

Függetlenség itt külön branch-et, kizárólagos új fájlokat, fagyasztott bemeneti szerződést és önálló tesztet jelent. Egy teljes nyelvi motor, recovery és refaktorálás természetesen közös dokumentumállapotot használ; a késztermékbe integrálás ezek nélkül nem minősíthető teljesnek. A csomagok közötti kapcsolatok nem merev egészcsomag-sorrendet alkotnak. Az eredeti követelménygráf marad az elfogadási feltételek sorrendje.

## Változatlan globális szerződések

- Egyetlen hiteles Rust dokumentummodell; a natív adapter és MCP ugyanarra a runtime-ra kapcsolódik.
- Rust 1.99.1, edition 2024; macOS 14+ arm64; Windows 11 x64 Intel/AMD. Linux külön későbbi platformprogram.
- Nulla alapú sor és UTF-16-oszlop; UTF-8/BOM és LF/CRLF megőrzése. Az elavult revízió vagy hibás/átfedő edit nem módosíthat részlegesen.
- Az agent közvetlenül szerkeszt, szerkesztésenkénti jóváhagyás nélkül. Mentés explicit, a mentés utáni undo/redo működik. Külső nyers fájlírás saját konfliktushatár.
- Kezdeti limit: 1 MiB/fájl, 10 buffer, 1 MiB IPC, 64 KiB read. Emelés vagy lease/evikció külön tesztelt, verziózott változás.
- Kompozíciót a natív editor birtokol. `composition_active` alatt nem alkalmazható idegen edit/save/build; a hibás commit után is helyesen lezárul a kompozíció.
- C ABI pufferét a Rust megfelelő free-je szabadítja; hívás alatt nincs handle-free. Panic nem lép át a C-határon.
- Compiler szűk környezetből indul, agentcredentialt nem örököl. A P18 valódi OS-fájlrendszer-/hálózathatárt épít, nem puszta argumentumszűrést.
- A felület egy koherens munkaterület. Panelek száma vagy vizuális hasonlóság nem bizonyít workflow-t, fókuszt, tartós draftot vagy tényleges toolállapotot.

## P00 contract-v1: a minimális indulási kapu

P00 külön `crates/latexide-contracts` crate-et ad, `serde` függőséggel. A jelenlegi irány `protocol → core`; ezért a közös DTO-k nem kerülhetnek a protocolba úgy, hogy a core is attól függjön. Új irány: core/protocol/build → contracts, contracts egyikre sem hivatkozik. A workspace-regisztrációt és a lockfile-t P00 kezeli.

A v1 közös alapok pontos alakja:

```rust
pub struct Utf16Range { pub start: u32, pub length: u32 }
pub struct SourceAnchor {
    pub document_id: String,
    pub revision: Option<u64>,
    pub content_sha256: String,
    pub line: u32,
    pub character: u32,
    pub range: Utf16Range,
}
pub struct SnapshotFile {
    pub path: String,
    pub revision: Option<u64>,
    pub content_sha256: String,
    pub utf8: Vec<u8>,
    pub dirty: bool,
}
pub struct WorkspaceSnapshot {
    pub id: String,
    pub epoch: u64,
    pub files: Vec<SnapshotFile>,
}
pub enum DiagnosticSeverity { Error, Warning, Information, Hint }
pub struct SourceDiagnostic {
    pub anchor: SourceAnchor,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}
pub struct RevisionedEdit {
    pub document_id: String,
    pub expected_revision: u64,
    pub start_utf16: u32,
    pub length_utf16: u32,
    pub replacement: String,
}
```

A szövegpillanatkép szolgáltatáson belül használható; ez nem engedély teljes workspace-szerializálásra minden billentyűütésnél. A DTO-k `Clone/Debug/Serialize/Deserialize` származtatást és roundtrip fixture-t kapnak. A `SourceAnchor.line/character` és az offset nulla alapú. PDF oldalszám a PDF-portban egy alapú; a geometria oldalpont, origó és rotáció explicit az adaptertesztben. Snapshot ID stabil az adott immutable tartalomra; azonos mtime nem tartalombizonyíték. Hash lowercase SHA-256 hex.

A csomagtervek megadják saját portjaikat és DTO-mezőiket; P00 ezekből csomagonkénti contracts-modult fagyaszt. Azonos nevű közös típus egyszer definiált. `PlatformError`, `RevisionedEdit`, `SourceAnchor` és `WorkspaceSnapshot` közös típusok; az egymástól eltérő hibaenumok csomagmodulban névterezettek. A fixture-ökben csak mesterséges sessionadat szerepelhet.

**A contract-v1 kapu ellenőrzése:**

1. `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked` zöld; régi és új v1 JSON roundtrip, ismeretlen major explicit hiba.
2. Minden induló csomag első portjához legalább happy-path, stale, quota, cancellation/timeout és malformed fixture; nem alkalmazható állapothoz dokumentált explicit hiba.
3. P00 modulregisztrációt ad a saját fájlokhoz, így a munkás nem módosít `lib.rs`/`Cargo.toml` közös fájlt. A nem Rust adapter megkapja az azonos DTO fixture-t és natív decode-tesztet.
4. A saját útvonalak és manifest a gépi jegyzékben; `python3 scripts/validate_development_packages.py` zöld.
5. A határ a `docs/development/contracts/v1.md` fájlba kerül; nincs produkciós service vagy új MCP-tool pusztán a DTO-k miatt.

## Fájltulajdon és integrációs queue

Minden csomag külön branch/worktree használatával dolgozik, saját könyvtárban. A gépi jegyzék `owned_paths` mezője kizárólagos. A csomagtervekben felsorolt jelenlegi `read_paths` olvashatók, de nem adnak írási jogot. Saját teszt és `docs/development/results/<ID>.md` is kizárólagos. A projekt meglévő kódját nem kell most teljesen átstrukturálni: az új szolgáltatás izoláltan épül, majd kis adapterpatch köti be.

Ezeket **csak P00 integrálja**, az érintett platformgazda review-jával:

- workspace `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` és meglévő crate `lib.rs`/manifest;
- core `workspace.rs`, `document.rs`, `project_index.rs`, `tex_scan.rs`, `completion.rs`, `definition.rs` közös hookjai;
- protocol `lib.rs`/`ipc.rs`, FFI `runtime.rs`/`session.rs`, MCP toolregisztráció és C header;
- Mac `WorkspaceStore.swift`, `WorkspaceView.swift`, `NativeRuntime.swift`, `NativeEditor.swift`, `SourceTextView.swift`, menü-/panelregisztráció és `Package.swift`;
- Windows meglévő `MainWindow*`, `NativeCore*`, projekt-/NuGet-/XAML-regisztráció; P21 adja a natív build/runtime ellenőrzést;
- közös build-/release-script, workflow, capability-leltár, handoff, ledger és skill-routing.

Ez nem tiltja a szükséges refaktorálást: P00 kis, reprodukálható patch-ként végzi el a seam-kivágást. Két szolgáltatás ne osszon meg írt fájlt. Új közös API igénye `integration/requests/<ID>.json` kompatibilitási döntésként érkezik; major-váltás nem kerül észrevétlenül párhuzamos branch alá.

Manifest kötelező kulcsai: `schema=1`, `package_id`, `contract_version=1`, `requirement_ids`, `source_paths`, `hook_targets`, `test_commands`, `compatibility`. A manifest a saját modul és a kért közös bekötési pontok listája, nem futtatható shellscript. P00 a gépi jegyzékből épített `OwnershipRegistry` alapján ellenőrzi az útvonalakat, követelménygazdát és argumentumokat. A regressziós parancsot nem szabad ellenőrizetlen projektadatból shellnek átadni.

## Fokozatos bekötés, kölcsönös függőségek

Az audit követelményfüggőségei `backlog.json`-ban változatlanul megmaradnak. A csomagjegyzék `integration_neighbors` mezője tájékoztat a más gazdájú függőségekről; **nem DAG**. Például recovery és tartós mentés, illetve QA és adatmodell egymás szolgáltatását vizsgálja. Egész csomagokra tett kölcsönös „készre várás” helyett:

1. provider port + fixture elkészül;
2. fogyasztó saját izolált implementációja zöld;
3. P00 beköti a legkisebb valódi provideres szeletet;
4. követelményenként teljesül a dependency és acceptance;
5. csak ezután teljes csomagátvétel és platformi minősítés.

Ajánlott első erőforráselosztás: P00 koordinátor; P01 recovery; P03 runtime; P04 VFS; P09 build; P18 sandbox; P19 fault; P21 Windows. P23 közben mérési/képernyőolvasó környezetet készít. A többiek szerződésszelet után ugyanígy párhuzamosan dolgozhatnak. A csomagok mindegyikéhez külön ember rendelhető, de 44 ember indítása nem technikai előfeltétel és nem becsült optimális csapatméret.

## Átvétel és bizonyíték

`planned` → `implemented` → `tested:<platform/scope>` → `integrated` → `release-qualified` külön állapot. A most kiadott tervek mind `planned`. Egy auditban régebben `partial` képesség bővítése nem nulláról indul; a [baseline-delta](../../development/BASELINE_DELTA.md) az irányadó.

Minden eredménynapló: pontos commit/tree, parancs, exitkód, tesztszám, skip, fixture/toolverzió, tényleges platform, kapott eredmény, ismert hiány és unrun kör. RED tényleges viselkedési hibát jelent, nem hiányzó SDK-t. Timeout nem PASS. Mock nem valódi compiler/provider. Rust Windows-check nem WinUI link/launch. Mac szintetikus vezérlőesemény nem az újonnan csomagolt app élő felhasználói köre.

Az U01–U20 csomagok további gate-je: a megnevezett technológiai vertikum tételes capability-mátrixa, aktuális hivatalos dokumentáció, rögzített toolkorpusz, legalább két projektprofil és verzióeltéréses regresszió. Az első smoke csak az SDK működését igazolja; teljes IntelliJ/PyCharm/Rider/stb. paritást nem. Ezek a hallgatói LaTeX-kiadásnak nem blokkolói.

## Repo-határ

A privát repó forráskódot, terveket, teszteket, projekt-skillt és a történeti helyi bizonyítékok hashjegyzékét tartalmazza. Nyers napló/képernyőkép/helyi metaadat, helyi toolchain, TeX Live runtime, cache, appbundle, sessiondescriptor és credential nem verziózott. Az evidence alatt csak a külön felsorolt három auditsegéd és hat nyilvános compiler-eredetellenőrzési fájl kivétel. A helyi felhasználói alkalmazást ez a feladat nem indítja újra. Nyílt forrású projektlicencet a felhasználó nem választott; a privát repó létrehozása nem ad új licencet harmadik félnek.
