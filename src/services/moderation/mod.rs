mod service;

mod local_rules;
mod types;

use crate::audio::ia::groq::GroqClient;
use crate::audio::ia::json_repairer::JsonRepairer;
use crate::audio::ia::openai::{OpenAiClient, OpenAiClientError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

pub use types::{
    ModerationAdminPanel, ModerationAiAssessment, ModerationCategory, ModerationDecision,
    ModerationEntityKind, ModerationLocalFinding, ModerationOpenAiFailure, ModerationParseFailure,
    ModerationProvider, ModerationProviderFailure, ModerationRequest, ModerationResult,
    ModerationVerdict,
};

const MODERATION_SYSTEM_PROMPT: &str = "You moderate user-generated metadata for Kamples, a music platform. Return JSON only. Prefer 'revision' when uncertain. Harmless profanity alone is not enough to reject. Focus on spam, scams, explicit sexual content, violent threats, hate, doxxing, illegal activity, copyright risk, and misleading metadata.";

const VISION_SYSTEM_PROMPT: &str = "You are an image content moderator for Kamples, a music production community. Analyze this image and return ONLY a JSON object with this exact format:\n{\"safe\": true/false, \"category\": \"safe|sexual|violence|illegal|spam|other\", \"confidence\": 0.0-1.0, \"summary\": \"brief description\"}\n\nBe STRICT about: explicit sexual content, graphic violence, illegal content, gore.\nBe PERMISSIVE about: artistic nudity, album art, music production screenshots, concert photos, mild language in images.\nWhen uncertain, set safe=true with lower confidence.";

const DEFAULT_MODERATION_GROQ_MODEL_CHAIN: [&str; 4] = [
    "openai/gpt-oss-safeguard-20b",
    "openai/gpt-oss-120b",
    "moonshotai/kimi-k2-instruct-0905",
    "llama-3.3-70b-versatile",
];

/* Modelos con capacidad de visión (multimodal) — mismos que usa el legacy PHP. */
const VISION_GROQ_MODEL_CHAIN: [&str; 2] = [
    "meta-llama/llama-4-scout-17b-16e-instruct",
    "meta-llama/llama-4-maverick-17b-128e-instruct",
];

/* Máximo 3 MB por imagen para data URL (límite de Groq). */
const MAX_VISION_IMAGE_BYTES: usize = 3 * 1024 * 1024;

/* [174A-43] Servicio de moderación reusable.
 * Separa cuatro capas explícitas: pre-filtro local, categorización IA,
 * decisión final y payload listo para panel admin. Todavía no persiste en una
 * cola porque el runtime nuevo aún no portó `moderation_queue`. */

#[derive(Clone)]
pub struct ModerationService {
    groq: Option<GroqClient>,
    openai: Option<OpenAiClient>,
}

#[derive(Debug, Error)]
pub enum ModerationServiceError {
    #[error("No se pudo inicializar {provider}: {message}")]
    ProviderInitialization {
        provider: &'static str,
        message: String,
    },
}

#[derive(Debug, Default)]
struct AiExecutionOutcome {
    assessment: Option<ModerationAiAssessment>,
    failures: Vec<ModerationProviderFailure>,
    providers_configured: bool,
}

#[derive(Debug, Deserialize)]
struct ModerationAiPayload {
    safe: Option<bool>,
    category: Option<String>,
    confidence: Option<f32>,
    recommended_level: Option<String>,
    reason_code: Option<String>,
    summary: Option<String>,
}

impl ModerationAiPayload {
    fn into_assessment(
        self,
        provider: ModerationProvider,
        model: String,
        attempt_count: usize,
        provider_key_index: Option<usize>,
    ) -> ModerationAiAssessment {
        let confidence = self.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
        let category = ModerationCategory::from_raw(self.category.as_deref().unwrap_or("safe"));
        let verdict = normalize_verdict(self.recommended_level.as_deref(), self.safe, confidence);
        let reason_code = sanitize_reason_code(
            self.reason_code
                .as_deref()
                .unwrap_or_else(|| category.as_str()),
        );

        ModerationAiAssessment {
            provider,
            model,
            category,
            verdict,
            confidence,
            reason_code,
            summary: self
                .summary
                .unwrap_or_else(|| default_summary(verdict, category)),
            attempt_count,
            provider_key_index,
        }
    }
}

fn parse_ai_payload(raw: &str) -> Result<ModerationAiPayload, String> {
    let object = JsonRepairer::extract_json_object(raw).map_err(|error| error.to_string())?;
    serde_json::from_value(Value::Object(object)).map_err(|error| error.to_string())
}

fn build_decision(
    request: &ModerationRequest,
    local_findings: &[ModerationLocalFinding],
    ai: &AiExecutionOutcome,
    vision: &AiExecutionOutcome,
) -> ModerationDecision {
    if let Some(local) = strongest_local_finding(local_findings) {
        if local.verdict == ModerationVerdict::Rejected {
            return ModerationDecision {
                verdict: local.verdict,
                category: local.category,
                reason_code: local.reason_code.clone(),
                summary: format!("Prefiltro local detecto '{}'", local.matched_text),
                manual_review: true,
            };
        }
    }

    /* El veredicto más restrictivo entre texto IA y visión IA prevalece. */
    let best_ai = match (&ai.assessment, &vision.assessment) {
        (Some(text), Some(vis)) => {
            if verdict_order(vis.verdict) > verdict_order(text.verdict) {
                vision
            } else {
                ai
            }
        }
        (Some(_) | None, None) => ai,
        (None, Some(_)) => vision,
    };

    if let Some(assessment) = &best_ai.assessment {
        let verdict = match assessment.verdict {
            ModerationVerdict::Rejected if assessment.confidence >= 0.85 => {
                ModerationVerdict::Rejected
            }
            ModerationVerdict::Rejected | ModerationVerdict::Review => ModerationVerdict::Review,
            ModerationVerdict::Approved if strongest_local_finding(local_findings).is_some() => {
                ModerationVerdict::Review
            }
            ModerationVerdict::Approved => ModerationVerdict::Approved,
        };

        return ModerationDecision {
            verdict,
            category: assessment.category,
            reason_code: assessment.reason_code.clone(),
            summary: assessment.summary.clone(),
            manual_review: verdict != ModerationVerdict::Approved,
        };
    }

    if let Some(local) = strongest_local_finding(local_findings) {
        return ModerationDecision {
            verdict: ModerationVerdict::Review,
            category: local.category,
            reason_code: local.reason_code.clone(),
            summary: format!(
                "Prefiltro local marco '{}' para revision",
                local.matched_text
            ),
            manual_review: true,
        };
    }

    if ai.providers_configured && !ai.failures.is_empty() && request.has_content() {
        return ModerationDecision {
            verdict: ModerationVerdict::Review,
            category: ModerationCategory::Other,
            reason_code: "provider_failure".to_owned(),
            summary: "Los proveedores IA fallaron; se requiere revision manual".to_owned(),
            manual_review: true,
        };
    }

    ModerationDecision {
        verdict: ModerationVerdict::Approved,
        category: ModerationCategory::Safe,
        reason_code: if ai.providers_configured {
            "sin_hallazgos"
        } else {
            "local_only_fallback"
        }
        .to_owned(),
        summary: default_summary(ModerationVerdict::Approved, ModerationCategory::Safe),
        manual_review: false,
    }
}

fn build_admin_panel(
    request: &ModerationRequest,
    decision: &ModerationDecision,
    local_findings: &[ModerationLocalFinding],
    ai: &AiExecutionOutcome,
) -> ModerationAdminPanel {
    let mut badges = vec![
        request.entity_kind.as_str().to_owned(),
        decision.verdict.as_str().to_owned(),
        decision.category.as_str().to_owned(),
    ];
    if let Some(assessment) = &ai.assessment {
        badges.push(assessment.provider.as_str().to_owned());
    } else if !ai.providers_configured {
        badges.push("local_only".to_owned());
    }

    let mut evidence = local_findings
        .iter()
        .map(|finding| finding.matched_text.clone())
        .collect::<Vec<_>>();
    if let Some(assessment) = &ai.assessment {
        evidence.push(format!(
            "ia:{} ({:.0}%)",
            assessment.reason_code,
            assessment.confidence * 100.0
        ));
    }
    evidence.extend(ai.failures.iter().take(2).map(summarize_failure));

    ModerationAdminPanel {
        headline: format!(
            "{} {}",
            request.entity_kind.as_str(),
            decision.verdict.as_str()
        ),
        summary: decision.summary.clone(),
        badges,
        evidence,
        priority: match decision.verdict {
            ModerationVerdict::Rejected => 95,
            ModerationVerdict::Review => 70,
            ModerationVerdict::Approved if ai.providers_configured => 15,
            ModerationVerdict::Approved => 10,
        },
    }
}

fn strongest_local_finding(findings: &[ModerationLocalFinding]) -> Option<&ModerationLocalFinding> {
    findings
        .iter()
        .max_by_key(|finding| finding.verdict.priority())
}

fn build_moderation_prompt(request: &ModerationRequest) -> String {
    format!(
        "Moderate this {kind} for Kamples and return JSON with keys safe, category, confidence, recommended_level, reason_code, summary. Use category one of: safe, spam, sexual, violence, hate, harassment, illegal, doxxing, scam, copyright, misleading_metadata, other. recommended_level must be aprobado, revision or rechazado.\n\nContent:\n{content}",
        kind = request.entity_kind.as_str(),
        content = request.combined_text(),
    )
}

fn normalize_verdict(
    level: Option<&str>,
    safe: Option<bool>,
    confidence: f32,
) -> ModerationVerdict {
    match level
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "rechazado" | "rejected" => ModerationVerdict::Rejected,
        "revision" | "review" => ModerationVerdict::Review,
        "aprobado" | "approved" => ModerationVerdict::Approved,
        _ => match safe {
            Some(true) => ModerationVerdict::Approved,
            Some(false) if confidence >= 0.85 => ModerationVerdict::Rejected,
            Some(false) | None => ModerationVerdict::Review,
        },
    }
}

fn sanitize_reason_code(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }
    sanitized.trim_matches('_').to_owned()
}

fn default_summary(verdict: ModerationVerdict, category: ModerationCategory) -> String {
    match verdict {
        ModerationVerdict::Approved => "Sin hallazgos relevantes de moderacion".to_owned(),
        ModerationVerdict::Review => {
            format!("Contenido marcado para revision por {}", category.as_str())
        }
        ModerationVerdict::Rejected => format!("Contenido bloqueado por {}", category.as_str()),
    }
}

fn summarize_failure(failure: &ModerationProviderFailure) -> String {
    match failure {
        ModerationProviderFailure::Groq(failure) => format!("groq:{}", failure.message),
        ModerationProviderFailure::OpenAi(failure) => format!("openai:{}", failure.message),
        ModerationProviderFailure::Parse(failure) => format!("parse:{}", failure.message),
    }
}

fn verdict_order(verdict: ModerationVerdict) -> u8 {
    match verdict {
        ModerationVerdict::Approved => 0,
        ModerationVerdict::Review => 1,
        ModerationVerdict::Rejected => 2,
    }
}

/* Convierte una URL de imagen (local `/uploads/...` o HTTP) a data URL base64
 * para el API de Groq Vision. Máximo 3 MB. */
async fn resolve_image_data_url(image_url: &str) -> Result<String, String> {
    if image_url.starts_with("data:") {
        return Ok(image_url.to_owned());
    }

    let bytes = if image_url.starts_with("http://") || image_url.starts_with("https://") {
        reqwest::get(image_url)
            .await
            .map_err(|e| format!("HTTP fetch failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("HTTP read failed: {e}"))?
            .to_vec()
    } else {
        /* Ruta local: resolver relativo a UPLOADS_DIR o directorio de trabajo. */
        let path = if image_url.starts_with('/') {
            std::path::PathBuf::from(image_url.trim_start_matches('/'))
        } else {
            std::path::PathBuf::from(image_url)
        };
        tokio::fs::read(&path)
            .await
            .map_err(|e| format!("Local read failed for {}: {e}", path.display()))?
    };

    if bytes.len() > MAX_VISION_IMAGE_BYTES {
        return Err(format!(
            "Image too large: {} bytes (max {} bytes)",
            bytes.len(),
            MAX_VISION_IMAGE_BYTES
        ));
    }

    let mime = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if bytes.len() > 4 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err("Unknown image format (expected JPEG, PNG, or WebP)".into());
    };

    Ok(format!(
        "data:{mime};base64,{}",
        BASE64.encode(&bytes)
    ))
}

fn map_openai_error(error: OpenAiClientError) -> ModerationOpenAiFailure {
    match error {
        OpenAiClientError::MissingApiKey => ModerationOpenAiFailure {
            status_code: None,
            retry_after_seconds: None,
            retryable: false,
            message: "missing_api_key".to_owned(),
        },
        OpenAiClientError::JsonSchemaRejected {
            status_code,
            message,
        } => ModerationOpenAiFailure {
            status_code: Some(status_code),
            retry_after_seconds: None,
            retryable: false,
            message,
        },
        OpenAiClientError::Request {
            status_code,
            retry_after_seconds,
            retryable,
            message,
        } => ModerationOpenAiFailure {
            status_code,
            retry_after_seconds,
            retryable,
            message,
        },
    }
}

#[cfg(test)]
mod tests;
