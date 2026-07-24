# PRD — Everything Modern

## Objectif

Créer un client Windows moderne pour Everything, conservant sa vitesse et sa syntaxe de recherche, avec une interface proche de l’Explorateur Windows 11. L’application utilise le moteur Everything existant via son SDK/IPC et ne réindexe pas les fichiers.

## Utilisateur cible

Utilisateur Windows qui recherche fréquemment des fichiers, apprécie les performances d’Everything, trouve son interface native datée et attend les interactions habituelles de l’Explorateur.

## Proposition de valeur

La vitesse d’Everything dans une interface moderne, cohérente avec Windows 11.

## MVP

Recherche instantanée, syntaxe Everything, annulation logique, résultats virtualisés, tri, icônes Shell progressives, sélection multiple, opérations courantes, thème système, navigation clavier et restauration de fenêtre.

## Hors périmètre initial

Réindexation, parité complète, preview handlers, onglets, bookmarks, filtres avancés, opérations complexes et autres OS.

## Contraintes

Première réponse visible sous 100 ms après réception des données, aucune cellule envoyée isolément, rendu limité aux lignes visibles, icônes non bloquantes et mémoire bornée pendant le parcours de grands ensembles.
