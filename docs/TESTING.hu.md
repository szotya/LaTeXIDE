# Tesztelői útmutató: hiba, hiányzó funkció és új ötlet bejelentése

**Itt kezdj:** [új bejelentés a GitHubon](https://github.com/szotya/LaTeXIDE/issues/new/choose) · [korábbi bejelentések](https://github.com/szotya/LaTeXIDE/issues?q=is%3Aissue)

Az **issue** egy bejelentés saját címmel, leírással és hozzászólásokkal. Nem kell programoznod, ismerned a hiba okát vagy a projekt belső kódjait. A saját tapasztalatodat írd le, lehetőleg úgy, hogy más is megértse és ki tudja próbálni.

## 1. Melyik űrlapot válaszd?

| Amit tapasztalsz | Ezt válaszd | Példa |
| --- | --- | --- |
| Egy meglévő funkció hibásan működik. | [Hibajelentés](https://github.com/szotya/LaTeXIDE/issues/new?template=bug.yml) | Elfogadod a kiegészítési javaslatot, de a lista a képernyőn marad. |
| Szükséged lenne valamire, de nem találod, vagy nem érhető el. | [Hiányzó funkció](https://github.com/szotya/LaTeXIDE/issues/new?template=missing-function.yml) | A tesztfeladat teljes projektre kiterjedő keresést kér, de csak az aktuális fájlban tudsz keresni. |
| Új lehetőséget vagy egy meglévő funkció továbbfejlesztését javaslod. | [Új funkcióötlet](https://github.com/szotya/LaTeXIDE/issues/new?template=feature.yml) | Saját sablonból szeretnél új dokumentumot létrehozni. |

**Ha bizonytalan vagy, attól még küldd be.** Hiányzó képességnél válaszd a **Hiányzó funkció** űrlapot, és írd bele: „Nem tudom, hogy ez már része-e ennek a verziónak.” A fejlesztők pontosítják a besorolást. Ha egy korábban működő funkció eltűnt, azt hibaként is jelentheted, a korábbi verzió megadásával. Egy tervben szereplő funkció nem feltétlenül készült már el.

A **Csomagfeladat** űrlap fejlesztői munkaszervezéshez készült; tesztelőként a fenti három közül válassz.

## 2. Bejelentés lépésről lépésre

1. **Jelentkezz be a GitHub-fiókodba.** A nyilvános repóban az issue beküldéséhez nem kell fejlesztői meghívás, forkolás vagy kódmódosítás.
2. **Keress rá a problémára a [korábbi issue-k között](https://github.com/szotya/LaTeXIDE/issues?q=is%3Aissue).** A nyitott és lezárt jegyeket is nézd meg. Ha már van ugyanilyen, hozzászólásban add hozzá a saját tapasztalatodat és a tesztelt verziót. Egy „nálam is” helyett írd le, mit és melyik gépen próbáltál.
3. **Nyisd meg az [Issues → New issue](https://github.com/szotya/LaTeXIDE/issues/new/choose) oldalt**, és válaszd ki a megfelelő űrlapot. A gomb neve a GitHub felületétől függően **Get started** is lehet.
4. **Adj konkrét címet.** Például: `[Hiba] Enter után nyitva marad a kiegészítési lista`. A „Nem működik” vagy „Van egy ötletem” önmagában kevés.
5. **Töltsd ki a mezőket a következő szakasz szerint.** Egy bejelentés egy önálló problémáról vagy ötletről szóljon. Ha valamit nem tudsz, írd: `nem ismert`, `nem találtam` vagy `nem teszteltem`.
6. **Olvasd át, majd kattints a Create vagy Submit new issue gombra.** Beküldés után őrizd meg a jegy linkjét; ugyanott tudsz válaszolni és további adatot hozzáadni.

Nem kell címkét, felelőst, mérföldkövet, prioritást vagy csomagazonosítót megadnod. A beküldés nem ad írási vagy merge-jogot a repóhoz.

## 3. Mit írj a bejelentésbe?

### Hiba: mit csináltál, mi történt, mit vártál?

Az űrlap a következőket kéri:

- **Tesztelt verzió és gép:** az alkalmazás verziója vagy a kapott tesztcsomag pontos fájlneve; operációs rendszer és verzió; géptípus, ha ismert. A „legújabb verzió” helyett konkrét azonosítót írj. Forráskód-azonosító (SHA) és fejlesztői eszközverzió nem kötelező.
- **A hiba megismétlésének lépései:** miből indultál, melyik fájl volt nyitva, mit kattintottál vagy gépeltél? Számozd a lépéseket. Billentyűhibánál a billentyűzetkiosztást és az aktív beviteli mezőt is nevezd meg.
- **Tapasztalt eredmény:** pontosan mi történt? A hibaüzenetet lehetőleg szó szerint másold be.
- **Elvárt eredmény:** minek kellett volna történnie? Ha tesztfeladat írja elő, hivatkozhatsz rá.
- **Gyakoriság, hatás és kerülőút:** egyszer láttad vagy minden próbánál? Például `2 hibás eset / 3 próba`. Akadályozza a munkát, vagy átmenetileg megkerülhető?
- **Melléklet vagy hivatkozás:** opcionális kép, rövid videó, naplórészlet vagy kicsi mintaprojekt. Melléklet nélkül is bejelenthető a hiba.

**Kitalált kitöltési példa, nem tényleges teszteredmény:**

> Cím: `[Hiba] Enter után nyitva marad a kiegészítési lista`
>
> Verzió és gép: `DEMO-001` csomag, macOS 15, Apple M2, magyar billentyűzet.
>
> Lépések: 1. Megnyitok egy másolt `main.tex` fájlt. 2. A szerkesztőben beírom: `\sect`. 3. Kijelölöm a `\section` javaslatot és Entert nyomok.
>
> Tapasztalat: a parancs bekerül, de a lista nyitva marad.
>
> Elvárás: a lista bezárul, és folytathatom a gépelést.
>
> Gyakoriság és hatás: 2/2 próba; a lista takarja a szöveget, Escape-pel bezárható. Adatvesztést nem tapasztaltam.

**Adatvesztés vagy fájlsérülés gyanújánál előbb őrizd meg a megmaradt munkát.** Ne ismételd meg a veszélyes műveletet az eredeti dokumentumon csak a bejelentés kedvéért. Írd le, amit egyszer láttál; új próbát csak külön másolaton végezz.

### Hiányzó funkció: mit keresel, és miért számítottál rá?

Az űrlap a következőket kéri:

- **Tesztelt verzió és gép:** ugyanazok az adatok, mint a hibánál; az ismeretlent jelöld.
- **Hiányzó lehetőség és keresés helye:** milyen feladatot szeretnél elvégezni, melyik menüben, panelen vagy munkafolyamatban kerested a funkciót?
- **Kívánt működés:** mit kellene tudnod megtenni, és mi lenne a látható eredmény?
- **Miért számítottál rá?** Opcionálisan tesztfeladat, dokumentáció, korábbi verzió vagy másik platform. Linket is adhatsz. Ha csak a saját elvárásod, azt is nyugodtan írd le.
- **Hatás és jelenlegi megoldás:** milyen gyakran kellene, mit akadályoz a hiánya, és hogyan oldod meg most?
- **Melléklet vagy hivatkozás:** opcionális kép arról, hol kerested, vagy kapcsolódó issue.

**Kitalált kitöltési példa, nem jelenlegi funkcióígéret:**

> Cím: `[Hiányzó funkció] Nem találom a teljes projektben keresést`
>
> Verzió és gép: `DEMO-001` csomag, macOS 15, Apple M2.
>
> Feladat és keresés: két `.tex` fájlban keresnék ugyanarra a szóra; a keresőpanelt és a menüt néztem meg.
>
> Kívánt működés: minden érintett fájlból lássam a találatokat fájlnévvel és sorszámmal.
>
> Miért vártam: a kapott mintateszt ezt kérte; nem tudom, hogy ebben a csomagban már elérhető-e.
>
> Hatás: több fájl javításánál kellene; most egyenként keresek bennük.

### Új ötlet: milyen problémát oldana meg?

Az űrlap a következőket kéri:

- **Probléma és jelenlegi megoldás:** milyen munka közben lenne hasznos, milyen gyakran, és hogyan oldod meg most?
- **Javasolt működés:** rövid használati példa. Írd le a legkisebb már hasznos változatot; az extra lehetőségeket külön jelezd.
- **Miből látnád, hogy jól működik?** Egy-két konkrét, kipróbálható eredmény. Nem kell technikai tesztet írnod.
- **Érintett platform:** macOS, Windows, Linux vagy mindegyik; ha a jelenlegi alkalmazásból indulsz ki, a tesztelt verziót is add meg, ha ismert.
- **Melléklet vagy hivatkozás:** opcionális vázlat, kép, más alkalmazásból vett példa vagy kapcsolódó issue.

**Kitalált kitöltési példa, nem elfogadott fejlesztési feladat:**

> Cím: `[Ötlet] Új dokumentum létrehozása saját sablonból`
>
> Probléma: minden új jegyzethez ugyanazt a fejlécet másolom át kézzel.
>
> Javaslat: saját sablont választhassak új dokumentum létrehozásakor. Elsőre egyetlen helyi sablon is elég lenne.
>
> Ellenőrizhető eredmény: a sablon szövege bekerül az új dokumentumba; az eredeti sablon változatlan marad.
>
> Platform: macOS; más platformon nem próbáltam.
>
> Jelenlegi megoldás: egy korábbi dokumentumot másolok le, és törlöm belőle a régi tartalmat.

Ötletnél a várható hasznot írd le; nem kell S1–S4 hibasúlyosságot vagy fejlesztési határidőt kitalálnod. A fejlesztők döntenek arról, mi valósul meg és mikor.

## 4. Képek, videók és mintafájlok

A GitHub-űrlap **Melléklet vagy hivatkozás** mezőjébe húzhatsz képet vagy támogatott fájlt; a szövegmező alatti csatolási lehetőséget is használhatod. A kép mellett írd le szövegben is a lényeget.

**A bejelentés és a csatolmány nyilvános.** Saját dolgozat vagy teljes projekt helyett lehetőleg néhány soros, kitalált mintát küldj. Ne kerüljön bele jelszó, API-kulcs, hozzáférési token, személyes adat vagy privát beszélgetés. Naplóból csak a szükséges részt másold be. Kitakarás után ellenőrizd újra a képet vagy fájlt.

Csak arról a verzióról és platformról írj teszteredményt, amelyet ténylegesen kipróbáltál. Másik operációs rendszeren ugyanaz a funkció eltérő állapotban lehet.

## 5. Ha nem jelenik meg a megfelelő űrlap

Menj vissza az [űrlapválasztóhoz](https://github.com/szotya/LaTeXIDE/issues/new/choose). Ha ott sem találod, válaszd az **Open a blank issue** lehetőséget, és másold be az alábbiak közül a megfelelőt. A címet külön add meg. Ha a GitHub-fiókoddal sem tudsz issue-t nyitni, a kitöltött szöveget add át a tesztelés szervezőjének.

**Hiba:**

```text
Tesztelt verzió és gép:
A hiba megismétlésének lépései:
1.
2.
Tapasztalt eredmény:
Elvárt eredmény:
Gyakoriság, hatás és kerülőút:
Melléklet vagy hivatkozás (ha van):
```

**Hiányzó funkció:**

```text
Tesztelt verzió és gép:
Hiányzó lehetőség és keresés helye:
Kívánt működés:
Miért számítottam rá? (ha ismert):
Hatás és jelenlegi megoldás:
Melléklet vagy hivatkozás (ha van):
```

**Új ötlet:**

```text
Probléma és jelenlegi megoldás:
Javasolt működés, első hasznos változat:
Miből látnám, hogy jól működik?
Érintett platform, ismert verzió:
Melléklet vagy hivatkozás (ha van):
```

## 6. Beküldés után

A kérdésekre ugyanabban a jegyben válaszolj. Ha közben új részletet tudsz meg, hozzászólásban add hozzá. Ha javított verziót kapsz, írd meg a pontos verzióját, az ismételt lépéseket és az eredményt: **javult / továbbra is hibás / nem teszteltem**. Másik, önálló problémához nyiss külön issue-t.

A lezárt jegy nem mindig jelent elkészült javítást: lehet ismétlődő bejelentés, ismert korlát vagy elutasított ötlet is. Nézd meg a lezárás indokát és az esetleges kapcsolódó jegyet. A beküldés önmagában nem jelent elfogadást, ütemezést vagy határidőígéretet.

Beküldés előtt gyorsan ellenőrizd:

- [ ] Egy önálló ügyet írtam le, konkrét címmel.
- [ ] A saját tapasztalatom és a kívánt eredmény érthető.
- [ ] A verziót és a platformot megadtam, vagy jelöltem az ismeretlen adatot.
- [ ] Nincs benne privát vagy titkos adat.

A GitHub felületének általános leírása: [issue létrehozása a GitHub dokumentációjában](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/creating-an-issue).
