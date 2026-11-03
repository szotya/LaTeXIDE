#!/usr/bin/env python3
"""Rebuild the parallel work packages from the immutable audit and package specs.

Only planning artifacts are generated. No proposed service is implemented here.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "docs/development"
PLANS = ROOT / "docs/superpowers/plans/2026-10-31-parallel"
AUDIT = ROOT / "docs/ide-audit-2026-10-31/backlog.json"


def render() -> dict[Path, str]:
    backlog = json.loads(AUDIT.read_text(encoding="utf-8"))
    specs = json.loads((DATA / "package-specs.json").read_text(encoding="utf-8"))
    ids = {item["id"] for item in backlog}
    ownership: dict[str, str] = {}
    for package in specs:
        for item in package["requirements"]:
            if item in ownership:
                raise ValueError(f"Duplicate owner for {item}")
            ownership[item] = package["id"]
    if set(ownership) != ids:
        raise ValueError(f"Unmapped requirements: {ids ^ set(ownership)}")
    outputs = {}
    packages = []
    rows = []
    header = """> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

"""
    for p in specs:
        items = [item for item in backlog if ownership[item["id"]] == p["id"]]
        filename = f"{p['id'].lower()}-{p['slug']}.md"
        plan = PLANS / filename
        test = p["test_path"]
        dependencies = sorted({ownership[dep] for item in items for dep in item["prerequisites"] if ownership[dep] != p["id"]})
        entry = dict(p, plan=plan.relative_to(ROOT).as_posix(), status="planned", requirement_count=len(items),
                     start_requires=[] if p["id"] in {"P00", "P19", "P21", "P23"} else ["P00:contract-v1"],
                     integration_neighbors=dependencies)
        packages.append(entry)
        body = f"# {p['id']} — {p['title']} Implementation Plan\n\n" + header
        body += f"**Goal:** {p['goal']}\n\n**Architecture:** {p['architecture']}\n\n"
        body += "**Tech Stack:** Rust 1.99.1 / edition 2024, serde; célplatformi natív adapter és meglévő tesztkeret. Új függőség csak rögzített lockfile-lal és indokolt licenc-/eredetellenőrzéssel.\n\n"
        body += "**Spec:** [közös csomagszerződés](../../specs/2026-10-31-parallel-development-design.md), [audit](../../../IDE_AUDIT_2026-10-31.md), [aktuális állapotváltozások](../../../development/BASELINE_DELTA.md).\n\n"
        body += "## Global Constraints\n\n"
        for line in [
            "Egyetlen hiteles Rust dokumentummodell; a natív adapter és az MCP ugyanazt a sessiont használja.",
            "Pozíció: nulla alapú sor/UTF-16-oszlop; minden módosítás expected_revision mellett atomi. UTF-8/BOM és LF/CRLF megőrzendő.",
            "Agent edit közvetlen; nincs szerkesztésenkénti jóváhagyási kapu. Save explicit; save után is működő undo/redo szükséges.",
            "Mac: arm64, macOS 14+; Windows: Windows 11 x64 Intel/AMD; Linux saját natív igazolásig planned.",
            "A kezdeti 1 MiB/fájl, 10 buffer, 1 MiB IPC és 64 KiB read limit csak külön verziózott/tesztelt döntéssel változhat.",
            "A csomag jelenleg planned/unrun. Meglévő teszt vagy ez a terv nem bizonyítja a teljes csomag megvalósítását.",
            "Csak a saját alábbi útvonalak írhatók párhuzamosan. Meglévő közös fájlhoz integration-manifest és P00-bekötés kell; P00 külön felsorolt integrátori útvonalai kivételek.",
        ]:
            body += f"- {line}\n"
        body += "\n## Review Focus\n\n"
        for i, focus in enumerate(p["review_focus"], 1):
            body += f"- R{i}: {focus}\n"
        body += "\n---\n\n## Tulajdon és indulás\n\n"
        branch = p.get("branch", f"feature/{p['id'].lower()}-{p['slug']}")
        body += f"**Branch:** `{branch}`. **Profil:** {p['tier']}.\n\n"
        if p.get("platform_guide"):
            body += f"**Platformbelépés és Mac-referencia:** [közzétett kezdőág, módosítási határok és paritás]({p['platform_guide']}). A teljes saját natív fára az itt generált, friss owned_paths érvényes; a szolgáltatás és natív teszt továbbra külön megvalósítandó.\n\n"
        body += "**Kizárólagosan létrehozható/módosítható útvonalak:**\n\n" + "".join(f"- `{path}`\n" for path in p["owned_paths"])
        if p.get("integration_only_paths"):
            body += "\n**P00 kizárólagos közös bekötési útvonalai** (csak fogadott manifest és regresszió mellett; a többi csomagnak olvasható):\n\n" + "".join(f"- `{path}`\n" for path in p["integration_only_paths"])
        body += f"\n**Teszt:** `{test}`. **Eredménynapló:** `docs/development/results/{p['id']}.md`.\n\n"
        body += f"**Olvasandó jelenlegi belépési pontok:** {', '.join('`'+path+'`' for path in p['read_paths'])}.\n\n"
        body += "Indulás: " + ("már most elindítható környezet-/tesztfeltárással; a termékbekötéshez P00 contract-v1 kell." if not entry["start_requires"] else "P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.") + "\n\n"
        body += "Integrációs szomszédok: " + (", ".join(dependencies) or "nincs más követelménygazda") + ". Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.\n\n"
        if p["tier"] == "optional":
            folder = Path(p["entry_path"]).parents[1].as_posix()
            body += "## Task 0: Vertikumszerződés és önálló tesztcsomag\n\n"
            body += f"- [ ] Hozd létre `{folder}/CAPABILITIES.md` fájlt a fenti Goal technológiai területeire: capability-ID, projektprofil, toolverzió, natív platform, elvárt workflow és teszt. A három állapot: required/optional/unsupported; unsupported tétel mellett nincs teljes-paritás állítás.\n"
            body += f"- [ ] `{folder}/Cargo.toml`: package `latexide-{p['id'].lower()}-{p['slug']}`, version `0.1.0`, edition `2024`, rust-version `1.99`, külön üres `[workspace]` és `latexide-contracts = {{ path = \"../../../crates/latexide-contracts\" }}` dependency. Így külön tesztelhető, nem módosít közös workspace-t.\n"
            body += f"- [ ] P00 SDK-v1 után `cargo +1.99.1 generate-lockfile --manifest-path {folder}/Cargo.toml`; commitold a saját lockfile-t, majd a terv további `--locked` parancsait használd.\n"
            body += "- [ ] Legalább két eltérő projekt-/toolverzióprofilra rögzíts kisméretű, licencelt fixture-t; a provider hiányát MissingCapability/UnrunPlatform állítás vizsgálja. Ez a smoke-kapu még nem a teljes vertikum.\n\n"
        body += "## Task 1: Önállóan ellenőrizhető szerződésszelet\n\n"
        body += f"**Create:** `{p['entry_path']}`, `{test}`. **Consumes / produces:**\n\n```rust\n{p['interface']}\n```\n\n"
        body += "Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.\n\n"
        if p.get("adapter_interface"):
            body += "**Saját adapter belépési pontja:**\n\n```" + p["adapter_language"] + "\n" + p["adapter_interface"] + "\n```\n\nA fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.\n\n"
        body += f"- [ ] **RED:** `{test}` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **{p['first_assertion']}**\n"
        body += f"- [ ] Futtasd: `{p['test_command']}`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.\n"
        body += f"- [ ] Valósítsd meg az első portot `{p['entry_path']}` alatt; az alkalmazás-regisztrációt add át `integration/requests/{p['id']}.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).\n"
        body += f"- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.\n"
        body += f"- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- {p['entry_path']} {test} integration/requests/{p['id']}.json` és `git commit -m \"feat({p['id'].lower()}): establish tested service slice\"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.\n\n"
        body += "## Követelményenkénti feladatsor\n\n"
        body += "Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.\n\n"
        for item in items:
            test_name = item["id"].lower().replace("-", "_")
            prereqs = ", ".join(f"{dep} ({ownership[dep]})" for dep in item["prerequisites"]) or "nincs"
            current = item["current_boundary"]
            if item["id"] == "LANG-06":
                current = "A történeti audit óta saját makró/ref/cite Cmd-click és pontos dirty/disk célhely elkészült; scope, peek, ambiguity picker és történet továbbra hiány. Lásd BASELINE_DELTA."
            body += f"### {item['id']} — {item['title']} ({item['priority']}, {item['effort']})\n\n"
            body += f"**Kiindulás:** {current}\n\n**Módosítás:** {item['proposed_change']}\n\n**Valódi bekötés előtt:** {prereqs}.\n\n"
            body += f"- [ ] `{test_name}` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **{item['acceptance']}**\n"
            body += f"- [ ] RED→minimális implementáció a saját modulban→GREEN a `{p['test_command']}` paranccsal; a szomszéd portokat a contract-v1 fixture adja.\n"
            body += f"- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.\n"
            body += f"- [ ] Kis commit és `{p['id']}` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `{item['id']}` implemented/tested állapotúnak.\n\n"
            rows.append({"requirement_id": item["id"], "package_id": p["id"], "priority": item["priority"], "effort": item["effort"], "status": "planned", "plan": plan.relative_to(ROOT).as_posix(), "prerequisites": ",".join(item["prerequisites"]), "acceptance": item["acceptance"]})
        body += "## Task Z: Bekötés és átadás\n\n"
        body += f"- [ ] `python3 scripts/validate_development_packages.py` és `{p['test_command']}` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.\n"
        body += "- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.\n"
        body += "- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.\n"
        body += f"- [ ] `docs/development/results/{p['id']}.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.\n\n"
        body += "## Másolható indítófeladat\n\n"
        body += f"> Hajtsd végre a `{plan.relative_to(ROOT).as_posix()}` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak {p['id']} saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.\n"
        outputs[plan] = body
    manifest = {"schema_version": 1, "baseline": "2026-10-31", "audit_sha256": hashlib.sha256(AUDIT.read_bytes()).hexdigest(),
                "core_packages": sum(p["tier"] == "core" for p in specs), "optional_packages": sum(p["tier"] == "optional" for p in specs),
                "requirement_count": len(backlog), "packages": packages}
    outputs[DATA / "packages.json"] = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader(); writer.writerows(rows)
    outputs[DATA / "requirement-map.csv"] = "\ufeff" + stream.getvalue()
    overview = "# Párhuzamos fejlesztési csomagok\n\n"
    overview += f"**24 fő csomag + 20 választható technológiai csomag; mind a {len(backlog)} auditkövetelménynek pontosan egy elsődleges gazdája van.** Minden termékfejlesztési csomag planned; a jelen munka a terveket és a GitHub-fejlesztési alapot készíti el.\n\n"
    overview += "[Közös szerződés és integráció](../superpowers/specs/2026-10-31-parallel-development-design.md) · [Állapotfrissítés](BASELINE_DELTA.md) · [Gépi csomagjegyzék](packages.json) · [226 soros kiosztás](requirement-map.csv) · [Repo/onboarding](REPOSITORY.md) · [44 GitHub-issue](github-issues.json)\n\n"
    overview += "[Windows/Linux platformbelépő és közzétett kezdőágak](PLATFORM_DEVELOPMENT.md) · [Mac által vezetett paritásmátrix](PLATFORM_PARITY.md). P21 a teljes Windows, P22 a teljes Linux natív fát birtokolja; közös runtime/CI és központi guidance P00-integráció. A fejlesztői előkészítés nem natív platformelfogadás.\n\n"
    overview += "## Párhuzamos indítás\n\n1. P00 rögzíti a contract-v1 DTO-kat, fixture-öket és a modulok bekötési helyeit. Közben P19 tesztkorpusz, P21 Windows környezet és P23 méréstervezés már indulhat.\n2. Az első elkészült contract-szelet után egyszerre indítható P01/P02/P03/P04/P05/P08/P09/P10/P12/P14/P18; izolált tesztekhez egymás szolgáltatásait fake port helyettesíti. A többi fő csomag ugyanígy indulhat a saját v1 szerződése után; nincs kötelező teljes csomagsorrend.\n3. Bekötéskor követelményenként érvényesek az eredeti függőségek. Az adapterek és a szolgáltatók egymástól független fejlesztése nem jelent provider nélküli készterméket.\n4. P20 kiadási jelöltet minősít. P21 Windows natív bizonyítékot ad; P22 Linux külön döntési/portolási program. U01–U20 csak külön választás után kerül a kiadási célba.\n\n"
    overview += "A hét P0 követelmény gazdái: P01 (recovery), P09 (buildfüggőség és snapshot), P18 (compilerhatár), P19 (fault bizonyíték), P20 (kiadás), P21 (Windows). Ezt a hat szálat érdemes elsőként ellátni fejlesztővel; P00 a koordinátor. Személynapot és dátumot nem állítunk mértnek: S/M/L/XL az audit relatív becslése, kapacitás nélkül nincs hiteles naptári határidő.\n\n"
    overview += "```mermaid\nflowchart LR\n  C[\"P00: contract-v1\"] --> A[\"P01–P18: izolált szolgáltatások\"]\n  C --> N[\"P21/P22: célplatformi adapterek\"]\n  A --> I[\"Követelményenkénti integráció\"]\n  N --> I\n  Q[\"P19/P23: korpusz és mérések\"] --> I\n  I --> R[\"P20: kiadási kapuk\"]\n  A -. SDK után .-> U[\"U01–U20: választható vertikumok\"]\n```\n\nEz irányítási nézet, nem a csoportosított backlog-függőségek DAG-ja. Az egyedi követelménygráf aciklikus; csomagcsoportosítás után kölcsönös kapcsolatok természetesek.\n\n"
    overview += "## Fő csomagok\n\n| ID | Önálló felelősség | Követelmény | Első tesztelhető eredmény |\n| --- | --- | ---: | --- |\n"
    for p in packages:
        if p["tier"] == "core":
            overview += f"| [{p['id']}](../../{p['plan']}) | {p['title']} | {p['requirement_count']} | {p['first_assertion']} |\n"
    overview += "\n## Választható technológiai csomagok\n\nEzek nem a hallgatói LaTeX-kiadás blokkolói. Egy SDK-adapter önmagában nem jelent teljes JetBrains-termékparitást: minden vertikumnál külön nyelvi/framework/debug/test/projektmodell capability-mátrix és tényleges korpusz kell.\n\n| ID | Terület | Audit-ID |\n| --- | --- | --- |\n"
    for p in packages:
        if p["tier"] == "optional":
            overview += f"| [{p['id']}](../../{p['plan']}) | {p['title']} | {', '.join(p['requirements'])} |\n"
    overview += "\n## Átvételi szabály\n\nEgy csomag akkor átvehető, ha saját tesztjei, valódi provideres integrációja és a vállalt platform workflow-ja teljesült; az összes hozzá tartozó követelmény külön bizonyítékot kapott. A PR tartalmazza a contract-verziót, a saját eredménynaplót és a bekötési manifestet. A CI zöld állapota nem helyettesíti az élő natív workflow-t, a recovery fault-próbát vagy a kiadási minősítést.\n"
    outputs[DATA / "README.md"] = overview
    return outputs


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    outputs = render()
    stale = []
    for path, content in outputs.items():
        if args.check:
            if not path.exists() or path.read_bytes() != content.encode("utf-8"):
                stale.append(path.relative_to(ROOT).as_posix())
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="")
    if stale:
        raise SystemExit("Stale generated plans: " + ", ".join(stale))
    print(f"{'Checked' if args.check else 'Generated'} {len(outputs)} planning artifacts; 226 requirements, 44 packages.")


if __name__ == "__main__":
    main()
