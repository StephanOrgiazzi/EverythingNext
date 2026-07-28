# Everything Next

Client Windows rapide pour **Everything 1.5**, construit en Rust avec **Tauri 2 + Leptos**. L’installateur embarque Everything **1.5.0.1418b x64** et **Everything SDK3 3.0.0.9** : aucune installation préalable d’Everything n’est nécessaire.

## Fonctionnalités

- recherche Everything avec syntaxe native et debounce de 55 ms ;
- connexion SDK3 explicite par named pipe ;
- pagination par viewport, cache glissant et liste virtualisée ;
- tri par nom, chemin, type, taille et date ;
- vues liste et icônes avec visuels Windows Shell progressifs ;
- sélection simple, Ctrl, Maj et `Ctrl+A` sans charger tous les résultats ;
- ouvrir, révéler, copier, renommer et déplacer vers la Corbeille ;
- thèmes clair et sombre, barre de titre personnalisée et état de fenêtre restauré ;
- intégration avec EverythingPowerToys et prise en charge de `-s "requête"`.

## Installation

Téléchargez puis exécutez l’installateur NSIS. L’installation est effectuée pour la machine dans `Program Files` et produit l’exécutable :

```text
EverythingNext.exe
```

Le moteur embarqué :

- fonctionne sans installation séparée d’Everything ;
- n’affiche ni fenêtre ni icône de notification ;
- stocke sa configuration et sa base dans `%LOCALAPPDATA%\EverythingNext\Engine` ;
- utilise le service privé `Everything Service (EverythingNext)` ;
- expose l’instance IPC Everything par défaut pour rester compatible avec les clients SDK2.

L’instance Everything par défaut est exclusive. Si Everything classique est déjà lancé sur cette instance, Everything Next demande de le fermer au lieu de se connecter à sa base.

## PowerToys Run

Everything Next est compatible avec le plugin [EverythingPowerToys](https://github.com/lin-ycv/EverythingPowerToys).

Dans **PowerToys Settings → PowerToys Run → Everything**, définissez **Everything path** sur l’exécutable installé `EverythingNext.exe`.

Le plugin utilise alors :

- l’IPC Everything par défaut pour afficher ses résultats ;
- `EverythingNext.exe -s "requête"` pour **Show more results**.

Si Everything Next est déjà lancé, sa fenêtre est restaurée, la requête est transférée et le champ de recherche reçoit le focus.

## Prérequis de développement

- Windows 11 x64 ;
- Rust stable **MSVC** avec la cible `wasm32-unknown-unknown` ;
- Visual Studio 2022 Build Tools avec le Windows SDK ;
- WebView2, inclus par défaut dans Windows 11.

Everything 1.4 et les anciens SDK ne sont pas pris en charge par l’application native.

## Installation rapide

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup.ps1
.\scripts\check.ps1
.\scripts\dev.ps1
```

`setup.ps1` télécharge et vérifie :

- Everything SDK3 **3.0.0.9** dans `src-tauri\Everything3_x64.dll` ;
- Everything **1.5.0.1418b x64 portable** dans `src-tauri\engine\Everything.exe`.

Le premier lancement de `dev.ps1` crée l’instance de développement `EverythingNextDev` et installe son service sous `Program Files\Everything Next Dev`.

Des binaires ou une instance locale peuvent être fournis explicitement :

```powershell
$env:EVERYTHING_ENGINE_EXE = "C:\chemin\Everything.exe"
$env:EVERYTHING_SDK3_DLL = "C:\chemin\Everything3_x64.dll"
$env:EVERYTHING_INSTANCE = "EverythingNextDev"
```

## Build installable

```powershell
.\scripts\build.ps1
```

Ce build local privilégie la vitesse de compilation. Pour créer un build de production
entièrement validé avec Thin LTO :

```powershell
.\scripts\build.ps1 -Production
```

Dans les deux cas, l’installateur NSIS est produit dans `target\release\bundle\nsis`.

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
| `Ctrl+A` | Sélectionner tous les résultats |
| `Échap` | Fermer le menu ou désélectionner |

## Licences tierces

Everything est redistribué selon sa licence MIT. Sa notice et celle de PCRE sont incluses dans `src-tauri/engine/THIRD-PARTY-LICENSES.txt` et dans l’installateur.

## Sécurité des opérations destructrices

La suppression utilise un snapshot immuable préparé avant confirmation et consommé une seule fois. Une opération de Corbeille est limitée à 10 000 éléments afin de conserver une mémoire bornée.
