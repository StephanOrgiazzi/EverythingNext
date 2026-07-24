# Architecture

## Flux d’une recherche

1. Toute modification du texte, du tri ou le rafraîchissement incrémente une génération.
2. L’UI notifie immédiatement le backend de la nouvelle génération, puis applique un debounce de 55 ms.
3. Le frontend demande les pages visibles par lots de 256 résultats.
4. Le backend rejette les requêtes obsolètes avant et après l’attente du verrou SDK.
5. `everything-core` configure recherche, offset, limite, tri et request flags, puis appelle `Everything_QueryW(TRUE)`.
6. Une page complète est sérialisée en une seule réponse IPC.
7. Les réponses d’une génération obsolète sont ignorées sans modifier l’état de chargement courant.
8. La liste rend uniquement la fenêtre visible, avec huit lignes d’overscan avant et après.
9. Une mesure `requestAnimationFrame` enregistre le délai entre réception IPC et prochain frame UI.

L’API Everything est globale et synchrone : une requête déjà entrée dans `Everything_QueryW` ne peut pas être interrompue sans risquer de corrompre l’état partagé. L’annulation est donc logique et coalescente : les travaux en attente sont abandonnés, l’éventuel appel déjà actif termine, puis la génération récente passe en priorité.

## Ressources du bundle

Tauri résout `Everything64.dll` dans `$RESOURCE` au démarrage et fournit ce chemin à `EverythingEngine::from_dll_path`. En développement, la crate conserve des fallbacks vers la variable `EVERYTHING_SDK_DLL`, le dossier de l’exécutable et `src-tauri`.

## Mémoire et concurrence

- huit pages frontend au maximum, soit environ 2048 résultats hors sélection ;
- cache LRU d’icônes borné à 512 clés, mutualisé par extension pour les fichiers génériques et exact pour les types à icône personnalisable ;
- quatre extractions Shell simultanées au maximum ;
- résultats et icônes transmis par lots ou URI complètes, jamais cellule par cellule ;
- sélection stockée comme plages d’indices fusionnées : `Ctrl+A` reste constant en mémoire ;
- les chemins d’une opération destructive sont résolus côté backend juste avant l’action, sans gonfler l’état frontend.

## Séparation des responsabilités

- `everything-core` : modèles partagés, sélection logique et bridge Everything, sans dépendance Tauri ;
- `windows-shell` : intégration Shell, presse-papiers natif et opérations fichiers sécurisées ;
- `src-tauri` : ressources, concurrence, annulation logique et commandes IPC ;
- `src` : état d’interface, virtualisation et interactions.

## Validation

- tests unitaires des modèles, conversions SDK et noms Windows ;
- CI Windows native et WASM ;
- benchmark reproductible du bridge dans `scripts/benchmark.ps1` ;
- campagne manuelle requise pour la fluidité, le bundle installé et la parité sur un corpus réel.
