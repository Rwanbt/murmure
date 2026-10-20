import { useEffect, useState } from 'react';
import { Volume2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from '@/i18n';
import { ExtensionPageLayout } from '@/features/extensions/extension-page-layout/extension-page-layout';
import { ExtensionHeader } from '@/features/extensions/extension-header/extension-header';

type PreviewState = 'idle' | 'generating' | 'playing' | 'paused' | 'stopped' | 'error';

interface PreviewEvent {
    state: PreviewState;
    detail?: string | null;
}

interface RuntimeStatus {
    installed: boolean;
    verified: boolean;
    cpuCompatible: boolean;
}

interface InstallProgress {
    receivedBytes: number;
    totalBytes: number | null;
}

export const PocketTts = () => {
    const { t } = useTranslation();
    const [state, setState] = useState<PreviewState>('idle');
    const [detail, setDetail] = useState<string | null>(null);
    const [speechText, setSpeechText] = useState('');
    const [model, setModel] = useState('french');
    const [volume, setVolume] = useState(80);
    const [downloadApproved, setDownloadApproved] = useState(false);
    const [consentBusy, setConsentBusy] = useState(false);
    const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus | null>(null);
    const [installing, setInstalling] = useState(false);
    const [installProgress, setInstallProgress] = useState<InstallProgress | null>(null);
    const [cancelRequested, setCancelRequested] = useState(false);
    const [clearingCache, setClearingCache] = useState(false);

    useEffect(() => {
        let active = true;
        void invoke<{ tts_model: string; tts_volume: number; tts_model_download_approved: boolean }>('get_all_settings')
            .then((settings) => {
                if (!active) return;
                setModel(settings.tts_model === 'french_24l' ? 'french_24l' : 'french');
                setVolume(Math.max(0, Math.min(100, settings.tts_volume)));
                setDownloadApproved(settings.tts_model_download_approved === true);
            })
            .catch(() => {
                if (active) setDetail(t('Unable to load TTS settings.'));
            });
        return () => { active = false; };
    }, [t]);

    useEffect(() => {
        let active = true;
        void invoke<RuntimeStatus>('pocket_tts_status')
            .then((status) => { if (active) setRuntimeStatus(status); })
            .catch(() => {
                if (active) setDetail(t('Unable to detect Pocket TTS runtime.'));
            });
        return () => { active = false; };
    }, [t]);

    useEffect(() => {
        let cancelled = false;
        let unsubscribe: (() => void) | undefined;

        void listen<PreviewEvent>('pocket-tts-preview', ({ payload }) => {
            if (cancelled) return;
            setState(payload.state);
            setDetail(payload.detail ?? null);
        })
            .then((stop) => {
                if (cancelled) stop();
                else unsubscribe = stop;
            })
            .catch(() => {
                if (!cancelled) setDetail(t('Unable to subscribe to Pocket TTS status.'));
            });

        return () => {
            cancelled = true;
            unsubscribe?.();
        };
    }, [t]);

    useEffect(() => {
        let active = true;
        let unlisten: (() => void) | undefined;
        void listen<string>('pocket-tts-selection-error', ({ payload }) => {
            if (!active) return;
            setState('error');
            setDetail(payload);
        }).then((fn) => {
            if (active) unlisten = fn;
            else fn();
        }).catch(() => {
            if (active) setDetail(t('Unable to subscribe to selection errors.'));
        });
        return () => { active = false; unlisten?.(); };
    }, [t]);

    const run = async (command: string, args?: { text: string }) => {
        try {
            setDetail(null);
            // Guard the UI immediately: the Tauri event is asynchronous and
            // double-clicking must not enqueue two generations.
            if (command === 'pocket_tts_preview' || command === 'pocket_tts_speak') setState('generating');
            await invoke(command, args);
        } catch (error) {
            setState('error');
            setDetail(String(error));
        }
    };

    const updateModel = async (next: string) => {
        try {
            await invoke('set_tts_model', { model: next });
            setModel(next);
        } catch (error) {
            setDetail(String(error));
        }
    };

    const updateVolume = async (next: number) => {
        try {
            await invoke('set_tts_volume', { percent: next });
            setVolume(next);
        } catch (error) {
            setDetail(String(error));
        }
    };

    useEffect(() => {
        let active = true;
        let unlisten: (() => void) | undefined;
        void listen<InstallProgress>('pocket-tts-install-progress', ({ payload }) => {
            if (active) setInstallProgress(payload);
        }).then((stop) => {
            if (active) unlisten = stop;
            else stop();
        }).catch(() => {
            if (active) setDetail(t('Unable to track Pocket TTS download progress.'));
        });
        return () => { active = false; unlisten?.(); };
    }, [t]);

    const cancelRuntimeInstall = async () => {
        try {
            if (await invoke<boolean>('pocket_tts_cancel_install')) {
                setCancelRequested(true);
            } else {
                setDetail(t('Pocket TTS installation has already finished or has not started.'));
            }
        } catch (error) {
            setDetail(String(error));
        }
    };

    const installOfficialRuntime = async () => {
        setInstalling(true);
        setInstallProgress(null);
        setCancelRequested(false);
        setDetail(null);
        try {
            await invoke('pocket_tts_install');
            const updated = await invoke<RuntimeStatus>('pocket_tts_status');
            setRuntimeStatus(updated);
            if (!updated.verified) {
                setDetail(t('Pocket TTS installed but its integrity check did not pass.'));
            }
        } catch (error) {
            setDetail(String(error));
        } finally {
            setInstalling(false);
            setInstallProgress(null);
            setCancelRequested(false);
        }
    };

    const clearModelCache = async () => {
        if (!window.confirm(t('Delete Pocket TTS models downloaded by Murmure?'))) return;
        setClearingCache(true);
        setDetail(null);
        try {
            await invoke<boolean>('pocket_tts_clear_model_cache');
            setDetail(t('Murmure Pocket TTS model cache cleared.'));
        } catch (error) {
            setDetail(String(error));
        } finally {
            setClearingCache(false);
        }
    };

    const changeDownloadApproval = async (approved: boolean) => {
        setConsentBusy(true);
        try {
            await invoke('set_tts_model_download_approved', { approved });
            setDownloadApproved(approved);
        } catch (error) {
            setDetail(String(error));
        } finally {
            setConsentBusy(false);
        }
    };

    const busy = state === 'generating' || state === 'playing' || state === 'paused';

    return (
        <ExtensionPageLayout colorScheme="standard" isActive={busy}>
            <ExtensionHeader
                icon={Volume2}
                title={t('Text to Speech')}
                colorScheme="standard"
                titleTestId="pocket-tts-title"
                subtitle={t('Preview a French voice generated locally on the CPU.')}
            />

            <section className="rounded-lg border border-border p-5 space-y-4" data-testid="pocket-tts-preview">
                <p className="text-sm text-muted-foreground">
                    {t('Pocket TTS standalone must be installed next to Murmure. The first generation may download French model weights.')}
                </p>
                {runtimeStatus && !runtimeStatus.cpuCompatible && (
                    <p role="alert" className="text-sm text-red-400">
                        {t('Your CPU does not support AVX2 required by this Pocket TTS executable.')}
                    </p>
                )}
                {runtimeStatus && runtimeStatus.cpuCompatible && runtimeStatus.installed && !runtimeStatus.verified && (
                    <p role="alert" className="text-sm text-red-400" data-testid="pocket-tts-unverified">
                        {t('Pocket TTS executable is present but does not match a verified Kyutai release for this system. Speech is disabled.')}
                    </p>
                )}
                {runtimeStatus && runtimeStatus.cpuCompatible && !runtimeStatus.installed && (
                    <p role="status" className="text-sm text-amber-500" data-testid="pocket-tts-missing">
                        {t('Pocket TTS is not installed in Murmure. Install the official verified Windows runtime explicitly.')} {' '}
                        <a href="https://github.com/kyutai-labs/pocket-tts/releases/tag/v4.0.0" target="_blank" rel="noopener noreferrer" className="underline">
                            {t('Official Pocket TTS release')}
                        </a>
                    </p>
                )}

                {navigator.userAgent.includes('Windows') && runtimeStatus?.cpuCompatible && !runtimeStatus.verified && (
                    <button
                        type="button"
                        disabled={installing || busy}
                        className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                        onClick={() => void installOfficialRuntime()}
                        data-testid="pocket-tts-install"
                    >
                        {installing ? t('Installing verified Pocket TTS…') : t('Install official Pocket TTS for Windows')}
                    </button>
                )}
                {installing && (
                    <div role="status" aria-live="polite" data-testid="pocket-tts-install-progress" className="space-y-2">
                        <p className="text-sm">
                            {cancelRequested
                                ? t('Cancelling Pocket TTS installation…')
                                : installProgress?.totalBytes
                                    ? `${t('Downloading Pocket TTS')}: ${Math.min(100, Math.round(100 * installProgress.receivedBytes / installProgress.totalBytes))}%`
                                    : `${t('Downloading Pocket TTS')}: ${installProgress?.receivedBytes ?? 0} ${t('bytes received')}`}
                        </p>
                        <button
                            type="button"
                            disabled={cancelRequested}
                            onClick={() => void cancelRuntimeInstall()}
                            className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                            data-testid="pocket-tts-cancel-install"
                        >
                            {t('Cancel installation')}
                        </button>
                    </div>
                )}
                <p className="text-xs text-muted-foreground">
                    {t('Installing the runtime downloads one pinned Kyutai executable. French model downloads require separate consent below.')}
                </p>

                <div role="status" aria-live="polite" data-testid="pocket-tts-status">
                    {state === 'idle' && t('Ready')}
                    {state === 'generating' && t('Generating audio on CPU…')}
                    {state === 'playing' && t('Playing French preview')}
                    {state === 'paused' && t('Paused')}
                    {state === 'stopped' && t('Stopped')}
                    {state === 'error' && t('Pocket TTS error')}
                </div>

                {detail && (
                    <p role="alert" className="text-sm text-red-400" data-testid="pocket-tts-error">
                        {detail}
                    </p>
                )}

                <div className="space-y-2">
                    <label className="flex items-start gap-2 text-sm">
                        <input
                            type="checkbox"
                            checked={downloadApproved}
                            // Revocation must remain accessible during playback.
                            disabled={consentBusy}
                            onChange={(event) => void changeDownloadApproval(event.target.checked)}
                            data-testid="pocket-tts-download-approval"
                            className="mt-1"
                        />
                        <span>{t('I authorize Pocket TTS to download its French model and voice files when I request speech. This permission stays on this device only.')}</span>
                    </label>
                    <p className="text-xs text-muted-foreground">
                        <a className="underline" href="https://huggingface.co/kyutai/pocket-tts-without-voice-cloning" target="_blank" rel="noopener noreferrer">
                            {t('Kyutai French model — CC BY 4.0')}
                        </a>
                        {' · '}
                        {t('Turning this off blocks future playback; it does not delete files already cached locally.')}
                    </p>
                </div>
                <button
                    type="button"
                    disabled={busy || installing || clearingCache}
                    onClick={() => void clearModelCache()}
                    className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                    data-testid="pocket-tts-clear-cache"
                >
                    {clearingCache ? t('Deleting downloaded Pocket TTS models…') : t('Delete downloaded Pocket TTS models')}
                </button>
                <label htmlFor="pocket-tts-model" className="block text-sm font-medium">
                    {t('French speech model')}
                </label>
                <select
                    id="pocket-tts-model"
                    value={model}
                    disabled={busy}
                    onChange={(event) => void updateModel(event.target.value)}
                    className="w-full rounded-md border border-border bg-background p-2 text-sm"
                    data-testid="pocket-tts-model"
                >
                    <option value="french">{t('French — efficient CPU model')}</option>
                    <option value="french_24l">{t('French — 24 layers (larger experimental model)')}</option>
                </select>
                <label htmlFor="pocket-tts-volume" className="block text-sm font-medium">
                    {t('Playback volume')}: {volume}%
                </label>
                <input
                    id="pocket-tts-volume"
                    type="range"
                    min={0}
                    max={100}
                    step={5}
                    value={volume}
                    onChange={(event) => setVolume(Number(event.target.value))}
                    onPointerUp={(event) => void updateVolume(Number(event.currentTarget.value))}
                    onKeyUp={(event) => void updateVolume(Number(event.currentTarget.value))}
                    onBlur={(event) => void updateVolume(Number(event.currentTarget.value))}
                    className="w-full"
                    data-testid="pocket-tts-volume"
                />
                <p className="text-xs text-muted-foreground">
                    {t('Changes apply to the next playback. Larger models may require more RAM and a separate download.')}
                </p>
                <label htmlFor="pocket-tts-text" className="block text-sm font-medium">
                    {t('Text to read aloud')}
                </label>
                <textarea
                    id="pocket-tts-text"
                    rows={3}
                    maxLength={1000}
                    value={speechText}
                    onChange={(event) => setSpeechText(event.target.value)}
                    className="w-full rounded-md border border-border bg-background p-3 text-sm"
                    data-testid="pocket-tts-text"
                    placeholder={t('Type a French sentence to read locally…')}
                />
                <p className="text-xs text-muted-foreground">
                    {t('Only text you explicitly submit is synthesized. Code blocks and likely credential lines are omitted.')}
                </p>
                <div className="flex flex-wrap gap-3">
                    <button
                        type="button"
                        className="rounded-md bg-sky-700 px-4 py-2 text-white disabled:opacity-50"
                        disabled={busy || consentBusy || !downloadApproved || !speechText.trim() || runtimeStatus?.verified !== true}
                        onClick={() => void run('pocket_tts_speak', { text: speechText })}
                        data-testid="pocket-tts-speak"
                    >
                        {t('Read my text')}
                    </button>
                    <button
                        type="button"
                        className="rounded-md bg-sky-700 px-4 py-2 text-white disabled:opacity-50"
                        disabled={busy || consentBusy || !downloadApproved || runtimeStatus?.verified !== true}
                        onClick={() => void run('pocket_tts_preview')}
                        data-testid="pocket-tts-play"
                    >
                        {t('Test French voice')}
                    </button>
                    <button
                        type="button"
                        className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                        disabled={state !== 'playing'}
                        onClick={() => void run('pocket_tts_pause')}
                    >
                        {t('Pause')}
                    </button>
                    <button
                        type="button"
                        className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                        disabled={state !== 'paused'}
                        onClick={() => void run('pocket_tts_resume')}
                    >
                        {t('Resume')}
                    </button>
                    <button
                        type="button"
                        className="rounded-md border border-border px-4 py-2 disabled:opacity-50"
                        disabled={!busy}
                        onClick={() => void run('pocket_tts_stop')}
                    >
                        {t('Stop')}
                    </button>
                </div>
            </section>
            <p className="text-xs text-muted-foreground">
                {t('Text-to-speech currently requires explicit input. Selection reading and global shortcuts will follow after runtime validation.')}
            </p>
        </ExtensionPageLayout>
    );
};
