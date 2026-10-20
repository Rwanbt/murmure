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

export const PocketTts = () => {
    const { t } = useTranslation();
    const [state, setState] = useState<PreviewState>('idle');
    const [detail, setDetail] = useState<string | null>(null);

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

    const run = async (command: string) => {
        try {
            setDetail(null);
            await invoke(command);
        } catch (error) {
            setState('error');
            setDetail(String(error));
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

                <div className="flex flex-wrap gap-3">
                    <button
                        type="button"
                        className="rounded-md bg-sky-700 px-4 py-2 text-white disabled:opacity-50"
                        disabled={busy}
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
                {t('Preview only: arbitrary-text TTS and global shortcuts will be enabled after privacy and runtime validation.')}
            </p>
        </ExtensionPageLayout>
    );
};
