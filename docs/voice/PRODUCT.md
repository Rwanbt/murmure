# Murmure Voice — cahier des charges

## Vision
Étendre le fork [Murmure](https://github.com/Rwanbt/murmure) avec **Pocket TTS** en français et un lecteur universel, tout en conservant le STT local de Murmure et son expérience Tauri/Rust/React. Le TTS figure comme module **Text to Speech** dans la catégorie **Extensions** de Murmure. Priorité Windows, prise en charge Linux et macOS après qualification.

## Objectifs produit
- STT local existant, compatible français, sans régression lors de l'activation du TTS.
- Pocket TTS exécuté principalement sur **CPU**, utilisable hors ligne après téléchargement des poids ; voix françaises sous licence vérifiée.
- Lecture par raccourci de la **sélection**, de la **zone au curseur** et du **contenu de la fenêtre au focus** quand l'API d'accessibilité le permet.
- Lire le contenu principal d'une page Web et les PDF textuels avec des adaptateurs explicites ; aucune prétention à une lecture universelle sans limite.
- Lire, mettre en pause, reprendre, arrêter immédiatement, naviguer phrase précédente/suivante.
- Réglages : moteur/modèle, voix, volume, vitesse, périphérique audio, segmentation, cache, raccourcis remappables.
- Petite capsule lecteur accessible ; retour clair en cas d'impossibilité de lire.
- API locale **authentifiée et versionnée** pour les intégrations externes futures.

## Hors périmètre — phase actuelle
**Pas de Jarvis**, mot de réveil, écoute continue, conversations temps réel full-duplex, barge-in vocal ou acceptation automatique de permissions. Aucun recours obligatoire à une API cloud ou un GPU. Aucun patch d'OpenCode officiel.

## Non-régression et qualité
Les fonctionnalités ajoutées exigent tests unitaires, intégration audio réelle, E2E Windows, conformité clavier/accessibilité et CI au SHA exact. Aucun résultat PASS présumé, aucune release tant que les gates sont incomplètes.

## Gouvernance
Branches de développement isolées et Draft PR courtes. Ne pas modifier l'upstream Kieirra/murmure. Ne pas publier de binaires signés sous l'identité de l'upstream. Assurer les obligations AGPL-3.0-or-later du fork et vérifier séparément les licences des poids/voix Pocket TTS.
