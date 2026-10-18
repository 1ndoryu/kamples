use super::*;

pub(super) fn normalize_tags(raw: &str) -> Vec<String> {
    let candidates = if raw.trim_start().starts_with('[') {
        serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
    } else {
        raw.split(',').map(ToString::to_string).collect()
    };

    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in candidates {
        let normalized_tag = tag.trim().to_ascii_lowercase();
        if normalized_tag.is_empty() || !seen.insert(normalized_tag.clone()) {
            continue;
        }
        normalized.push(normalized_tag);
    }
    normalized
}

pub(super) fn detect_audio_format(
    bytes: &[u8],
    filename: Option<&str>,
) -> Result<AudioUploadFormat, AppError> {
    let extension = filename
        .and_then(|value| std::path::Path::new(value).extension())
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let allowed_extension = matches!(
        extension.as_str(),
        "wav" | "mp3" | "flac" | "aiff" | "aif" | "ogg"
    );
    if !allowed_extension {
        return Err(AppError::UnsupportedMediaType(
            "Formato de audio no valido. Formatos aceptados: WAV, MP3, FLAC, AIFF, OGG".into(),
        ));
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Ok(AudioUploadFormat::Wav);
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"fLaC" {
        return Ok(AudioUploadFormat::Flac);
    }
    if bytes.len() >= 12
        && &bytes[0..4] == b"FORM"
        && (&bytes[8..12] == b"AIFF" || &bytes[8..12] == b"AIFC")
    {
        return Ok(AudioUploadFormat::Aiff);
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
        return Ok(AudioUploadFormat::Ogg);
    }
    if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
        return Ok(AudioUploadFormat::Mp3);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return Ok(AudioUploadFormat::Mp3);
    }

    Err(AppError::UnsupportedMediaType(
        "El contenido del archivo no coincide con un formato de audio valido".into(),
    ))
}

pub(super) fn generate_short_id() -> String {
    const ALPHABET: [char; 36] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ];
    nanoid::nanoid!(8, &ALPHABET)
}

pub(super) fn build_storage_key(user_id: i32, slug: &str, extension: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "samples/{}/{:04}/{:02}/{}.{}",
        user_id,
        now.year(),
        now.month(),
        slug,
        extension,
    )
}

pub(super) fn build_upload_url(state: &AppState, storage_key: &str) -> String {
    if let Some(base) = &state.public_base_url {
        format!("{}/uploads/{}", base.trim_end_matches('/'), storage_key)
    } else {
        format!("/uploads/{storage_key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn normalizes_tags_like_legacy() {
        let tags = normalize_tags("  Trap,drill,TRAP , boom bap ");
        assert_eq!(tags, vec!["trap", "drill", "boom bap"]);
    }

    #[test]
    fn builds_storage_key_under_samples_prefix() {
        let key = build_storage_key(9, "mi-sample-abc123", "wav");
        let now = chrono::Utc::now();
        assert!(key.starts_with(&format!("samples/9/{:04}/{:02}/", now.year(), now.month())));
        assert!(std::path::Path::new(&key)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav")));
    }
}
