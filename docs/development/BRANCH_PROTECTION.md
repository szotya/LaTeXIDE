# Branchvédelem

A [main-protection ruleset](https://github.com/szotya/LaTeXIDE/rules/26284711) **aktív**, API-ból visszaellenőrizve 2026-10-31-én. A `~DEFAULT_BRANCH` célzás jelenleg a `main` ágat védi; alapágváltáskor az új alapágat követi. A munkabranchekre nem vonatkozik.

Az alkalmazott [JSON-konfiguráció](../../.github/rulesets/main-protection.json) újra felhasználható API-input. A [visszaellenőrzési összegzés](BRANCH_PROTECTION_VERIFICATION.json) dátumozott pillanatkép, nem folyamatos állapotfigyelő. A helyi JSON szerkesztése önmagában nem módosítja a GitHub-beállítást.

| Szabály | Beállítás és indok |
| --- | --- |
| Változtatás PR-on keresztül | Kötelező; közvetlen push helyett saját branch és követhető PR. |
| Kötelező ellenőrzés | `Planning and Python contracts`, kizárólag a GitHub Actions alkalmazástól (`integration_id: 15368`). |
| Aktuális alapág | A kötelező checknek a legfrissebb alapággal együtt kell megfelelnie (`strict`); szükség esetén frissítsd a PR branchét és várd meg az új CI-t. |
| Review-beszélgetések | Merge előtt minden kódbeszélgetést le kell zárni. |
| Jóváhagyások | Jelenleg **0 kötelező approval**. A felméréskor egyetlen közreműködő volt, `@szotya` admin; a saját PR és a kizárólag saját CODEOWNERS-bejegyzés kötelező önjóváhagyása blokkolná a munkát. |
| Új commit utáni review | A korábbi jóváhagyás érvényét veszti. CODEOWNER- és last-push-approval külön nincs előírva. |
| Merge mód | Squash vagy rebase; lineáris előzmény kötelező. |
| Force push / törlés | Mindkettő tiltott a védett ágon. |
| Bypass | Üres lista; a szabályok az adminra is érvényesek. A visszaolvasott `current_user_can_bypass` érték `never`. |

Az admin továbbra is módosíthatja magát a rulesetet a repositorybeállításokban. Ez nem kivétel a napi push-/merge-folyamatra. Nincs új kötelező aláírás, merge queue, deployment- vagy fizetős security-check előfeltétel.

## A kötelező CI pontos határa

A [Source and contract checks](../../.github/workflows/ci.yml) minden PR-on elindul. A kötelező job a csomag-/követelménykiosztás validátorát, a Python-szerződésteszteket és a korlátos repositorytartalom-ellenőrzést futtatja. Ez nem teljes biztonsági scan és nem Rust-/natív termékteszt.

A [Rust and Mac checks](../../.github/workflows/rust-contracts.yml) útvonalszűrt. A három `Rust contracts (...)` job ezért **nem kötelező ruleset-check**: pusztán dokumentációs PR-on nem is jönnének létre, így állandó required checkként a merge függőben maradna. A releváns forrásváltozások továbbra is elindítják a meglévő workflow-t, de annak eredményét ez a ruleset nem kényszeríti ki. Forrás-PR áttekintésekor a megfelelő Rust/Mac eredményt külön ellenőrizni kell.

Tervezett szigorítás: minden PR-on létrejövő, útvonal szerint feltételesen futó Rust/Mac jobokat összesítő, hiba esetén blokkoló kapu. Csak sikeres forrás- és docs-only távoli próba után kerüljön a required checkek közé. Ez a kapu most nincs implementálva. Az első független aktív reviewer belépésekor a kötelező approval számát érdemes 1-re emelni.

## Alkalmazás és visszaellenőrzés

Hitelesített, repository-admin jogosultságú GitHub CLI szükséges. A repo privát marad; token vagy credential nem része a konfigurációnak.

Az első létrehozás tényleges parancsa:

```sh
gh api --method POST repos/szotya/LaTeXIDE/rulesets --input .github/rulesets/main-protection.json
```

A meglévő ruleset módosítása ugyanazzal az azonosítóval történjen, ne újabb azonos célú szabály létrehozásával:

```sh
gh api --method PUT repos/szotya/LaTeXIDE/rulesets/26284711 --input .github/rulesets/main-protection.json
```

Csak olvasó ellenőrzések:

```sh
gh api repos/szotya/LaTeXIDE/rulesets/26284711
gh api repos/szotya/LaTeXIDE/rules/branches/main
gh api repos/szotya/LaTeXIDE/branches/main --jq '{name,protected,sha:.commit.sha}'
gh api repos/szotya/LaTeXIDE/rules/branches/feature/p00-integration
gh api repos/szotya/LaTeXIDE/commits/main/check-runs
```

A létrehozás utáni Python-összehasonlítás minden kért konfigurációs mezőt egyeztetett a visszaolvasott rulesettel, a GitHub által hozzáadott alapértelmezett mezők megtartásával. Eredmény: 5 aktív szabály a `main` ágon, `protected: true`, 0 szabály a meglévő P00 munkabranchre; az előírt check a megfigyelt main SHA-n `success`. Az adminbypass tiltott. A `main`, a munkabranch és a meglévő #45 draft PR headje nem változott.

Ez API-konfigurációs és hatókör-ellenőrzés. A ruleset létrehozásakor tiltott push, törlés vagy merge kísérletét nem futtattuk, és távoli commit nem változott. A konfiguráció és az átadás külön dokumentációs PR-ba kerül a `szotya/branch-protection` branchről. Workflow és termékkód nem változik; új natív Mac/Windows/Linux kvalifikáció nem történt.

A célzás, az alkalmazáshoz kötött status check és a `strict` opció pontos jelentése: [GitHub REST ruleset-séma](https://docs.github.com/en/rest/repos/rules#create-a-repository-ruleset). Az útvonalszűrt required workflow korlátja: [GitHub required-check hibaelhárítás](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks).
