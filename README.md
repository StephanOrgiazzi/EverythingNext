# Everything Modern

Client Windows moderne pour **Everything 1.5**, construit en Rust avec **Tauri 2 + Leptos**. Il interroge directement l’index existant via **Everything SDK3 / IPC3** et ne réindexe aucun fichier.

## Fonctionnalités du MVP

- recherche Everything avec syntaxe native et debounce de 55 ms ;
- connexion SDK3 explicite par named pipe, avec client, état de recherche et listes de résultats dédiés ;
- invalidation logique des anciennes générations côté frontend **et backend Rust** ;
- pagination SDK3 par viewport, par lots de 256 résultats, et cache glissant de huit pages ;
- liste virtualisée, tri nom/chemin/taille/date et icônes Shell progressives avec cache hybride par extension ou chemin sensible ;
- sélection logique par plages : simple, Ctrl, Maj, grandes plages et `Ctrl+A` sans charger tous les résultats ;
- ouvrir, révéler, copier via le presse-papiers natif, renommer et déplacer vers la Corbeille ;
- dialogues applicatifs de renommage et de confirmation ;
- thème Windows clair/sombre et barre de titre personnalisée ;
- restauration automatique de la taille et de la position de fenêtre ;
- chargement explicite de `Everything3_x64.dll` depuis les ressources de l’application installée ;
- métrique visible `UI x ms` mesurant réponse IPC → prochaine passe de rendu.

## Prérequis

- Windows 11 x64 ;
- **Everything 1.5 x64** lancé en arrière-plan ;
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

Le script `setup.ps1` installe la cible WASM et les outils manquants, génère `Cargo.lock`, applique `rustfmt`, puis télécharge **Everything SDK3 3.0.0.9** depuis voidtools. La DLL x64 est copiée dans `src-tauri` et incluse comme ressource Tauri dans les bundles installables.

## Build installable

```powershell
.\scripts\build.ps1
```

L’installateur NSIS est produit dans `target\release\bundle\nsis`.

## Benchmark du bridge Everything

Everything 1.5 doit être lancé :

```powershell
.\scripts\benchmark.ps1 -Query "*.pdf" -Iterations 40
```

Le script mesure les latences p50, p95 et maximale d’une page de 256 résultats après warm-up.

## Architecture

```text
Everything Modern
├── src/                       # UI Leptos CSR
├── src-tauri/                 # orchestration Tauri et commandes IPC
├── crates/everything-core/    # bridge Everything SDK3 indépendant de Tauri
└── crates/windows-shell/      # icônes et opérations fichiers Windows
```

Le bridge se connecte par défaut à l’instance principale d’Everything 1.5. Pour cibler une instance nommée ou charger une autre copie de la DLL SDK3 :

```powershell
$env:EVERYTHING_INSTANCE = "travail"
$env:EVERYTHING_SDK3_DLL = "C:\chemin\Everything3_x64.dll"
```

Une valeur vide de `EVERYTHING_INSTANCE` cible l’instance principale.

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

## Corrections après audit

Le détail des corrections est disponible dans [`docs/FIXES.md`](docs/FIXES.md). L’état exact des validations est consigné dans [`docs/VALIDATION-STATUS.md`](docs/VALIDATION-STATUS.md).

## Validation

La CI Windows génère le lockfile, normalise le formatage, installe la DLL SDK3, exécute les tests et checks natifs/WASM, construit le frontend Trunk puis produit un installateur NSIS. Elle publie aussi `Cargo.lock` et l’installateur comme artefacts. Un build final doit être exécuté et testé sur Windows avec Everything 1.5 lancé avant publication.

## Sécurité des opérations destructrices

La suppression utilise un snapshot immuable, préparé avant confirmation puis consommé une seule fois. Une confirmation ne peut donc pas être rejouée et les changements ultérieurs de l’index Everything ne modifient jamais les fichiers ciblés. Pour garder une mémoire bornée, une opération de Corbeille est limitée à 10 000 éléments.
