use crate::audio::ia::groq::{GroqAttemptFailure, GroqChatRequest, GroqClient, GroqClientError};
use crate::audio::ia::json_repairer::{AudioCreativeMetadata, JsonRepairer};
use crate::audio::ia::openai::{OpenAiChatRequest, OpenAiClient, OpenAiClientError};
use crate::audio::ia::prompts::{
    build_analysis_prompt, build_correction_prompt, AudioAnalysisPromptInput,
    AudioExtractionContext, MetadataCorrectionPromptInput,
};
use thiserror::Error;

/* [174A-41] Servicio IA del dominio audio.
 * Orquesta prompt -> Groq -> JsonRepairer -> OpenAI fallback sin mezclar HTTP ni
 * parsing en el worker. Deja `is_retryable()` y `retry_after_seconds()` listos
 * para que el worker de la siguiente tarea decida backoff sin reinterpretar fallos. */

#[derive(Clone)]
pub struct AudioIaService {
    groq: Option<GroqClient>,
    openai: Option<OpenAiClient>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetadataCorrectionRequest {
    pub current_metadata_json: String,
    pub title: String,
    pub instructions: String,
    pub bpm: Option<f32>,
    pub musical_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioIaAnalysisRequest {
    pub original_filename: String,
    pub user_description: String,
    pub user_tags: Vec<String>,
    pub bpm: Option<f32>,
    pub musical_key: Option<String>,
    pub scale: Option<String>,
    pub duration_seconds: Option<f32>,
    pub upload_origin: Option<String>,
    pub extraction_context: Option<AudioExtractionContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioIaProvider {
    Groq,
    OpenAi,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioIaAnalysisResult {
    pub metadata: AudioCreativeMetadata,
    pub provider: AudioIaProvider,
    pub model: String,
    pub attempt_count: usize,
    pub provider_key_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioIaFailure {
    Groq(GroqAttemptFailure),
    OpenAi(OpenAiAttemptFailure),
    Parse(AudioIaParseFailure),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiAttemptFailure {
    pub status_code: Option<u16>,
    pub retry_after_seconds: Option<f32>,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioIaParseFailure {
    pub provider: AudioIaProvider,
    pub model: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum AudioIaServiceError {
    #[error("No hay proveedores IA configurados")]
    MissingProviders,
    #[error("No se pudo inicializar {provider}: {message}")]
    ProviderInitialization {
        provider: &'static str,
        message: String,
    },
    #[error("Todos los proveedores IA fallaron")]
    Exhausted { failures: Vec<AudioIaFailure> },
}

impl AudioIaService {
    pub fn from_env() -> Result<Self, AudioIaServiceError> {
        let groq = match GroqClient::from_env() {
            Ok(client) => Some(client),
            Err(GroqClientError::MissingApiKeys) => None,
            Err(error) => {
                return Err(AudioIaServiceError::ProviderInitialization {
                    provider: AudioIaProvider::Groq.as_str(),
                    message: error.to_string(),
                });
            }
        };

        let openai = match OpenAiClient::from_env() {
            Ok(client) => Some(client),
            Err(OpenAiClientError::MissingApiKey) => None,
            Err(error) => {
                return Err(AudioIaServiceError::ProviderInitialization {
                    provider: AudioIaProvider::OpenAi.as_str(),
                    message: error.to_string(),
                });
            }
        };

        Self::new(groq, openai)
    }

    pub fn new(
        groq: Option<GroqClient>,
        openai: Option<OpenAiClient>,
    ) -> Result<Self, AudioIaServiceError> {
        if groq.is_none() && openai.is_none() {
            return Err(AudioIaServiceError::MissingProviders);
        }

        Ok(Self { groq, openai })
    }

    pub async fn analyze_audio(
        &self,
        request: &AudioIaAnalysisRequest,
    ) -> Result<AudioIaAnalysisResult, AudioIaServiceError> {
        let prompt = build_analysis_prompt(&AudioAnalysisPromptInput::from(request));
        let mut failures = Vec::new();

        if let Some(groq) = &self.groq {
            match groq
                .chat_completion(&GroqChatRequest::new(prompt.clone()))
                .await
            {
                Ok(success) => match JsonRepairer::extract_metadata_from_text(&success.content) {
                    Ok(metadata) => {
                        return Ok(AudioIaAnalysisResult {
                            metadata,
                            provider: AudioIaProvider::Groq,
                            model: success.model,
                            attempt_count: success.attempt_count,
                            provider_key_index: Some(success.key_index),
                        });
                    }
                    Err(error) => failures.push(AudioIaFailure::Parse(AudioIaParseFailure {
                        provider: AudioIaProvider::Groq,
                        model: success.model,
                        message: error.to_string(),
                    })),
                },
                Err(GroqClientError::Exhausted {
                    failures: groq_failures,
                }) => {
                    failures.extend(groq_failures.into_iter().map(AudioIaFailure::Groq));
                }
                Err(GroqClientError::MissingApiKeys | GroqClientError::MissingModels) => {}
            }
        }

        if let Some(openai) = &self.openai {
            match openai
                .chat_completion(&OpenAiChatRequest::new(prompt))
                .await
            {
                Ok(success) => match JsonRepairer::extract_metadata_from_text(&success.content) {
                    Ok(metadata) => {
                        return Ok(AudioIaAnalysisResult {
                            metadata,
                            provider: AudioIaProvider::OpenAi,
                            model: success.model,
                            attempt_count: success.attempt_count,
                            provider_key_index: None,
                        });
                    }
                    Err(error) => failures.push(AudioIaFailure::Parse(AudioIaParseFailure {
                        provider: AudioIaProvider::OpenAi,
                        model: success.model,
                        message: error.to_string(),
                    })),
                },
                Err(error) => failures.push(AudioIaFailure::OpenAi(map_openai_error(error))),
            }
        }

        if failures.is_empty() {
            Err(AudioIaServiceError::MissingProviders)
        } else {
            Err(AudioIaServiceError::Exhausted { failures })
        }
    }

    /* [254A-8b] Corrige metadata IA existente segun instrucciones del usuario.
     * Usa la misma cadena Groq -> OpenAI fallback que `analyze_audio` pero con
     * `build_correction_prompt`. Devuelve la metadata corregida lista para que
     * el handler la persista en el sample. */
    pub async fn correct_metadata(
        &self,
        request: &MetadataCorrectionRequest,
    ) -> Result<AudioIaAnalysisResult, AudioIaServiceError> {
        let prompt = build_correction_prompt(&MetadataCorrectionPromptInput {
            current_metadata_json: request.current_metadata_json.clone(),
            title: request.title.clone(),
            instructions: request.instructions.clone(),
            bpm: request.bpm,
            musical_key: request.musical_key.clone(),
        });
        let mut failures = Vec::new();

        if let Some(groq) = &self.groq {
            match groq
                .chat_completion(&GroqChatRequest::new(prompt.clone()))
                .await
            {
                Ok(success) => match JsonRepairer::extract_metadata_from_text(&success.content) {
                    Ok(metadata) => {
                        return Ok(AudioIaAnalysisResult {
                            metadata,
                            provider: AudioIaProvider::Groq,
                            model: success.model,
                            attempt_count: success.attempt_count,
                            provider_key_index: Some(success.key_index),
                        });
                    }
                    Err(error) => failures.push(AudioIaFailure::Parse(AudioIaParseFailure {
                        provider: AudioIaProvider::Groq,
                        model: success.model,
                        message: error.to_string(),
                    })),
                },
                Err(GroqClientError::Exhausted {
                    failures: groq_failures,
                }) => {
                    failures.extend(groq_failures.into_iter().map(AudioIaFailure::Groq));
                }
                Err(GroqClientError::MissingApiKeys | GroqClientError::MissingModels) => {}
            }
        }

        if let Some(openai) = &self.openai {
            match openai
                .chat_completion(&OpenAiChatRequest::new(prompt))
                .await
            {
                Ok(success) => match JsonRepairer::extract_metadata_from_text(&success.content) {
                    Ok(metadata) => {
                        return Ok(AudioIaAnalysisResult {
                            metadata,
                            provider: AudioIaProvider::OpenAi,
                            model: success.model,
                            attempt_count: success.attempt_count,
                            provider_key_index: None,
                        });
                    }
                    Err(error) => failures.push(AudioIaFailure::Parse(AudioIaParseFailure {
                        provider: AudioIaProvider::OpenAi,
                        model: success.model,
                        message: error.to_string(),
                    })),
                },
                Err(error) => failures.push(AudioIaFailure::OpenAi(map_openai_error(error))),
            }
        }

        if failures.is_empty() {
            Err(AudioIaServiceError::MissingProviders)
        } else {
            Err(AudioIaServiceError::Exhausted { failures })
        }
    }
}

impl AudioIaProvider {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Groq => "Groq",
            Self::OpenAi => "OpenAI",
        }
    }
}

impl AudioIaFailure {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Groq(failure) => failure.retryable,
            Self::OpenAi(failure) => failure.retryable,
            Self::Parse(_) => false,
        }
    }

    #[must_use]
    pub fn retry_after_seconds(&self) -> Option<f32> {
        match self {
            Self::Groq(failure) => failure.retry_after_seconds,
            Self::OpenAi(failure) => failure.retry_after_seconds,
            Self::Parse(_) => None,
        }
    }

    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Groq(failure) => format!(
                "Groq/{model} key#{key} status={status:?} retryable={retryable}: {message}",
                model = failure.model,
                key = failure.key_index,
                status = failure.status_code,
                retryable = failure.retryable,
                message = failure.message,
            ),
            Self::OpenAi(failure) => format!(
                "OpenAI status={status:?} retryable={retryable}: {message}",
                status = failure.status_code,
                retryable = failure.retryable,
                message = failure.message,
            ),
            Self::Parse(failure) => format!(
                "Parse {provider}/{model}: {message}",
                provider = failure.provider.as_str(),
                model = failure.model,
                message = failure.message,
            ),
        }
    }
}

impl AudioIaServiceError {
    #[must_use]
    pub fn failures(&self) -> &[AudioIaFailure] {
        match self {
            Self::Exhausted { failures } => failures,
            Self::MissingProviders | Self::ProviderInitialization { .. } => &[],
        }
    }

    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Exhausted { failures } => failures.iter().any(AudioIaFailure::is_retryable),
            Self::MissingProviders | Self::ProviderInitialization { .. } => false,
        }
    }

    #[must_use]
    pub fn retry_after_seconds(&self) -> Option<f32> {
        match self {
            Self::Exhausted { failures } => failures
                .iter()
                .filter_map(AudioIaFailure::retry_after_seconds)
                .max_by(f32::total_cmp),
            Self::MissingProviders | Self::ProviderInitialization { .. } => None,
        }
    }

    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::MissingProviders => "sin proveedores configurados".to_owned(),
            Self::ProviderInitialization { provider, message } => {
                format!("{provider}: {message}")
            }
            Self::Exhausted { failures } => failures
                .iter()
                .take(4)
                .map(AudioIaFailure::summary)
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

impl From<&AudioIaAnalysisRequest> for AudioAnalysisPromptInput {
    fn from(value: &AudioIaAnalysisRequest) -> Self {
        Self {
            original_filename: value.original_filename.clone(),
            user_description: value.user_description.clone(),
            user_tags: value.user_tags.clone(),
            bpm: value.bpm,
            musical_key: value.musical_key.clone(),
            scale: value.scale.clone(),
            duration_seconds: value.duration_seconds,
            upload_origin: value.upload_origin.clone(),
            extraction_context: value.extraction_context.clone(),
        }
    }
}

fn map_openai_error(error: OpenAiClientError) -> OpenAiAttemptFailure {
    match error {
        OpenAiClientError::MissingApiKey => OpenAiAttemptFailure {
            status_code: None,
            retry_after_seconds: None,
            retryable: false,
            message: "missing_api_key".to_owned(),
        },
        OpenAiClientError::JsonSchemaRejected {
            status_code,
            message,
        } => OpenAiAttemptFailure {
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
        } => OpenAiAttemptFailure {
            status_code,
            retry_after_seconds,
            retryable,
            message,
        },
    }
}

#[cfg(test)]
#[path = "ia_service/tests.rs"]
mod tests;
