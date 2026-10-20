# Roadmap et jalons

## M0 — Fondation
Vérifier filiation upstream, baseline Murmure 2.0.3, builds/tests sur Windows/Linux/macOS ; politique de sync upstream ; revue licences du code, modèles et voix. **Gate : baseline reproductible au SHA.**

## M1 — TTS français CPU
Runtime Pocket TTS Rust, poids vérifiés et gestion du téléchargement, lecteur avec pause/reprise/stop sans audio résiduel ; page Extensions et réglages persistants. **Gate : voix française produite réellement hors ligne et stop déterministe.**

## M2 — Lecture universelle contrôlée
Raccourcis globaux, lecture sélection, zone depuis curseur, fenêtre focus, navigation par phrase, lecteur Web/PDF avec permission explicite ; overlay lecteur accessible. **Gate : scénarios Windows réels et échecs sûrs.**

## M3 — Interface locale sécurisée
API versionnée, authentification, permissions, événements, annulation et tests négatifs. **Gate : client externe autorisé fonctionnel, clients arbitraires refusés.**

## M4 — Intégration OpenCode V2
Développement dans le dépôt séparé [opencode-murmure](https://github.com/Rwanbt/opencode-murmure) : STT des prompts avec aperçu, TTS des réponses, overlay hover externe, sans patch d'OpenCode officiel. **Gate : tests TUI/Desktop exact SHA.**

## M5 — Qualification et diffusion
Tests Windows/Linux/macOS, a11y, packaging/auto-updater distinct, sécurité et licences, compatibilité upgrades upstream, provenance et notes de release. **Gate : tous les contrôles requis verts.**

Les issues MV-001…MV-022 et OC-001…OC-012 décrivent le travail et les critères d'acceptation ; toute fonctionnalité finale nécessite preuve RED→GREEN et revue. **Mode Jarvis explicitement exclu.**
