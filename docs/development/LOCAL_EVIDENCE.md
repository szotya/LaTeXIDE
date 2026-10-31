# Helyben megőrzött történeti bizonyítékok

## Retained locally

A privát repository kezdeti importja a teljes termékforrást, teszteket, terveket és fejlesztői dokumentációt tartalmazza. A nyers korábbi futási naplók, képernyőképek, generált compilerkimenetek, másolt forráspillanatképek és helyi környezeti metaadatok **nem kerülnek fel GitHubra**. Az eredeti munkamappában változatlanul megmaradtak.

A tételes [helyi artifactjegyzék](local-evidence.json) repository-relatív útvonalat, méretet és SHA-256-ot közöl a tartalom helyett. `.latexide` sessiondescriptort és cache-t még ez a jegyzék sem tartalmaz. Az [átirányítási jegyzék](evidence-link-migration.json) őrzi az egyes történeti dokumentumlinkek eredeti célját; így egy logcím egyértelműen visszakereshető. A történeti eredményállítások nem változtak, a repositorykiadás linkjei ezt a helyi bizonyítékhatárt jelölik.

A GitHubon olvasható korábbi PASS-adat dokumentált történeti megfigyelés; nyers napló nélkül a clone-ból önmagában nem újraellenőrizhető. A terméktesztek forrása és a CI reprodukciós lehetősége a repó része, az új futásnak saját commit-/platform-/fixture-azonosságot kell adnia. A történeti auditvalidator teljes evidencefával működik, nem tiszta clone CI-kapu.

Kivételként csak három auditsegéd Python-forrás, a TeX Live két nyilvánosan aláírt SHA-512 metadata-párja, ASCII publikus kulcsa és publikus GPG-keyringje verziózott. Ezek a reprodukáló kódhoz/installer-eredetellenőrzéshez kellenek; nem sessionadatok vagy titkos kulcsok. Az `evidence/README.md` felsorolja a határt.

A kezdeti szélesebb import push-át az automatikus jóváhagyási ellenőrzés visszautasította a nyers evidence/helyi metaadatok lehetséges érzékenysége miatt. A feltöltési készletet ezért forrásra/dokumentációra és erre a pontos kilenc kivételre szűkítettük. Nincs nyers napló-/sessionfeltöltés és nincs a visszautasított payloadot tartalmazó korábbi commit.
