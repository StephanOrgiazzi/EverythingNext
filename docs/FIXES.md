# Corrections apportées après audit

- résolution de `Everything64.dll` depuis les ressources du bundle Tauri ;
- sélection remplacée par des plages d’indices fusionnées, indépendantes du cache de pages ;
- `Ctrl+A`, `Maj+End` et les grandes sélections couvrent réellement tous les résultats avec une mémoire constante ;
- résolution des chemins sélectionnés côté moteur uniquement au moment de l’opération ;
- cache d’icônes hybride : mutualisé pour les extensions génériques, exact pour `.lnk`, `.url`, `.ico`, exécutables et dossiers personnalisés ;
- scripts de setup/build capables de générer `Cargo.lock`, formater les sources et installer automatiquement le SDK ;
- barre de titre native désactivée et contrôles de fenêtre personnalisés autorisés ;
- raccourcis de la liste isolés des champs de saisie et boutons ;
- correction de la première sélection avec `Flèche bas` ;
- navigation ajoutée : Home, End, Page Up/Down, Maj+navigation, Ctrl+Espace et Maj+F10 ;
- navigation `Ctrl+flèches` sans écraser la sélection et menu clavier aligné sur la ligne focalisée ;
- recalcul de la hauteur virtualisée lors du redimensionnement de fenêtre ;
- métrique du prochain frame UI via `requestAnimationFrame` ;
- invalidation des générations obsolètes côté frontend et backend ;
- concurrence des icônes Shell limitée et cache borné ;
- copie via le presse-papiers Windows natif ;
- renommage sécurisé : noms réservés/interdits, collisions, changements de casse et rollback en cas d’échec ;
- suppression vers la Corbeille avec confirmation, déduplication, ordre enfants→parents et rapport d’échecs partiels ;
- dialogues intégrés à la place de `prompt()`/`confirm()` ;
- menu contextuel maintenu dans les limites de la fenêtre ;
- rafraîchissement sans mutation artificielle de la requête ;
- tests unitaires, benchmark du bridge, scripts stricts et CI Windows avec build NSIS.

## Validation encore nécessaire avant publication

Le dépôt contient les moyens de validation, mais un release doit être compilé, installé et testé sur Windows 11 avec une instance Everything réelle. Les critères de performance et la parité sur un corpus de recherches ne peuvent pas être certifiés par inspection statique.
