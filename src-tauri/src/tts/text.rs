//! Local speech text preparation.
//!
//! This is intentionally separate from the audio backend: neither the
//! original STT engine nor an external synthesizer sees unfiltered tool output.
//! Filters are conservative, not a substitute for a permission boundary.

pub const MAX_INPUT_CHARS: usize = 12_000;
pub const MAX_SEGMENT_CHARS: usize = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechTextError {
    Empty,
    TooLong,
}

fn sensitive_line(line: &str) -> bool {
    let lowered = line.trim().to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "authorization:", "bearer ", "password=", "password:",
        "passwd=", "api_key", "api-key", "secret=", "secret:",
        "access_token", "refresh_token", "token=", "token:",
        "-----begin ", "sk-", "ghp_", "github_pat_", "npm_",
    ];
    MARKERS.iter().any(|marker| lowered.contains(marker))
}

fn strip_inline_code_and_urls(line: &str) -> String {
    let mut visible = String::new();
    let mut in_inline_code = false;
    // Strip backtick-delimited code and HTML tags before splitting words.
    let mut in_html_tag = false;
    for c in line.chars() {
        match c {
            '`' => in_inline_code = !in_inline_code,
            '<' if !in_inline_code => in_html_tag = true,
            '>' if in_html_tag => in_html_tag = false,
            _ if !in_inline_code && !in_html_tag => visible.push(c),
            _ => {}
        }
    }

    visible
        .split_whitespace()
        .filter(|word| {
            let w = word.trim_start_matches(|c: char| "([<\"'".contains(c));
            !["https://", "http://", "www.", "file://", "ssh://"]
                .iter()
                .any(|prefix| w.to_ascii_lowercase().starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn segment_text(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        // Normalise spaces while keeping punctuation and UTF-8 intact.
        if c.is_whitespace() {
            if !current.ends_with(' ') && !current.is_empty() {
                current.push(' ');
            }
        } else {
            current.push(c);
        }

        let len = current.chars().count();
        let sentence_end = matches!(c, '.' | '!' | '?' | '。' | '！' | '？');
        if (sentence_end && len >= 12) || len >= MAX_SEGMENT_CHARS {
            let piece = current.trim();
            if !piece.is_empty() {
                segments.push(piece.to_owned());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        segments.push(current.trim().to_owned());
    }
    segments
}

/// Build small pronounceable French-friendly segments while omitting obvious
/// code blocks, inline code, URLs and credential-looking lines.
///
/// For sensitive documents the caller MUST still require an explicit user
/// action and consent to the scope of content being read.
pub fn prepare_speech(input: &str) -> Result<Vec<String>, SpeechTextError> {
    if input.chars().count() > MAX_INPUT_CHARS {
        return Err(SpeechTextError::TooLong);
    }
    let mut content = String::new();
    let mut fence: Option<&str> = None;
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if fence.is_none() {
                fence = Some("```");
            } else if fence == Some("```") {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("~~~") {
            if fence.is_none() {
                fence = Some("~~~");
            } else if fence == Some("~~~") {
                fence = None;
            }
            continue;
        }
        if fence.is_some() || sensitive_line(line) {
            continue;
        }

        // Markdown decorations are never passed as raw binary audio commands.
        let plain = trimmed.trim_start_matches(|c: char| {
            c == '#' || c == '-' || c == '*' || c == '>' || c == ' '
        });
        let plain = strip_inline_code_and_urls(plain);
        if !plain.is_empty() {
            content.push_str(&plain);
            content.push(' ');
        }
    }

    let segments = segment_text(content.trim());
    if segments.is_empty() {
        Err(SpeechTextError::Empty)
    } else {
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_french_accents_and_splits_sentences() {
        let parts = prepare_speech("Bonjour à toutes et à tous. Ça fonctionne très bien !").unwrap();
        assert_eq!(parts, vec!["Bonjour à toutes et à tous.", "Ça fonctionne très bien !"]);
    }

    #[test]
    fn omits_code_blocks_and_inline_code() {
        let parts = prepare_speech(
            "Voici l'analyse.\n```rust\nprintln!(\"secret\");\n```\nLe test `foo()` est validé."
        ).unwrap();
        assert_eq!(parts.join(" "), "Voici l'analyse. Le test est validé.");
    }

    #[test]
    fn ignores_sensitive_lines_and_urls() {
        let parts = prepare_speech(
            "La compilation est terminée.\nAuthorization: Bearer SECRET\nLien: https://example.test/key\nStatut : vert."
        ).unwrap();
        let spoken = parts.join(" ");
        assert!(!spoken.contains("SECRET"));
        assert!(!spoken.contains("https://"));
        assert!(spoken.contains("Statut : vert."));
    }

    #[test]
    fn rejects_only_code_and_credential_content() {
        assert_eq!(prepare_speech("```\nlet x = 2;\n```"), Err(SpeechTextError::Empty));
        assert_eq!(prepare_speech("password=secret"), Err(SpeechTextError::Empty));
        assert_eq!(prepare_speech("   "), Err(SpeechTextError::Empty));
    }

    #[test]
    fn bounds_length_without_cutting_utf8() {
        assert_eq!(prepare_speech(&"é".repeat(MAX_INPUT_CHARS + 1)), Err(SpeechTextError::TooLong));
        let parts = prepare_speech(&"é".repeat(600)).unwrap();
        assert!(parts.iter().all(|p| p.chars().count() <= MAX_SEGMENT_CHARS));
        assert_eq!(parts.concat(), "é".repeat(600));
    }

    #[test]
    fn ignores_html_tags_and_keeps_content() {
        let parts = prepare_speech("<p>Bonjour</p> <span>aux utilisateurs.</span>").unwrap();
        assert_eq!(parts.join(" "), "Bonjour aux utilisateurs.");
    }

    #[test]
    fn keeps_mismatched_fences_hidden_until_matching_end() {
        let parts = prepare_speech("Avant.\n```\nsecret but code\n~~~\nstill code\n```\nAprès.").unwrap();
        assert_eq!(parts, vec!["Avant.", "Après."]);
    }
}
