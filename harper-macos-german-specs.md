# Harper Desktop Deutsch für macOS – Implementierungs- und Release-Spezifikation

> Zielgruppe: Coding Agent auf einem echten Mac.  
> Ausgangspunkt: Branch `feature/german-language-support` aus [Dronakurl/harper](https://github.com/Dronakurl/harper/tree/feature/german-language-support), Stand `75c77b497b10cad3b79251a198de410a4b68418b` (Merge-Commit, 2026-10-14).  
> Upstream-Kontext: [Automattic/harper PR #3402](https://github.com/Automattic/harper/pull/3402).

---

## 1. Ziel

Aus dem genannten PR-Branch soll eine **auf macOS installierbare, testbare Harper-Desktop-Version mit sauberer deutscher Sprachunterstützung** entstehen.

Das Deliverable besteht aus:

1. Einer Desktop-App für **Apple Silicon und Intel** als Universal-App (`arm64` + `x86_64`).
2. Einer installierbaren `.dmg`.
3. Einer UI, in der Deutsch sichtbar, auswählbar und persistent ist – ohne dass ein Speichern Deutsch versehentlich wieder auf amerikanisches Englisch zurücksetzt.
4. Automatisierten Tests und GitHub Actions für PR-Builds.
5. Einer optionalen, von Geheimnissen getrennten Release-Strecke für Codesigning, Notarisierung und Updater-Artefakte.

### Definition of Done

- Auf einem Mac mit deutscher Systemregion startet Harper mit **Deutsch (Deutschland)** als sinnvoller Voreinstellung.
- Im Settings-Screen kann der Nutzer mindestens auswählen:
  - Automatisch (Systemsprache)
  - English (United States, United Kingdom, Australia, Canada, India)
  - Deutsch (Deutschland)
  - Deutsch (Österreich)
  - Deutsch (Schweiz)
- Die Auswahl wird persistent gespeichert und nach App-Neustart exakt wiederhergestellt.
- Die UI darf einen vom Rust-Backend gelieferten deutschen Wert nie als `American` fehlinterpretieren.
- Ein Universal-Build enthält wirklich `arm64` und `x86_64`.
- Pull Requests erzeugen einen **unsignierten**, installierbaren Test-DMG als GitHub-Artifact, ohne Release-Geheimnisse.
- Ein Version-Tag kann – nur bei vorhandenen Secrets – ein signiertes, notarisiertes DMG erstellen und veröffentlichen.
- Bestehende englische Funktionen und Konfigurationsdateien bleiben kompatibel.

---

## 2. Nicht-Ziele (bewusst nicht heimlich aufblasen)

- Kein App-Store-Release und kein App-Sandbox-Umbau. Harper nutzt macOS Accessibility/TCC für andere Apps; die vorgesehene Distribution ist **Developer ID außerhalb des App Stores**.
- Keine KI-/Cloud-Korrekturen. Die Prüfung bleibt lokal.
- Keine neue Grammatik-Engine: Die deutsche Engine aus PR #3402 ist Basis.
- Kein automatisches Erkennen der Sprache jedes einzelnen Absatzes in dieser Iteration. „Automatisch“ bedeutet **Systemsprache beim ersten Start bzw. explizit aktivierter Systemmodus**, nicht magische Dokument-Spracherkennung.
- Keine Veröffentlichung von Apple-, GitHub-, Tauri- oder Match-Secrets im Repository.

---

## 3. Bestand und relevante Befunde

### 3.1 Desktop-Stack

- Rust + Tauri 2: `harper-desktop/src-tauri/`
- Svelte 5 + TypeScript + Vite: `harper-desktop/`
- Package Manager: pnpm `10.10.0` (siehe `harper-desktop/package.json`)
- Desktop-Metadaten: `harper-desktop/src-tauri/tauri.conf.json`
  - Produktname: `Harper`
  - Bundle-ID: `com.elijahpotter.harper-desktop`
  - Bundling ist aktiv, Icon `.icns` existiert.
  - Updater ist aktiviert und enthält bereits den öffentlichen Tauri-Updater-Key.

### 3.2 Deutsche Engine ist bereits im PR vorhanden

- `harper-desktop/src-tauri/Cargo.toml`: Default-Feature `multilingual`, darin `de`, `pt`, `sk`, `pl`.
- Deutsches Sprachmodul: `harper-core/src/language/german/`.
- German dialects:
  - `GermanDialect::Standard` = Deutschland
  - `GermanDialect::Austrian` = Österreich
  - `GermanDialect::Swiss` = Schweiz
  - Quelle: `harper-core/src/language/german/dialects.rs`.
- BCP-47-Parsing unterstützt `de`, `de-DE`, `de-AT`, `de-CH`:
  - Quelle: `harper-core/src/language/languages.rs:27-80`.
- Das Rust-Desktop-Config-Objekt initialisiert derzeit die Sprache via macOS-Systemlocale:
  - `harper-desktop/src-tauri/src/config/mod.rs:37-57`.
- Es gibt bereits deutschbezogene Desktop-Rust-Tests, u. a. für Wörter wie `Haus` und `Freiheit`:
  - `harper-desktop/src-tauri/src/config/mod.rs:546-558`.

### 3.3 Der vorhandene UI-Fehler

Die Frontend-Schicht ist noch englisch-only:

- `harper-desktop/src/lib/client.ts:12-20` kennt nur `American | British | Australian | Canadian | Indian`.
- `rustDialectToDialect()` fällt für **jeden unbekannten Wert** auf `American` zurück (`client.ts:247-275`).
- `harper-desktop/src/lib/settings/settings-data.ts:121-127` zeigt ausschließlich englische Optionen.
- `harper-desktop/src/lib/settings/pages/GeneralPage.svelte:353-379` nennt das Feld sogar `English dialect`.
- Das Backend akzeptiert hingegen bereits ein komplettes `Language`-Enum (`src-tauri/src/commands.rs:151-164`).

**Folge:** Ein deutscher Backend-Wert kann durch die UI falsch angezeigt und beim Speichern überschrieben werden. Das ist der zentrale Bug dieser Umsetzung.

---

## 4. Architekturentscheidung: stabiler Sprach-DTO statt englischem Frontend-Enum

### Pflichtentscheidung

**Nicht** die bestehenden `harper.js`-`Dialect`-Integerwerte als allgemeines Desktop-Sprachmodell missbrauchen. Diese sind englisch-zentriert und der Grund für den Fehler.

Stattdessen ein explizites, versionsstabiles Tauri-IPC-DTO einführen, z. B.:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSettingDto {
    pub mode: LanguageModeDto, // "system" | "explicit"
    pub language: Option<String>, // z. B. "de", "en"
    pub region: Option<String>,   // z. B. "DE", "AT", "CH", "US", "GB"
}
```

Die exakte Form darf besser gewählt werden, muss aber diese Eigenschaften erfüllen:

- JSON darf keine Rust-Enum-Interna (`{"German":"Standard"}`) als öffentliche Frontend-API erzwingen.
- Der DTO ist validierbar und eindeutig.
- Ungültige Kombinationen werden Rust-seitig mit einer verständlichen Fehlermeldung abgelehnt.
- Konfiguration auf Disk bleibt rückwärtskompatibel zu existierenden `Language`-Werten oder erhält eine klar getestete Migration.
- Die Frontend-Auswahl wird nicht durch einen stillen Fallback auf Amerikanisch verfälscht.

### Empfohlene Konfigurationssemantik

Erweitere die persistente Konfiguration konzeptionell von nur `dialect: Language` auf eine Auswahl:

```rust
enum LanguageSelection {
    System,
    Explicit(Language),
}
```

- `System`: bei jedem App-Start die aktuelle OS-BCP-47-Locale über `tauri_plugin_os::locale()` und `parse_language()` auflösen; bei unbekannter Locale auf bisherigen Default zurückfallen.
- `Explicit(Language)`: Benutzerwahl bleibt stabil, auch wenn macOS-Region sich ändert.
- Für alte Config-Dateien mit `dialect` muss eine Migration auf `Explicit(<alter Wert>)` erfolgen, damit bestehende Nutzer nicht überrascht werden.
- Falls eine risikofreie Migration im bestehenden Serialisierungsformat nicht möglich ist, darf `dialect` intern erhalten bleiben, aber es muss ein zusätzliches, klar dokumentiertes `language_mode` geben. Teste alte Konfigurationsfixtures zwingend.

---

## 5. Funktionale Anforderungen

### A. Sprachwahl in Settings

Ersetze die englisch-only-Auswahl durch einen neutralen Abschnitt **Language**.

Mindestoptionen und Labels:

```text
Automatic (System Language)
English (United States)
English (United Kingdom)
English (Australia)
English (Canada)
English (India)
Deutsch (Deutschland)
Deutsch (Österreich)
Deutsch (Schweiz)
```

Weitere im Build verfügbare Sprachen (Portugiesisch, Polnisch, Slowakisch) dürfen mit aufgenommen werden, aber nur wenn sie vollständig korrekt modelliert und getestet werden. Für dieses Deliverable haben Deutsch und bestehendes Englisch Priorität.

Verhalten:

- Beim Laden: Backend-Wert ohne Informationsverlust anzeigen.
- Bei Änderung: erst Backend schreiben, dann UI-State bestätigen; Fehler sichtbar anzeigen und den vorherigen gültigen Wert behalten.
- Während Laden/Speichern: Control deaktivieren wie bisher.
- Deutsche Labels müssen nicht die gesamte App lokalisieren; eine deutsche Sprachwahl ist ausreichend.

### B. Backend-IPC

- Neue Commands, z. B. `get_language_selection` und `set_language_selection`, oder bestehende `get_dialect`/`set_dialect` kompatibel erweitern.
- Alte Commands nur entfernen, wenn alle Aufrufer im Repository migriert sind und ein Suchlauf dies belegt.
- Alle öffentlichen Command-Inputs validieren. Keine untrusted String-to-enum-Konvertierung mit `unwrap()`.
- Unit-Tests für:
  - Systemmodus mit `de-DE`, `de-AT`, `de-CH`.
  - explizites Deutsch DE/AT/CH.
  - alle vorhandenen englischen Varianten.
  - unbekannte Locale / ungültiger DTO.
  - Persistenz und Neustart-/Reload-Semantik.
  - Migration aus einer alten Konfiguration.

### C. Deutsche Funktionsprüfung

Die Testfälle müssen beweisen, dass die Sprachwahl die tatsächlich genutzte Harper-Konfiguration beeinflusst, nicht nur einen Dropdown-Wert.

Mindesttests:

- Deutsch `Standard` baut ein deutsches Dictionary und erkennt bekannte deutsche Wörter (bestehender Test kann erweitert werden).
- Eine gezielt falsche deutsche Schreibweise bzw. ein einfacher deutscher Regeltest erzeugt einen Lint.
- Ein deutsches korrektes Beispiel erzeugt keinen offensichtlichen englischen False Positive.
- Englisch bleibt nach expliziter Sprachwahl Englisch.

Nutze reale vorhandene deutsche Regeln und Fixtures aus `harper-core/src/language/german/tests/`; keine leeren Mock-Tests.

### D. Frontend-Tests

Aktuell gibt es im Desktop-Paket keine Frontend-Tests. Ergänze eine schlanke Testumgebung passend zur bestehenden Toolchain (z. B. Vitest + Testing Library; keine riesige Testarchitektur für einen Selector).

Mindestens testen:

- DTO → UI-Wert: `de/DE`, `de/AT`, `de/CH`, englische Werte und Systemmodus.
- UI-Wert → gültiger DTO.
- Keine unbekannte Backend-Sprache wird still zu `American`.
- Language Selector zeigt die korrekten Optionen und ruft beim Speichern den erwarteten Tauri-Command auf.

`pnpm check` bleibt Pflicht; füge einen expliziten `pnpm test`-Befehl hinzu.

---

## 6. macOS Build und Paketierung

### Vorbedingungen auf dem Mac

- Aktuelles Xcode bzw. Xcode Command Line Tools:
  ```bash
  xcode-select --install
  ```
- Rustup/Rust Stable.
- Node entsprechend Repository-Anforderungen; Corepack aktivieren.
- pnpm exakt/kompatibel zu `packageManager` im `package.json`.
- `just`.
- `wasm-pack`, falls der bestehende `just`-Build ihn benötigt.
- Tauri-Systemvoraussetzungen gemäß Tauri 2 macOS-Dokumentation.

Beispiel-Setup (Versionen vor Ausführung gegen Projekt-Lockfiles prüfen):

```bash
xcode-select --install
rustup toolchain install stable
rustup target add aarch64-apple-darwin x86_64-apple-darwin
corepack enable
corepack prepare pnpm@10.10.0 --activate
brew install just
cargo install wasm-pack
```

### Existierende, relevante Build-Rezepte

In `justfile`:

```bash
just check-desktop
just test-rust
just build-desktop-macos-arm64
just build-desktop-macos
just build-desktop-macos-unsigned
```

- `build-desktop-macos-arm64`: Apple-Silicon `.app` + `.dmg`.
- `build-desktop-macos`: verlangt ein Universal-Bundle über `universal-apple-darwin`.
- `build-desktop-macos-unsigned`: Universal-App/DMG ohne Updater-Artefakte; korrekt für PR-Tests ohne Secrets.

**Wichtig:** Das Rezept installiert die beiden Rust-macOS-Targets nicht selbst. Vor einem Universal-Build zwingend:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### Lokale Build-Abnahme

```bash
just check-desktop
just test-rust
just build-desktop-macos-unsigned
```

Danach Pfade nicht raten: `find harper-desktop/src-tauri/target -name '*.dmg' -o -name 'Harper.app'` nutzen und die resultierenden Pfade in CI konsistent übernehmen.

Prüfen:

```bash
APP="<Pfad-zu-Harper.app>"
DMG="<Pfad-zur-Harper.dmg>"

lipo -archs "$APP/Contents/MacOS/Harper"
# Erwartet: arm64 x86_64

hdiutil attach "$DMG" -nobrowse -readonly
# App im gemounteten Volume finden, dann:
codesign --verify --deep --strict --verbose=4 "$APP"
# Bei unsigniertem PR-Build ist Gatekeeper-/spctl-Erfolg nicht erwartbar.
hdiutil detach "<mount-point>"
```

Ein realer Smoke-Test ist Pflicht: DMG auf einem sauberen macOS-Benutzerkonto öffnen, Harper nach `/Applications` kopieren, starten, Settings öffnen, Deutsch auswählen, App beenden/starten und Auswahl prüfen.

---

## 7. GitHub Actions

### Problem im Branch

`harper-desktop/.github/workflows/build_artifacts.yml` wird von GitHub **nicht ausgeführt**, da GitHub Workflows ausschließlich in der Repository-Wurzel unter `.github/workflows/` erkennt. Außerdem ruft es `just build-macos` auf; das aktuelle Rezept heißt `build-desktop-macos`.

### Zu erstellen: `.github/workflows/desktop_macos.yml`

#### PR-Job (keine Secrets)

Trigger:

- `pull_request`
- `workflow_dispatch`

Aufgaben:

1. Checkout mit fest gepinnten Action-Versionen/SHAs nach Repository-Konvention.
2. Node/pnpm, Rust stable, `rustfmt`, `clippy`, `just`, `wasm-pack` installieren.
3. `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
4. `pnpm install --frozen-lockfile` bzw. die projektnative Installation verwenden.
5. `just check-desktop`.
6. Die neuen Frontend-Tests ausführen.
7. Deutsche Rust-Tests und Desktop-Tests ausführen; mindestens den äquivalenten Teil von `just test-rust`.
8. `just build-desktop-macos-unsigned`.
9. Prüfen, dass App und DMG existieren.
10. `lipo -archs` ausführen und explizit auf `arm64` und `x86_64` prüfen.
11. DMG mounten und sicherstellen, dass `Harper.app` enthalten ist.
12. DMG und optional einen SHA-256-Checksum-File als Actions Artifact hochladen.

Der PR-Job darf **nie** Apple-, Notarisierungs-, GitHub-Release- oder Tauri-Private-Key-Secrets beziehen. Bei Fork-PRs müssen Secrets ebenfalls ausgeschlossen sein.

#### Release-Job (nur Tags / geschützte manuelle Ausführung)

Trigger: `push.tags: ['v*']` sowie optional geschütztes `workflow_dispatch`.

Zusatzschritte:

1. Nur nach erfolgreichem Build/Test.
2. Zertifikat über vorhandenen Fastlane-Lane importieren:
   ```bash
   bundle exec fastlane set_up_signing
   ```
3. Mit Developer-ID und Hardened Runtime bauen/signieren. Keine pauschale `codesign --deep`-Reparatur verwenden; verschachtelte Binaries korrekt im Build-Prozess signieren.
4. Tauri-Updater-Private-Key ausschließlich als Action Secret bereitstellen (typisch `TAURI_SIGNING_PRIVATE_KEY` und ggf. Passwort). Der im `tauri.conf.json` vorhandene **öffentliche** Key bleibt öffentlich; private Key niemals committen.
5. Das fertige DMG notarisieren und staplen:
   ```bash
   bundle exec fastlane notarize_macos package:"$DMG"
   xcrun stapler validate "$DMG"
   spctl --assess --type open --context context:primary-signature -vv "$DMG"
   codesign --verify --deep --strict --verbose=4 "$APP"
   ```
6. Erst nach erfolgreicher Validierung Release-Assets veröffentlichen.
7. Updater-Artefakt und Signatur veröffentlichen, nur wenn der konfigurierte Endpoint tatsächlich dieses Tauri-Format ausliefert.

### Secrets: nur als Namen dokumentieren, nie als Werte

Bestehende Fastlane-Konfiguration nennt:

- `MATCH_S3_ACCESS_KEY`
- `MATCH_S3_SECRET_ACCESS_KEY`
- `MATCH_PASSWORD`
- `APP_STORE_CONNECT_API_KEY_KEY_ID`
- `APP_STORE_CONNECT_API_KEY_ISSUER_ID`
- `APP_STORE_CONNECT_API_KEY_KEY`
- `GITHUB_TOKEN`
- Tauri-Updater-Signing-Key (+ optional Passwort)

Die vorhandene `fastlane/Fastfile` enthält eine Upstream-Team-ID und ein Upstream-S3-Backend. In einem eigenen Fork **nicht übernehmen oder voraussetzen**. Stattdessen eigene Apple-Team-ID, eigenes Zertifikatsmanagement, eigenes Release-Repo und eigene Secrets konfigurieren.

---

## 8. Tests und Qualitätsgates

### Test-first-Regel

Für jede neue Produktionsänderung:

1. Test schreiben.
2. Test bewusst fehlschlagen sehen.
3. Minimal implementieren.
4. Spezifischen Test und danach vollständige relevante Suite ausführen.

### Verbindliche Checks vor Übergabe

```bash
pnpm --dir harper-desktop check
pnpm --dir harper-desktop test
just check-desktop
just test-rust
just build-desktop-macos-unsigned
```

Zusätzliche Abnahme:

- `cargo fmt -- --check` und `cargo clippy --all-targets -- -Dwarnings ...` via `just check-desktop-rust`.
- Kein neuer TypeScript- oder Svelte-Check-Fehler.
- Kein Config-Datenverlust beim Upgrade einer alten Config.
- Native Smoke-Tests auf echter Apple-Silicon- und echter Intel-Hardware oder expliziten nativen macOS-Runnern. Rosetta allein zählt nicht als Intel-Nachweis.
- Release: Prüfung auf einem sauberen Mac-Benutzerprofil; Gatekeeper muss das notarisiert signierte DMG akzeptieren.
- Accessibility-Flow manuell dokumentieren: Nutzer muss Harper unter **System Settings → Privacy & Security → Accessibility** freigeben. Notarisierung erteilt diese TCC-Berechtigung nicht automatisch.

---

## 9. Implementierungsreihenfolge und kleine Commits

1. **Branch/Toolchain reproduzierbar machen**
   - Fork/Clone, alle Basistests ausführen, Ausgangsfehler dokumentieren.
   - Commit nur falls Toolchain-/CI-Voraussetzungen angepasst werden.

2. **Rust-Sprachselektion modellieren**
   - Test für DTO/Systemmodus/Migration schreiben (RED).
   - Selection/Config/IPC implementieren (GREEN).
   - Bestehende Config-Kompatibilität testen.
   - Commit: `feat(desktop): model language selection independently of English dialects`

3. **Svelte/TypeScript migrieren**
   - Frontend-Testumgebung + fehlschlagende Tests für Deutsch/Systemmodus.
   - `client.ts`, `settings-data.ts`, `GeneralPage.svelte` auf Sprach-DTO umstellen.
   - Englischen Fallback entfernen; unbekannte Antwort als sichtbarer Fehler, nicht als „American“ behandeln.
   - Commit: `feat(desktop): expose German language variants in settings`

4. **End-to-end-nahe Backend-/Config-Tests**
   - Deutsche Wörter/Regeln + Persistenz + Neuinitialisierung testen.
   - Commit: `test(desktop): cover German language persistence and linting`

5. **macOS CI für unsignierte Artefakte**
   - Root-Workflow hinzufügen, Mac Targets installieren, Prüfungen/DMG/Lipo/Mount durchführen.
   - Artifact inklusive Checksums hochladen.
   - Commit: `ci: build and verify universal macOS desktop artifacts`

6. **Signierte Release-Pipeline (optional, wenn Secrets vorliegen)**
   - Geschützten Release-Job ergänzen.
   - Signatur/Notarisierung/Stapelung/Gatekeeper-Checks.
   - Commit: `ci: notarize macOS desktop releases`

7. **Dokumentation**
   - README oder `docs/macos-release.md`: lokale Build-Schritte, Troubleshooting, Accessibility-Erlaubnis, Unterschiede zwischen Test- und Release-DMG.
   - Commit: `docs: document macOS German desktop build and release`

---

## 10. Quellen

### Projekt / PR

- [Automattic/harper](https://github.com/Automattic/harper)
- [PR #3402 – add multi-language support (German, Portuguese)](https://github.com/Automattic/harper/pull/3402)
- [Feature-Branch `feature/german-language-support`](https://github.com/Dronakurl/harper/tree/feature/german-language-support)

### Lokale Quellpfade im Ausgangsbranch

- `justfile:175-255` – Desktop-Checks und macOS-Build-Rezepte.
- `justfile:571-585` – Rust-/Desktop-Testablauf.
- `harper-desktop/src-tauri/tauri.conf.json` – Tauri-Bundle, Bundle-ID, Icons, Updater.
- `harper-desktop/src-tauri/Cargo.toml:96-107` – `multilingual`/`de` als Default-Features.
- `harper-desktop/src-tauri/src/config/mod.rs:37-57` – Systemlocale-Initialisierung.
- `harper-desktop/src-tauri/src/commands.rs:151-164` – vorhandener `set_dialect`-Command mit `Language`.
- `harper-desktop/src/lib/client.ts:12-20, 80-86, 247-275` – englisch-only IPC und fehlerhafter Fallback.
- `harper-desktop/src/lib/settings/settings-data.ts:121-127` – aktuelle Optionsliste.
- `harper-desktop/src/lib/settings/pages/GeneralPage.svelte:353-379` – aktueller Selector.
- `harper-core/src/language/languages.rs:27-80, 400-435` – BCP-47-Mapping und `Language`-Enum.
- `harper-core/src/language/german/dialects.rs` – DE/AT/CH-Dialekte.
- `harper-desktop/.github/workflows/build_artifacts.yml` – nicht wirksamer, veralteter Desktop-Mac-Workflow; nur als Referenz, nicht kopieren.
- `fastlane/Fastfile` – vorhandene Upstream-Vorlage für Developer-ID-Signing, Notarisierung, Release; nicht mitsamt Upstream-Infrastruktur übernehmen.

### Externe Dokumentation (bei Umsetzung gegen aktuelle Versionen prüfen)

- [Tauri v2 – macOS code signing](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri v2 – Bundling](https://v2.tauri.app/distribute/)
- [Apple – Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [GitHub Actions – Workflow syntax](https://docs.github.com/actions/writing-workflows/workflow-syntax-for-github-actions)

---

## 11. Übergabeformat vom Coding Agent

Der Agent liefert am Ende:

1. Link zum Fork/PR sowie Commit-Hash.
2. Liste veränderter Dateien und Begründung.
3. Exakte Befehle plus vollständige Testausgaben/Zusammenfassung.
4. Link zum unsignierten PR-DMG-Artifact und SHA-256.
5. Architekturprüfung (`lipo`) und DMG-Mount-Prüfung.
6. Falls Signierung möglich war: `codesign`, `spctl`, `stapler`-Nachweise sowie Release-URL.
7. Offene Risiken klar benennen – insbesondere fehlende Apple-Credentials, nicht getestete Intel-Hardware oder noch nicht bereitstehender Updater-Server.

Keine Behauptung wie „macOS-ready“, wenn nur ein Cross-/Universal-Build erzeugt wurde. Die App gilt erst als produktionsbereit, wenn sie auf sauberem macOS nativ installiert, gestartet und Gatekeeper-validiert wurde.
