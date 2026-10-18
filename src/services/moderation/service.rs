use super::local_rules::inspect_request;
use super::{
    build_admin_panel, build_decision, build_moderation_prompt, map_openai_error,
    normalize_verdict, parse_ai_payload, resolve_image_data_url, verdict_order,
    AiExecutionOutcome, DEFAULT_MODERATION_GROQ_MODEL_CHAIN, MODERATION_SYSTEM_PROMPT,
    ModerationAiAssessment, ModerationCategory, ModerationParseFailure, ModerationProvider,
    ModerationProviderFailure, ModerationRequest, ModerationResult, ModerationService,
    ModerationServiceError, ModerationVerdict, VISION_GROQ_MODEL_CHAIN, VISION_SYSTEM_PROMPT,
};
use crate::audio::ia::groq::{
    load_groq_api_keys, GroqAttemptFailure, GroqChatRequest, GroqClient, GroqClientError,
};
use crate::audio::ia::openai::{OpenAiChatRequest, OpenAiClient, OpenAiClientError};

impl ModerationService {
    pub fn from_env() -> Result<Self, ModerationServiceError> {
        let groq = match GroqClient::from_env_with_model_chain(
            DEFAULT_MODERATION_GROQ_MODEL_CHAIN
                .iter()
                .map(ToString::to_string)
                .collect(),
        ) {
            Ok(client) => Some(client),
            Err(GroqClientError::MissingApiKeys) => None,
            Err(error) => {
                return Err(ModerationServiceError::ProviderInitialization {
                    provider: ModerationProvider::Groq.as_str(),
                    message: error.to_string(),
                });
            }
        };

        let openai = match OpenAiClient::from_env() {
            Ok(client) => Some(client),
            Err(OpenAiClientError::MissingApiKey) => None,
            Err(error) => {
                return Err(ModerationServiceError::ProviderInitialization {
                    provider: ModerationProvider::OpenAi.as_str(),
                    message: error.to_string(),
                });
            }
        };

        Ok(Self { groq, openai })
    }

    #[must_use]
    pub fn new(groq: Option<GroqClient>, openai: Option<OpenAiClient>) -> Self {
        Self { groq, openai }
    }

    pub async fn moderate(&self, request: &ModerationRequest) -> ModerationResult {
        let local_findings = inspect_request(request);
        let ai = if local_findings
            .iter()
            .any(|finding| finding.verdict == ModerationVerdict::Rejected)
        {
            AiExecutionOutcome::default()
        } else {
            self.execute_ai_layer(request).await
        };

        /* Capa de visión: analizar imágenes si hay y el texto pasó. */
        let vision = if !request.image_urls.is_empty()
            && !matches!(ai.assessment.as_ref().map(|a| a.verdict), Some(ModerationVerdict::Rejected))
        {
            self.execute_vision_layer(&request.image_urls).await
        } else {
            AiExecutionOutcome::default()
        };

        let decision = build_decision(request, &local_findings, &ai, &vision);
        let admin_panel = build_admin_panel(request, &decision, &local_findings, &ai);

        ModerationResult {
            local_findings,
            ai_assessment: ai.assessment,
            provider_failures: {
                let mut all = ai.failures;
                all.extend(vision.failures);
                all
            },
            decision,
            admin_panel,
        }
    }

    /* Analiza imágenes con modelos vision (Llama 4 Scout/Maverick).
     * Replica la capa 2 del legacy PHP: por cada imagen, consulta Groq Vision
     * y devuelve el veredicto más restrictivo. */
    async fn execute_vision_layer(&self, image_urls: &[String]) -> AiExecutionOutcome {
        if self.groq.is_none() {
            return AiExecutionOutcome::default();
        }

        let mut worst_verdict = ModerationVerdict::Approved;
        let mut worst_category = ModerationCategory::Safe;
        let mut worst_summary = String::new();
        let mut worst_confidence: f32 = 0.0;
        let mut failures = Vec::new();

        let vision_chain: Vec<String> =
            VISION_GROQ_MODEL_CHAIN.iter().map(ToString::to_string).collect();
        let Ok(vision_client) = GroqClient::with_model_chain(
            load_groq_api_keys(),
            vision_chain,
        ) else {
            return AiExecutionOutcome::default();
        };

        for image_url in image_urls {
            let data_url = match resolve_image_data_url(image_url).await {
                Ok(url) => url,
                Err(message) => {
                    failures.push(ModerationProviderFailure::Groq(GroqAttemptFailure {
                        model: "vision-resolver".into(),
                        key_index: 0,
                        status_code: None,
                        retry_after_seconds: None,
                        retryable: false,
                        message,
                    }));
                    continue;
                }
            };

            let request = GroqChatRequest {
                user_prompt: format!(
                    "Analiza esta imagen para moderación en una comunidad musical. {VISION_SYSTEM_PROMPT}"
                ),
                system_prompt: "Eres un moderador de imágenes para una comunidad musical.".into(),
                temperature: 0.1,
                max_tokens: 400,
                require_json_object: true,
                images: vec![data_url],
            };

            match vision_client.chat_completion(&request).await {
                Ok(success) => match parse_ai_payload(&success.content) {
                    Ok(payload) => {
                        let category =
                            ModerationCategory::from_raw(payload.category.as_deref().unwrap_or("safe"));
                        let confidence = payload.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
                        let verdict = normalize_verdict(
                            payload.recommended_level.as_deref(),
                            payload.safe,
                            confidence,
                        );
                        let summary = payload.summary.unwrap_or_default();

                        if verdict_order(verdict) > verdict_order(worst_verdict) {
                            worst_verdict = verdict;
                            worst_category = category;
                            worst_confidence = confidence;
                            worst_summary = summary;
                        }
                    }
                    Err(message) => failures.push(ModerationProviderFailure::Parse(
                        ModerationParseFailure {
                            provider: ModerationProvider::Groq,
                            model: success.model,
                            message,
                        },
                    )),
                },
                Err(GroqClientError::Exhausted {
                    failures: inner, ..
                }) => failures
                    .extend(inner.into_iter().map(ModerationProviderFailure::Groq)),
                Err(_) => {}
            }
        }

        if worst_verdict == ModerationVerdict::Approved {
            AiExecutionOutcome { failures, ..Default::default() }
        } else {
            AiExecutionOutcome {
                assessment: Some(ModerationAiAssessment {
                    provider: ModerationProvider::Groq,
                    model: "vision".into(),
                    category: worst_category,
                    verdict: worst_verdict,
                    confidence: worst_confidence,
                    reason_code: format!("vision_{}", worst_category.as_str()),
                    summary: worst_summary,
                    attempt_count: image_urls.len(),
                    provider_key_index: None,
                }),
                failures,
                providers_configured: true,
            }
        }
    }

    async fn execute_ai_layer(&self, request: &ModerationRequest) -> AiExecutionOutcome {
        if !request.has_content() {
            return AiExecutionOutcome::default();
        }

        let prompt = build_moderation_prompt(request);
        let mut outcome = AiExecutionOutcome {
            providers_configured: self.groq.is_some() || self.openai.is_some(),
            ..AiExecutionOutcome::default()
        };

        if let Some(groq) = &self.groq {
            let request = GroqChatRequest {
                user_prompt: prompt.clone(),
                system_prompt: MODERATION_SYSTEM_PROMPT.to_owned(),
                temperature: 0.1,
                max_tokens: 400,
                require_json_object: true,
                images: Vec::new(),
            };

            match groq.chat_completion(&request).await {
                Ok(success) => match parse_ai_payload(&success.content) {
                    Ok(payload) => {
                        outcome.assessment = Some(payload.into_assessment(
                            ModerationProvider::Groq,
                            success.model,
                            success.attempt_count,
                            Some(success.key_index),
                        ));
                        return outcome;
                    }
                    Err(message) => outcome.failures.push(ModerationProviderFailure::Parse(
                        ModerationParseFailure {
                            provider: ModerationProvider::Groq,
                            model: success.model,
                            message,
                        },
                    )),
                },
                Err(GroqClientError::Exhausted { failures }) => outcome
                    .failures
                    .extend(failures.into_iter().map(ModerationProviderFailure::Groq)),
                Err(GroqClientError::MissingApiKeys | GroqClientError::MissingModels) => {}
            }
        }

        if let Some(openai) = &self.openai {
            let request = OpenAiChatRequest {
                user_prompt: prompt,
                system_prompt: MODERATION_SYSTEM_PROMPT.to_owned(),
                temperature: 0.1,
                max_tokens: 400,
                require_json_object: true,
            };

            match openai.chat_completion(&request).await {
                Ok(success) => match parse_ai_payload(&success.content) {
                    Ok(payload) => {
                        outcome.assessment = Some(payload.into_assessment(
                            ModerationProvider::OpenAi,
                            success.model,
                            success.attempt_count,
                            None,
                        ));
                    }
                    Err(message) => outcome.failures.push(ModerationProviderFailure::Parse(
                        ModerationParseFailure {
                            provider: ModerationProvider::OpenAi,
                            model: success.model,
                            message,
                        },
                    )),
                },
                Err(error) => outcome
                    .failures
                    .push(ModerationProviderFailure::OpenAi(map_openai_error(error))),
            }
        }

        outcome
    }
}
