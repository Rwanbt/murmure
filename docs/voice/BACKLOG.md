# Backlog Murmure Voice — 22 user stories

> **État :** [22 issues GitHub publiées](https://github.com/Rwanbt/murmure/issues), MV-001…MV-022 (issues #4 à #25), avec priorités et milestones. Les cases ne sont pas réputées validées sans preuves au SHA.

**Périmètre :** STT existant + Pocket TTS français principalement CPU. Raccourcis, lecture sélection/document/article, extension TTS, API locale sécurisée. **Pas de Jarvis, wake word, écoute continue ni full-duplex.**

## Ordre de développement

- **M0 :** MV-001, MV-002, MV-003, MV-010, MV-020
- **M1 :** MV-004, MV-005, MV-006, MV-007, MV-009
- **M2 :** MV-008, MV-011, MV-012, MV-013, MV-014, MV-015, MV-018, MV-021
- **M3 :** MV-016, MV-017
- **M5 :** MV-019, MV-022

## [MV-001](https://github.com/Rwanbt/murmure/issues/4) — Fork natif et baseline Murmure 2.0.3

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux conserver la filiation Kieirra/murmure et qualifier le STT existant pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Origine et commit de base tracés
- [ ] Baseline Windows/Tauri/Rust réelle au SHA final
- [ ] Aucun changement upstream
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-002](https://github.com/Rwanbt/murmure/issues/5) — CI Tauri/Rust/React multiplateforme

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux rendre les tests et builds reproductibles pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Format/lint/build/tests requis
- [ ] Windows prioritaire, Linux/macOS qualifiés
- [ ] Pas de bypass CI
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-003](https://github.com/Rwanbt/murmure/issues/6) — Audit licences AGPL, Pocket TTS, modèles et voix

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux qualifier les obligations avant distribution pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Notices Murmure préservées
- [ ] Licences du code, poids et voix distinctes
- [ ] Politique de téléchargement/licences documentée
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-004](https://github.com/Rwanbt/murmure/issues/7) — Prototype Pocket TTS CPU Rust français

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux produire une vraie voix française sans GPU/cloud pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Backend Rust/sidecar évalué et contrôlé
- [ ] Prononciation accentuée testée sur CPU Windows
- [ ] Test hors ligne et mesures latence/RAM/CPU
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-005](https://github.com/Rwanbt/murmure/issues/8) — Téléchargement, intégrité et cache des modèles

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux maîtriser modèles, stockage et sécurité pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Téléchargement volontaire avec progression/annulation
- [ ] Provenance/hashes des poids vérifiés
- [ ] Cache nettoyable et fonctionnement offline
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-006](https://github.com/Rwanbt/murmure/issues/9) — Moteur audio pause, reprise et arrêt déterministe

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux contrôler à tout moment la sortie vocale pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] STOP vide queue et annule génération
- [ ] Aucun segment fantôme après annulation
- [ ] Pause/reprise et périphérique indisponible couverts
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-007](https://github.com/Rwanbt/murmure/issues/10) — Extension Text to Speech dans les paramètres Murmure

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux configurer le TTS depuis la rubrique Extensions pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Interrupteur actif, choix de modèle/voix
- [ ] Réglages vitesse, volume, audio, bouton prévisualisation
- [ ] États erreurs et persistance, accessibilité clavier
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-008](https://github.com/Rwanbt/murmure/issues/11) — Lire la sélection via raccourci global

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux écouter la sélection de l'application active pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Sélection récupérée via accessibilité quand possible
- [ ] Presse-papiers et sélection non altérés
- [ ] Champs sensibles exclus, erreurs explicites
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-009](https://github.com/Rwanbt/murmure/issues/12) — Raccourcis lecture, pause/reprise et arrêt

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux piloter le lecteur sans changer de fenêtre pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Tous les raccourcis remappables et désactivables
- [ ] Détection collisions avec OS et raccourcis STT
- [ ] Tests de raccourcis globaux Windows
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-010](https://github.com/Rwanbt/murmure/issues/13) — Tests de non-régression du STT Murmure

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux préserver la dictée française existante pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Dictée OK avec TTS activé et désactivé
- [ ] Pas de capture micro déclenchée par TTS
- [ ] Tests STT réels et mesures ressources
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-011](https://github.com/Rwanbt/murmure/issues/14) — Segmentation et navigation phrase par phrase

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux lire de manière naturelle et naviguer facilement pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Phrases et ponctuation françaises traitées
- [ ] Blocs code/logs/URL exclus par défaut
- [ ] Navigation sans replay des segments annulés
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-012](https://github.com/Rwanbt/murmure/issues/15) — Lire depuis le curseur et la fenêtre au focus

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux écouter le texte en cours de lecture dans une application pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] UI Automation Windows ou adaptateurs natifs
- [ ] Affichage de la portée/source avant lecture
- [ ] Refus explicite des contenus inaccessibles/sensibles
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-013](https://github.com/Rwanbt/murmure/issues/16) — Lecteur de pages Web avec permissions limitées

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux écouter un article sans menus ni navigation pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Accès activeTab / action utilisateur, pas de droits globaux
- [ ] Extraction article principal + aperçu si ambigu
- [ ] Scénarios E2E Chromium avec pages sensibles
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-014](https://github.com/Rwanbt/murmure/issues/17) — Lecture des PDF textuels

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux écouter un document PDF pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Extraction du texte intégré, sans OCR implicite
- [ ] Pages, ordre de lecture et erreurs gérés
- [ ] Tests PDF multicolonne et document simple
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-015](https://github.com/Rwanbt/murmure/issues/18) — Overlay lecteur accessible

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux contrôler la lecture dans une petite capsule pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Lecture, pause, stop, phrase suivante/précédente
- [ ] Aucune capture intempestive du focus
- [ ] Windows DPI/multi-écran et navigation clavier
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-016](https://github.com/Rwanbt/murmure/issues/19) — API vocale locale versionnée

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux permettre à un client externe de piloter TTS/STT pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Contrats status/speak/pause/resume/stop et events
- [ ] Annulation, identités requêtes, backpressure
- [ ] Loopback uniquement par défaut
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-017](https://github.com/Rwanbt/murmure/issues/20) — Authentification et protection de l'API locale

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux empêcher les sites et processus non autorisés de piloter la voix pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Jeton aléatoire protégé par coffre OS
- [ ] Origin/Host/CORS, débit et taille bornés
- [ ] Tests attaques localhost et absence de fuite logs
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-018](https://github.com/Rwanbt/murmure/issues/21) — Réglages avancés TTS et gestion des profils

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux adapter le moteur à chaque usage pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Vitesse sans variation excessive de hauteur
- [ ] Sélection périphérique et profils utilisateur
- [ ] Nettoyage du cache et migration des réglages
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-019](https://github.com/Rwanbt/murmure/issues/22) — Releases signées et updater distinct de l'upstream

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux distribuer notre fork de manière identifiable pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Identifiant applicatif distinct de Murmure officiel
- [ ] Mises à jour et signatures dédiées
- [ ] SBOM/notes et validation CI multi-OS
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-020](https://github.com/Rwanbt/murmure/issues/23) — Synchronisation upstream contrôlée

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux intégrer les correctifs de Murmure sans casser STT/TTS pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Suivi releases upstream avec comparatif
- [ ] PR d'intégration après revue et CI
- [ ] Aucune fusion upstream automatique
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-021](https://github.com/Rwanbt/murmure/issues/24) — Confidentialité des sources de texte et audio

**Priorité : P0** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux prévenir lecture/stockage accidentels d'informations privées pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Geste explicite pour sélection/document/article
- [ ] Champs password/protégés refusés
- [ ] Aucune télémétrie ni persistance audio/texte sans opt-in
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## [MV-022](https://github.com/Rwanbt/murmure/issues/25) — Qualification UX et performances CPU avant release

**Priorité : P1** · **Statut : à faire**

**User story :** En tant qu'utilisateur ou mainteneur de Murmure Voice, je veux disposer d'une application fiable hors ligne pour obtenir une application locale fiable et maintenable.

**Critères d'acceptation :**
- [ ] Tests Windows réels après packaging et reprise veille
- [ ] A11y clavier, multi-écran et non-régressions
- [ ] Benchmarks CPU/RAM/latence et gates verts au SHA final
- [ ] Tests déterministes et validation au SHA final ; aucun contournement

## Règles de suivi

Chaque user story est une unité de suivi ; conserver son identifiant lors de la création future de l'issue. Les bugs découverts durant les tests doivent être documentés avec reproduction RED → correctif → GREEN. Draft PR courtes et isolées ; pas de modifications dans upstream ni de merge sans gates. Les dépendances OC-002/OC-005 du plugin OpenCode sont bloquées tant que MV-016 et MV-017 ne sont pas validées.
