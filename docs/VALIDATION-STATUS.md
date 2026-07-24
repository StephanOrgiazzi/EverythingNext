# État de validation

## Vérifié dans l’archive

- structure des délimiteurs des sources Rust ;
- parsing de tous les manifests TOML et JSON ;
- syntaxe du module JavaScript embarqué ;
- présence des dépendances locales, icônes et ressources déclarées ;
- absence de marqueurs `TODO`, `FIXME`, `todo!`, `unimplemented!` et `panic!` ;
- tests unitaires ajoutés pour les plages de sélection, la normalisation SDK, les noms Windows, les collisions, le renommage de casse et les clés d’icônes.

## Validation Windows automatisée incluse

La workflow `.github/workflows/windows.yml` :

1. installe Rust, Trunk et Tauri CLI ;
2. génère `Cargo.lock` et applique `rustfmt` ;
3. télécharge et valide l’architecture du SDK Everything officiel ;
4. lance les tests et checks natifs/WASM ;
5. construit le frontend et un installateur NSIS ;
6. publie l’installateur et le lockfile comme artefacts.

## Gate de publication

Une version ne doit être qualifiée de finale qu’après réussite de la CI et du smoke test Windows décrit dans `TESTING.md`, avec Everything réellement lancé. La génération de cette archive n’a pas permis d’exécuter Rust, Trunk ou un installateur Windows dans l’environnement courant.
