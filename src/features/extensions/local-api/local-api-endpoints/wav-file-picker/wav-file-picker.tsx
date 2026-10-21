import { useRef } from 'react';
import { FileAudio } from 'lucide-react';
import { Page } from '@/components/page';
import { useTranslation } from '@/i18n';

interface WavFilePickerProps {
    file: File | null;
    onChange: (file: File | null) => void;
    testId: string;
}

export const WavFilePicker = ({ file, onChange, testId }: WavFilePickerProps) => {
    const { t } = useTranslation();
    const inputRef = useRef<HTMLInputElement>(null);

    return (
        <div className="flex items-center gap-3 min-w-0">
            <input
                ref={inputRef}
                type="file"
                accept=".wav,audio/wav,audio/x-wav"
                className="hidden"
                // The visible button is the control: it carries the name and
                // receives the focus. The input only exists to open the file
                // dialogue, so it is removed from the tab order and hidden
                // from assistive technology. Without this, keyboard users
                // focus an invisible field and screen readers announce an
                // unnamed control (2.4.3 and 4.1.2).
                tabIndex={-1}
                aria-hidden="true"
                onChange={(event) => onChange(event.target.files?.[0] ?? null)}
                data-testid={`${testId}-input`}
            />
            <Page.SecondaryButton size="sm" onClick={() => inputRef.current?.click()} data-testid={testId}>
                <FileAudio className="w-3.5 h-3.5" />
                {t('Choose a WAV file')}
            </Page.SecondaryButton>
            {file !== null && <span className="text-sm text-foreground truncate">{file.name}</span>}
        </div>
    );
};
