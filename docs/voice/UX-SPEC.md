# UX — Extension Text to Speech

## Page Extensions > Text to Speech
Interrupteur actif/inactif, statut poids/model (non installé, téléchargement, prêt, erreur), voix française avec preview, volume, vitesse, périphérique audio, segmentation de phrase, taille du cache, filtres de lecture code/log/URLs, nettoyage des modèles. Persistance des réglages. UX clavier, lecteur d'écran, contrastes, agrandissement.

## Raccourcis proposés (non figés)
- Lire sélection : Ctrl+Alt+L
- Lire fenêtre/document actif : Ctrl+Alt+P
- Pause/reprise : Ctrl+Alt+Espace
- Arrêter : Ctrl+Alt+K
- Phrase précédente/suivante : touches définies par l'utilisateur

Contrôle de conflit OS/app ; chaque raccourci configurable ou désactivable. Les raccourcis ci-dessus ne sont **pas implémentés**.

## Lecture
Prévisualiser la source ("sélection", "document", "article"). Si la source est sensible, inaccessible ou ambiguë, refuser la lecture et proposer une action explicite de l'utilisateur. Ne jamais vider la sélection ou le presse-papiers à son insu. Une lecture nouvelle remplace l'ancienne suivant réglage explicite. Stop fonctionne même pendant le chargement du modèle.

## Overlay
Capsule facultative avec lecture, pause, stop, navigation phrase, origine du contenu et moteur sélectionné. Ne doit ni voler le focus ni bloquer les clics hors de sa surface. Windows multi-écrans/DPI d'abord ; comportement prévisible en cas d'application maximisée.
