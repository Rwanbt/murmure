/**
 * French messages for the errors the Rust backend can return.
 *
 * The backend reports technical errors in English. The interface shows a
 * localized sentence when it knows the error, and falls back to the raw
 * message otherwise, so an unknown error still carries its information.
 *
 * The keys must match the messages returned by `src-tauri/src/tts`.
 */
const TTS_ERROR_MESSAGES: Record<string, string> = {
    'Enable Pocket TTS model downloads explicitly in Extensions before reading.':
        "Autorisez le téléchargement du modèle Pocket TTS dans Extensions avant de lire.",
    'Pocket TTS generation failed': "La génération vocale Pocket TTS a échoué.",
    'Pocket TTS produced no WAV': "Pocket TTS n'a produit aucun fichier audio.",
    'Pocket TTS WAV could not be decoded': "Le fichier audio produit par Pocket TTS est illisible.",
    'Unable to send text to Pocket TTS': "Impossible d'envoyer le texte à Pocket TTS.",
    'Pocket TTS preview timed out; generation was stopped':
        "La génération Pocket TTS a dépassé la durée maximale et a été arrêtée.",
    'Unable to monitor Pocket TTS': "Impossible de suivre le processus Pocket TTS.",
    'Unable to start Pocket TTS': "Impossible de démarrer Pocket TTS.",
    'This Pocket TTS binary requires an AVX2 CPU':
        "Ce moteur Pocket TTS exige un processeur compatible AVX2.",
    'Pocket TTS executable does not match the verified Kyutai release':
        "Le moteur Pocket TTS installé ne correspond pas à la version Kyutai vérifiée.",
    'Pocket TTS is already active': "Une lecture Pocket TTS est déjà en cours.",
    'Pocket TTS preview worker is unavailable': "Le lecteur Pocket TTS n'est plus disponible.",
    'No usable audio output device': "Aucun périphérique de sortie audio n'est disponible.",
    'Unable to open output device': "Impossible d'ouvrir le périphérique de sortie audio.",
    'Stop Pocket TTS playback before clearing its model cache':
        "Arrêtez la lecture Pocket TTS avant de supprimer le cache des modèles.",
    'Wait for the Pocket TTS installation to finish before clearing its model cache':
        "Attendez la fin de l'installation du moteur Pocket TTS avant de supprimer le cache des modèles.",
    'Unable to clear the Pocket TTS model cache': "Impossible de supprimer le cache des modèles Pocket TTS.",
    'Pocket TTS model cache is not a regular directory':
        "Le cache des modèles Pocket TTS n'est pas un dossier ordinaire.",
    'Text exceeds the Pocket TTS length limit': "Le texte dépasse la longueur maximale acceptée par Pocket TTS.",
    'No readable text was provided': "Aucun texte lisible n'a été fourni.",
};

/**
 * Return the French sentence for a backend error, or the raw message when
 * the error is not known yet.
 */
export const translateTtsError = (message: string): string => {
    const known = TTS_ERROR_MESSAGES[message];
    return known ?? message;
};
