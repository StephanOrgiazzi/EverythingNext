# Corrections après review-factory

- La suppression est préparée en snapshot immuable avant confirmation.
- Chaque snapshot est consommable une seule fois côté Rust.
- Les boutons sont désactivés pendant l’exécution et les doubles soumissions sont rejetées.
- La préparation vérifie l’annulation entre chaque page et limite une opération à 10 000 éléments.
- Les erreurs de recherche et les erreurs d’action utilisent des états distincts.
- Rust, Trunk et Tauri CLI sont épinglés dans la toolchain et les scripts.

## Validation restante

`Cargo.lock` doit être généré puis versionné depuis un environnement disposant de Cargo. La CI échoue explicitement s’il est absent afin d’éviter un build non reproductible silencieux.
