# Plan de validation Windows

## Automatique

```powershell
.\scripts\setup.ps1
.\scripts\check.ps1
.\scripts\benchmark.ps1 -Query "*.pdf" -Iterations 40
.\scripts\build.ps1
```

## Smoke test fonctionnel

1. Démarrer Everything et vérifier l’état « connecté ».
2. Comparer au moins 20 requêtes avec Everything natif : texte simple, `ext:`, regex, taille, dates, chemins, exclusions et tris.
3. Tester souris, Ctrl, Maj, Ctrl+Maj, flèches, Page Up/Down, Home/End, Ctrl+A, Ctrl+Espace, F2, Suppr et Maj+F10.
4. Sur une recherche de plus de 100 000 résultats, tester `Ctrl+A`, `Maj+End` et une plage traversant plusieurs pages ; le compteur doit être exact sans chargement massif.
5. Tester des chemins contenant espaces, accents, virgules et noms longs.
6. Renommer vers un nom interdit, un nom existant et un fichier supprimé entre-temps.
7. Installer le NSIS sur une machine propre et vérifier que la DLL est trouvée sans variable d’environnement.
8. Redimensionner et déplacer la fenêtre, fermer puis relancer.

## Performance

- mesurer p50/p95 du bridge avec le benchmark ;
- profiler une navigation continue sur plusieurs centaines de milliers de résultats ;
- confirmer que le nombre de nœuds DOM reste proportionnel à la hauteur de la fenêtre ;
- vérifier que les noms apparaissent avant les icônes ;
- contrôler la métrique `UI x ms` affichée dans la barre d’état, cible <100 ms ;
- confirmer au besoin la mesure avec les DevTools sur une build release.
