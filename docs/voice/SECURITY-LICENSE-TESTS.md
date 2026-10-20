# Qualification, sécurité et licences

## Tests requis
- **Rust** : segmentation, états, corruption modèle, queue et annulation, cache.
- **Audio réel CPU Windows** : français accentué, pause/reprise/stop durant chargement/génération/lecture, sortie WAV et périphérique débranché.
- **E2E Tauri** : réglages TTS dans Extensions, STT inchangé, raccourcis globaux et collisions, multi-écrans/focus, UI accessible.
- **Lecteurs** : champ mot de passe exclu, sélection préservée, page Web à permission limitée, PDF à contenu textuel.
- **API** : clients autorisés vs anonymes, CORS/Origin/Host, tailles/timeouts, arrêt immédiat et reconnexion.
- **Offline** : réseau coupé après installation des poids ; aucune transmission audio/texte aux services externes.
- **Performance** : mesurer latence CPU/RAM/temps de chargement sur machines réelles sans inventer de seuil déjà validé.
- **CI** : SHA exact de chaque Draft PR, interdiction de contourner les contrôles.

## Confidentialité
Traitement local par défaut. Pas d'enregistrement persistant des transcriptions/audio sans opt-in. Les logs/rapports crash ne contiennent ni texte lu, ni audio, ni tokens. Pas d'accès réseau autre que loopback pour l'API. Protéger les secrets d'authentification dans le coffre système.

## Licences
Murmure : AGPL-3.0-or-later. Pocket TTS : distinguer code, poids et voix ; contrôler les licences et exigences de distribution par ressource. Préserver copyrights et notices ; ne pas expédier des modèles dont les conditions n'ont pas été vérifiées. ID applicatif, signature et updater du fork distincts de l'upstream.
