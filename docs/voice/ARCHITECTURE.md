# Architecture STT/TTS

## Vue d'ensemble
```text
Murmure Tauri/React
  ├─ STT existant (Parakeet / moteur de dictée Murmure)
  └─ Extensions > Text to Speech
       ├─ tts-core : Pocket TTS CPU, gestion et vérification des modèles
       ├─ player : queue bornée, pause/reprise/stop, périphériques audio
       ├─ text-sources : sélection, curseur, fenêtre focus (accessibilité OS)
       ├─ readers : article Web, PDF textuel, filtrage/segmentation
       ├─ shortcuts : raccourcis globaux configurables et gestion conflits
       └─ voice-api : liaison loopback locale auth + events, versionnée
```

## Contrat moteur
Machine à états : IDLE → LOADING → SYNTHESIZING → PLAYING ⇄ PAUSED → IDLE / STOPPED / ERROR. Un STOP invalide la génération en cours et vide les files, y compris après reconnexion. IDs par lecture et segmentation à la phrase ; backpressure, quotas mémoire et cache configurables. Mesurer le comportement sur CPU Windows réel.

## Sources de texte
**Windows :** UI Automation TextPattern/SelectionPattern lorsque disponible ; ne jamais simuler Ctrl+A/C en aveugle ni remplacer silencieusement le presse-papiers. Mac et Linux : interfaces d'accessibilité natives adaptées. **Web :** extraction d'article par permission activeTab/interaction explicite, pas de permission globale permanente. **PDF :** texte natif uniquement d'abord, scans/OCR hors périmètre V1. Ne pas afficher ni lire champs mots de passe et interfaces protégées.

## API locale future
Processus/service unique, loopback seulement, authentification avec secret protégé par coffre OS, vérification d'origine/Host, tailles, délais, débit. Méthodes envisagées : `status`, `speak`, `pause`, `resume`, `stop`, `voices`, `transcribe`; événements de progression et versions explicites. **Aucun texte, secret ou audio dans les logs par défaut.**

## Investigations avant code
- Intégration Pocket TTS Rust comme bibliothèque vs sidecar et implications de packaging.
- Formats, licence et installation des poids/voix françaises.
- Qualité, CPU/RAM et latence du modèle français léger vs 24L.
- Conflits entre audio sortant TTS et raccourcis STT Murmure.
- Différences UI Automation / Wayland / macOS et comportements de focus.
