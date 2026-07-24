# Conformité au PRD

| Exigence | Implémentation | Validation |
|---|---|---|
| SDK/IPC Everything, sans réindexation | `everything-core` | benchmark et test manuel Windows |
| Syntaxe Everything | chaîne transmise sans transformation | corpus manuel à comparer avec Everything |
| Annulation logique | génération frontend + `AtomicU32` backend | inspection et CI |
| Liste virtualisée | fenêtre visible + overscan | aperçu navigateur et test manuel |
| Lots/pagination | pages de 256 | tests de navigation |
| Colonnes et tri | nom, chemin, taille, modification | test manuel |
| Icônes progressives | cache LRU 512 hybride : extension générique, chemin exact pour dossiers/raccourcis/exécutables, concurrence limitée à 4 | test de charge manuel |
| Sélection multiple | plages logiques fusionnées, `Ctrl+A` global, souris, Ctrl, Maj et clavier | tests unitaires + test manuel |
| Actions fichiers | ouvrir, révéler, presse-papiers natif, renommer, Corbeille avec rapport partiel | tests unitaires + smoke test Windows |
| Interface système | thème clair/sombre, titlebar et layout Windows 11 | revue visuelle |
| Navigation clavier | flèches, pages, Home/End, sélection, contexte | test manuel |
| État de fenêtre | plugin window-state | test de redémarrage |
| DLL dans l’installateur | ressource Tauri résolue via `$RESOURCE` | installation NSIS |
| Mémoire bornée | huit pages UI, cache d’icônes 512 et sélection en plages constantes | tests unitaires + profilage long recommandé |
| Première réponse visible <100 ms après réception | mise à jour de page unique, icônes asynchrones et métrique `UI x ms` intégrée | mesure automatisée dans l’app + validation Windows |

Aucune publication ne doit être qualifiée de « finie » avant réussite du build Windows, installation du bundle et campagne de tests décrite dans `docs/TESTING.md`.
