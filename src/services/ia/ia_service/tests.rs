use super::*;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Default)]
struct MockState {
    responses: Arc<Mutex<VecDeque<MockResponse>>>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

#[derive(Debug, Clone)]
struct MockResponse {
    status: StatusCode,
    retry_after_header: Option<&'static str>,
    body: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedRequest {
    authorization: String,
    model: String,
    used_json_response_format: bool,
}

#[tokio::test]
async fn returns_metadata_from_groq_when_primary_provider_succeeds() {
    let groq_state = MockState::default();
    groq_state.responses.lock().push_back(MockResponse {
        status: StatusCode::OK,
        retry_after_header: None,
        body: json!({
            "choices": [{
                "message": {
                    "content": "{\"tags\":[\"Warm\"],\"genero\":[\"house\"],\"emocion\":[\"uplifting\"],\"instrumentos\":[\"synth\"],\"carpeta_primaria\":\"Loops\",\"carpeta_secundaria\":\"House\"}"
                }
            }]
        }),
    });

    let groq =
        GroqClient::with_model_chain(vec!["gsk_test".to_owned()], vec!["groq-main".to_owned()])
            .expect("groq client should build")
            .with_endpoint(spawn_test_server(groq_state.clone()).await)
            .with_max_attempts_per_model(1);

    let service = AudioIaService::new(Some(groq), None).expect("service should build");
    let result = service
        .analyze_audio(&build_request())
        .await
        .expect("groq should classify metadata");

    assert_eq!(result.provider, AudioIaProvider::Groq);
    assert_eq!(result.model, "groq-main");
    assert_eq!(result.provider_key_index, Some(0));
    assert_eq!(result.metadata.tags, vec!["Warm"]);
    assert_eq!(result.metadata.genero, vec!["house"]);
    assert_eq!(result.metadata.emocion, vec!["uplifting"]);
    assert_eq!(result.metadata.carpeta_primaria, "Loops");
}

#[tokio::test]
async fn falls_back_to_openai_after_groq_exhaustion() {
    let groq_state = MockState::default();
    groq_state.responses.lock().push_back(MockResponse {
        status: StatusCode::TOO_MANY_REQUESTS,
        retry_after_header: None,
        body: json!({ "error": { "message": "Please retry in 4.0s." } }),
    });

    let openai_state = MockState::default();
    openai_state.responses.lock().push_back(MockResponse {
        status: StatusCode::OK,
        retry_after_header: None,
        body: json!({
            "choices": [{
                "message": {
                    "content": "{\"tags\":[\"Dusty\"],\"genero\":[\"boom bap\"],\"instrumentos\":[\"drums\"],\"emocion\":[\"gritty\"],\"carpeta_primaria\":\"Drums\",\"carpeta_secundaria\":\"Snares\"}"
                }
            }]
        }),
    });

    let groq =
        GroqClient::with_model_chain(vec!["gsk_test".to_owned()], vec!["groq-main".to_owned()])
            .expect("groq client should build")
            .with_endpoint(spawn_test_server(groq_state).await)
            .with_max_attempts_per_model(1);
    let openai = OpenAiClient::new("sk-openai".to_owned())
        .expect("openai client should build")
        .with_endpoint(spawn_test_server(openai_state).await);

    let service = AudioIaService::new(Some(groq), Some(openai)).expect("service should build");
    let result = service
        .analyze_audio(&build_request())
        .await
        .expect("openai fallback should succeed");

    assert_eq!(result.provider, AudioIaProvider::OpenAi);
    assert_eq!(result.model, "gpt-4o-mini");
    assert_eq!(result.provider_key_index, None);
    assert_eq!(result.metadata.tags, vec!["Dusty"]);
    assert_eq!(result.metadata.carpeta_primaria, "Drums");
}

#[tokio::test]
async fn falls_back_to_openai_when_groq_response_cannot_be_repaired() {
    let groq_state = MockState::default();
    groq_state.responses.lock().push_back(MockResponse {
        status: StatusCode::OK,
        retry_after_header: None,
        body: json!({
            "choices": [{
                "message": {
                    "content": "sin estructura util"
                }
            }]
        }),
    });

    let openai_state = MockState::default();
    openai_state.responses.lock().push_back(MockResponse {
        status: StatusCode::OK,
        retry_after_header: None,
        body: json!({
            "choices": [{
                "message": {
                    "content": "{\"tags\":[\"Sad\"],\"genero\":[\"ambient\"],\"instrumentos\":[\"piano\"],\"emocion\":[\"melancholic\"],\"carpeta_primaria\":\"Instruments\",\"carpeta_secundaria\":\"Keys\"}"
                }
            }]
        }),
    });

    let groq =
        GroqClient::with_model_chain(vec!["gsk_test".to_owned()], vec!["groq-main".to_owned()])
            .expect("groq client should build")
            .with_endpoint(spawn_test_server(groq_state).await)
            .with_max_attempts_per_model(1);
    let openai = OpenAiClient::new("sk-openai".to_owned())
        .expect("openai client should build")
        .with_endpoint(spawn_test_server(openai_state).await);

    let service = AudioIaService::new(Some(groq), Some(openai)).expect("service should build");
    let result = service
        .analyze_audio(&build_request())
        .await
        .expect("parse failure on groq should fallback to openai");

    assert_eq!(result.provider, AudioIaProvider::OpenAi);
    assert_eq!(result.metadata.genero, vec!["ambient"]);
    assert_eq!(result.metadata.instrumentos, vec!["piano"]);
}

#[tokio::test]
async fn reports_retryability_and_retry_after_when_all_providers_fail() {
    let groq_state = MockState::default();
    groq_state.responses.lock().push_back(MockResponse {
        status: StatusCode::TOO_MANY_REQUESTS,
        retry_after_header: None,
        body: json!({ "error": { "message": "Please retry in 12.0s." } }),
    });

    let openai_state = MockState::default();
    openai_state.responses.lock().push_back(MockResponse {
        status: StatusCode::SERVICE_UNAVAILABLE,
        retry_after_header: Some("7"),
        body: json!({ "error": { "message": "upstream unavailable" } }),
    });

    let groq =
        GroqClient::with_model_chain(vec!["gsk_test".to_owned()], vec!["groq-main".to_owned()])
            .expect("groq client should build")
            .with_endpoint(spawn_test_server(groq_state).await)
            .with_max_attempts_per_model(1);
    let openai = OpenAiClient::new("sk-openai".to_owned())
        .expect("openai client should build")
        .with_endpoint(spawn_test_server(openai_state).await);

    let service = AudioIaService::new(Some(groq), Some(openai)).expect("service should build");
    let error = service
        .analyze_audio(&build_request())
        .await
        .expect_err("all providers should fail");

    assert!(error.is_retryable());
    assert_eq!(error.retry_after_seconds(), Some(12.0));
    assert_eq!(error.failures().len(), 2);
}

fn build_request() -> AudioIaAnalysisRequest {
    AudioIaAnalysisRequest {
        original_filename: "warm_house_loop.wav".to_owned(),
        user_description: "Analog drum loop with dusty hats".to_owned(),
        user_tags: vec!["house".to_owned(), "drums".to_owned()],
        bpm: Some(124.0),
        musical_key: Some("Am".to_owned()),
        scale: Some("minor".to_owned()),
        duration_seconds: Some(8.0),
        upload_origin: Some("Packs/House/Drums".to_owned()),
        extraction_context: None,
    }
}

async fn spawn_test_server(state: MockState) -> String {
    let app = Router::new()
        .route("/", post(chat_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("should bind test listener");
    let address = listener
        .local_addr()
        .expect("listener should expose local addr");
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("test server should serve");
    });

    format!("http://{address}/")
}

async fn chat_handler(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let model = payload
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let used_json_response_format = payload.get("response_format").is_some();
    state.requests.lock().push(RecordedRequest {
        authorization,
        model,
        used_json_response_format,
    });

    let response = state.responses.lock().pop_front().unwrap_or(MockResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        retry_after_header: None,
        body: json!({ "error": { "message": "missing mock response" } }),
    });

    let mut response_headers = HeaderMap::new();
    if let Some(retry_after) = response.retry_after_header {
        response_headers.insert(
            reqwest::header::RETRY_AFTER,
            retry_after
                .parse()
                .expect("retry-after header should parse"),
        );
    }

    (response.status, response_headers, Json(response.body)).into_response()
}
