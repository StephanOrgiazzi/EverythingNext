# Everything Modern

Client Windows moderne pour **Everything 1.5**, construit en Rust avec **Tauri 2 + Leptos**. L’installateur embarque Everything **1.5.0.1418b x64** et **Everything SDK3 3.0.0.9** : aucune installation préalable d’Everything n’est nécessaire.

## Fonctionnalités du MVP

- recherche Everything avec syntaxe native et debounce de 55 ms ;
- connexion SDK3 explicite par named pipe, avec client, état de recherche et listes de résultats dédiés ;
- instance privée nommée `EverythingModern`, indépendante d’une éventuelle instance Everything classique ;
- invalidation logique des anciennes générations côté frontend **et backend Rust** ;
- pagination SDK3 par viewport, par lots de 256 résultats, et cache glissant de huit pages ;
- liste virtualisée, tri nom/chemin/taille/date et icônes Shell progressives avec cache hybride par extension ou chemin sensible ;
- sélection logique par plages : simple, Ctrl, Maj, grandes plages et `Ctrl+A` sans charger tous les résultats ;
- ouvrir, révéler, copier via le presse-papiers natif, renommer et déplacer vers la Corbeille ;
- dialogues applicatifs de renommage et de confirmation ;
- thème Windows clair/sombre et barre de titre personnalisée ;
- restauration automatique de la taille et de la position de fenêtre ;
- moteur et SDK3 officiels vérifiés puis embarqués dans l’installateur NSIS ;
- métrique visible `UI x ms` mesurant réponse IPC → prochaine passe de rendu.

## Installation utilisateur

Téléchargez et exécutez l’installateur NSIS. L’installation est effectuée pour la machine sous `Program Files` et demande les droits administrateur afin que le binaire du service d’indexation privé ne soit pas modifiable par un utilisateur standard.

Le moteur embarqué :

- fonctionne sans installation séparée d’Everything ;
- n’affiche ni fenêtre ni icône de notification ;
- stocke sa configuration et sa base dans `%LOCALAPPDATA%\EverythingModern\Engine` ;
- lance son client avec l’application et l’arrête lorsque l’application quitte réellement ;
- conserve le service d’indexation léger en arrière-plan entre deux lancements ;
- utilise une instance, un pipe IPC3 et un service distincts de l’Everything classique.

Everything classique peut donc rester installé et lancé simultanément. Les deux applications conservent leurs propres processus, bases et réglages.

## Prérequis de développement

- Windows 11 x64 ;
- Rust stable **MSVC** avec la cible `wasm32-unknown-unknown` ;
- Visual Studio 2022 Build Tools, charge de travail « Développement Desktop en C++ » et Windows 10/11 SDK ;
- WebView2, inclus par défaut dans Windows 11.

Everything 1.4 et les anciens SDK ne sont pas pris en charge.

## Installation rapide

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup.ps1
.\scripts\check.ps1
.\scripts\dev.ps1
```

`setup.ps1` installe les outils manquants puis télécharge :

- Everything SDK3 **3.0.0.9** dans `src-tauri\Everything3_x64.dll` ;
- Everything **1.5.0.1418b x64 portable** dans `src-tauri\engine\Everything.exe`.

Le SDK3 et le runtime sont vérifiés avec des empreintes SHA-256 épinglées dans le dépôt. Le runtime est également contrôlé par architecture PE x64 et par certificat Authenticode épinglé. Les binaires restent ignorés par Git et sont ajoutés au bundle Tauri pendant le build.

Le premier lancement de `dev.ps1` crée l’instance de développement `EverythingModernDev`. Il demande une élévation afin de copier le moteur sous `Program Files\Everything Modern Dev` et d’y installer le service associé. Les lancements suivants réutilisent ce service.

En développement, des binaires locaux peuvent être fournis explicitement :

```powershell
$env:EVERYTHING_ENGINE_EXE = "C:\chemin\Everything.exe"
$env:EVERYTHING_SDK3_DLL = "C:\chemin\Everything3_x64.dll"
$env:EVERYTHING_INSTANCE = "EverythingModernDev"
```

Sans override explicite, l’application installée utilise `EverythingModern` et `dev.ps1` utilise `EverythingModernDev`. Chaque override valide dispose de son propre répertoire de configuration et de base. Les noms d’instance acceptent uniquement 1 à 64 lettres ASCII, chiffres, points, tirets et underscores.

## Build installable

```powershell
.\scripts\build.ps1
```

L’installateur NSIS autonome est produit dans `target\release\bundle\nsis`. Le build échoue si le SDK3, le moteur ou les licences tierces ne sont pas présents.

## Benchmark du bridge Everything

Lancez Everything Modern une première fois, puis :

```powershell
.\scripts\benchmark.ps1 -Query "*.pdf" -Iterations 40
```

Le script mesure les latences p50, p95 et maximale d’une page de 256 résultats après warm-up.

## Architecture

```text
Everything Modern
├── src/                              # frontend Leptos CSR
│   ├── main.rs                       # démarrage et composition
│   ├── backend.rs                    # adaptateur IPC vers les commandes natives
│   ├── window.rs                     # contrôles de la fenêtre Tauri
│   └── app/
│       ├── mod.rs                    # composition et structure visuelle
│       ├── search.rs                 # générations, pagination et cache
│       ├── results.rs                # viewport virtuel et icônes progressives
│       ├── selection.rs              # sélection, focus et navigation
│       ├── file_operations.rs        # ouverture, renommage et Corbeille
│       ├── context_menu.rs           # état et positionnement du menu
│       ├── columns.rs                 # tri et redimensionnement des colonnes
│       ├── formatting.rs              # présentation des tailles, dates et totaux
│       └── icons.rs                   # primitives SVG
├── src-tauri/src/
│   ├── lib.rs                        # composition du runtime Tauri
│   ├── search.rs                     # moteur Everything et invalidation
│   ├── shell_commands.rs             # commandes Shell Windows
│   ├── trash.rs                      # snapshots immuables de suppression
│   └── desktop.rs                    # instance unique, tray et démarrage auto
├── crates/everything-core/           # modèle, façade moteur, SDK3 et moteur privé
└── crates/windows-shell/src/
    ├── lib.rs                        # interface publique stable
    ├── file_operations.rs            # fichiers, dossiers et Corbeille
    ├── icons.rs                      # extraction et cache des icônes
    └── clipboard.rs                  # presse-papiers natif
```

Les dépendances vont de l’interface vers les adaptateurs natifs, puis vers
`everything-core` et `windows-shell`. Les règles du moteur et le modèle de
sélection restent indépendants de Leptos et de Tauri.

## Raccourcis

| Raccourci | Action |
|---|---|
| `Ctrl+L` | Focus recherche |
| `↑` / `↓` | Navigation |
| `Page Up` / `Page Down` | Navigation par page |
| `Home` / `End` | Premier / dernier résultat |
| `Maj` + navigation | Étendre la sélection |
| `Ctrl+Espace` | Basculer la sélection courante |
| `Entrée` | Ouvrir |
| `F2` | Renommer |
| `Suppr` | Corbeille avec confirmation |
| `Maj+F10` | Menu contextuel |
| `Ctrl+A` | Sélectionner tous les résultats, sans les charger en mémoire |
| `Échap` | Fermer le menu ou désélectionner |

## Validation

La CI Windows télécharge et vérifie le SDK3 et le runtime avec des empreintes stockées dans le dépôt, exécute les tests et checks natifs/WASM, construit le frontend Trunk, compile l’installateur NSIS, puis vérifie silencieusement l’installation, la création du service et la désinstallation avant de publier l’artefact.

Avant publication, un test manuel doit encore vérifier sur une VM Windows propre : parcours UAC visible, première indexation et recherche, coexistence avec Everything classique, double lancement, redémarrage Windows et mise à jour.

## Licences tierces

Everything est redistribué selon sa licence MIT. Sa notice et celle de PCRE sont incluses dans `src-tauri/engine/THIRD-PARTY-LICENSES.txt` et dans l’installateur.

## Sécurité des opérations destructrices

La suppression utilise un snapshot immuable, préparé avant confirmation puis consommé une seule fois. Une confirmation ne peut donc pas être rejouée et les changements ultérieurs de l’index Everything ne modifient jamais les fichiers ciblés. Pour garder une mémoire bornée, une opération de Corbeille est limitée à 10 000 éléments.
