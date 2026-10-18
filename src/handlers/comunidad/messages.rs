mod payload;

use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::CurrentUser;
use crate::repositories::{
    ConversationMessage, ConversationRepository, ConversationSummary, CreateMessageParams,
    DirectMessageKind, MessageRepository, ModerationRepository, ProfileRepository,
};
use crate::services::NotificationFanoutService;
use crate::AppState;
use tracing::warn;

use self::payload::{build_message_storage_key, parse_create_message_request};

const MAX_MESSAGE_CHARS: usize = 5_000;
const MAX_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_IMAGE_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
const MAX_AUDIO_UPLOAD_BYTES: usize = 30 * 1024 * 1024;
const MAX_MESSAGE_MULTIPART_BODY_BYTES: usize = MAX_AUDIO_UPLOAD_BYTES + (4 * 1024 * 1024);
const DEFAULT_MESSAGES_LIMIT: i64 = 50;
const MAX_MESSAGES_LIMIT: i64 = 100;
const SPAM_TERMS: [&str; 9] = [
    "buy now",
    "free money",
    "crypto signal",
    "dm for promo",
    "telegram",
    "whatsapp",
    "airdrop",
    "100% profit",
    "cashapp",
];

/* [174A-71/174A-72/174A-74] Mensajería directa: conversaciones, listado
 * paginado, envío texto/media/sample y marcación de leídos. El corte actual ya
 * emite el evento websocket legado `mensaje_nuevo`; la persistencia base de
 * notificaciones vive en 174A-74 y el pipeline emisor unificado queda para 174A-78. */

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct MessageListQuery {
    #[serde(alias = "antes_de_id")]
    pub before_id: Option<i32>,
    #[serde(default = "default_messages_limit")]
    pub limit: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateMessageJsonRequest {
    #[serde(default)]
    pub contenido: String,
    pub tipo: Option<String>,
    #[serde(alias = "sampleId")]
    pub sample_id: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateMessageMultipartRequestDoc {
    #[serde(default)]
    #[schema(required = false, value_type = String, format = Binary)]
    pub media: Option<String>,
    pub contenido: Option<String>,
    pub tipo: Option<String>,
    pub sample_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StartConversationRequest {
    #[serde(alias = "usuarioId", alias = "userId")]
    pub usuario_id: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationListResponse {
    pub data: Vec<ConversationSummary>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MessageListResponse {
    pub data: Vec<ConversationMessage>,
    pub total: i64,
    pub hay_mas: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationMutationResponse {
    pub data: ConversationSummary,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MessageMutationResponse {
    pub data: ConversationMessage,
}

#[utoipa::path(
    get,
    path = "/api/mensajes/conversaciones",
    tag = "messages",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Lista de conversaciones del usuario", body = ConversationListResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn list_conversations(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ConversationListResponse>, AppError> {
    let hidden_ids = collect_hidden_user_ids(&state, user.user_id).await?;
    let conversations =
        ConversationRepository::list_for_user(&state.pool, user.user_id, &hidden_ids)
            .await?
            .into_iter()
            .map(|conversation| {
                normalize_conversation_summary(conversation, state.public_base_url.as_deref())
            })
            .collect();
    Ok(Json(ConversationListResponse {
        data: conversations,
    }))
}

#[utoipa::path(
    get,
    path = "/api/mensajes/{conversacionId}",
    tag = "messages",
    params(
        ("conversacionId" = i32, Path, description = "ID de la conversación"),
        MessageListQuery
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Mensajes de la conversación", body = MessageListResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Conversación no disponible", body = ErrorResponse),
        (status = 404, description = "Conversación no encontrada", body = ErrorResponse)
    )
)]
pub async fn list_messages(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(conversacion_id): Path<i32>,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<MessageListResponse>, AppError> {
    let other_id = require_conversation_access(&state, user.user_id, conversacion_id).await?;
    ensure_not_blocked_pair(&state, user.user_id, other_id).await?;

    let limit = query.limit.clamp(1, MAX_MESSAGES_LIMIT);
    let messages = MessageRepository::list_by_conversation(
        &state.pool,
        conversacion_id,
        limit,
        query.before_id,
    )
    .await?
    .into_iter()
    .map(|message| normalize_message(message, state.public_base_url.as_deref()))
    .collect::<Vec<_>>();
    let total = MessageRepository::count_by_conversation(&state.pool, conversacion_id).await?;
    let hay_mas = match messages.first() {
        Some(message) => {
            MessageRepository::has_previous(&state.pool, conversacion_id, message.id).await?
        }
        None => false,
    };

    Ok(Json(MessageListResponse {
        data: messages,
        total,
        hay_mas,
    }))
}

#[utoipa::path(
    post,
    path = "/api/mensajes/{conversacionId}",
    tag = "messages",
    request_body(
        content = CreateMessageMultipartRequestDoc,
        content_type = "multipart/form-data",
        description = "Acepta multipart con media o JSON equivalente con { contenido, tipo, sample_id }"
    ),
    params(("conversacionId" = i32, Path, description = "ID de la conversación")),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Mensaje creado", body = MessageMutationResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "No puedes enviar mensajes a este usuario", body = ErrorResponse),
        (status = 404, description = "Conversación o sample no encontrado", body = ErrorResponse),
        (status = 413, description = "Media demasiado grande", body = ErrorResponse),
        (status = 415, description = "Tipo de media no soportado", body = ErrorResponse),
        (status = 422, description = "Validación del mensaje", body = ErrorResponse)
    )
)]
pub async fn send_message(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(conversacion_id): Path<i32>,
    request: Request,
) -> Result<(StatusCode, Json<MessageMutationResponse>), AppError> {
    ensure_active_profile(&state, user.user_id, "enviar mensajes").await?;
    let other_id = require_conversation_access(&state, user.user_id, conversacion_id).await?;
    ensure_not_blocked_pair(&state, user.user_id, other_id).await?;

    let parsed = parse_create_message_request(request, &state).await?;
    if parsed.kind == DirectMessageKind::Texto {
        ensure_not_spam(&parsed.contenido)?;
    }

    let mut contenido = parsed.contenido;
    let mut stored_media_key = None;
    let mut media_url = None;
    let mut media_metadata = None;

    match parsed.kind {
        DirectMessageKind::Texto => {}
        DirectMessageKind::Imagen | DirectMessageKind::Audio => {
            let media = parsed
                .media
                .ok_or_else(|| AppError::Validation("la media adjunta es obligatoria".into()))?;
            let storage_key = build_message_storage_key(user.user_id, &media.extension);
            state.storage.put_bytes(&storage_key, &media.bytes).await?;
            media_metadata = Some(serde_json::json!({
                "content_type": media.content_type,
                "size_bytes": media.bytes.len(),
                "original_filename": media.original_filename,
                "extension": media.extension,
                "media_kind": media.kind.as_db_str(),
            }));
            media_url = Some(storage_key.clone());
            stored_media_key = Some(storage_key);
        }
        DirectMessageKind::Sample => {
            let sample_id = parsed
                .sample_id
                .ok_or_else(|| AppError::Validation("sample_id es obligatorio".into()))?;
            let sample = MessageRepository::find_sample_for_share(&state.pool, sample_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("sample {sample_id} no existe o no está activo"))
                })?;
            if contenido.is_empty() {
                contenido.clone_from(&sample.titulo);
            }
            media_metadata = Some(serde_json::json!({
                "sample_id": sample.id,
                "titulo": sample.titulo,
                "id_corto": sample.id_corto,
                "slug": sample.slug,
                "tipo": sample.tipo,
                "bpm": sample.bpm,
                "key": sample.music_key,
            }));
        }
    }

    let message_id = match MessageRepository::create(
        &state.pool,
        CreateMessageParams {
            conversacion_id,
            autor_id: user.user_id,
            contenido: &contenido,
            tipo: parsed.kind,
            media_url: media_url.as_deref(),
            media_metadata,
        },
    )
    .await
    {
        Ok(message_id) => message_id,
        Err(error) => {
            if let Some(key) = stored_media_key.as_deref() {
                state.storage.delete(key).await?;
            }
            return Err(error);
        }
    };

    let message = MessageRepository::get(&state.pool, message_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("mensaje {message_id} no existe")))?;
    let message = normalize_message(message, state.public_base_url.as_deref());

    if let Err(error) =
        NotificationFanoutService::dispatch_new_message(&state, other_id, user.user_id, &message)
            .await
    {
        warn!(
            sender_id = user.user_id,
            recipient_id = other_id,
            conversation_id = conversacion_id,
            error = %error,
            "falló fanout de mensaje directo"
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(MessageMutationResponse { data: message }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/mensajes/{conversacionId}/leer",
    tag = "messages",
    params(("conversacionId" = i32, Path, description = "ID de la conversación")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Mensajes marcados como leídos", body = OkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Conversación no disponible", body = ErrorResponse),
        (status = 404, description = "Conversación no encontrada", body = ErrorResponse)
    )
)]
pub async fn mark_read(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(conversacion_id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    let other_id = require_conversation_access(&state, user.user_id, conversacion_id).await?;
    ensure_not_blocked_pair(&state, user.user_id, other_id).await?;
    let _ = MessageRepository::mark_read(&state.pool, conversacion_id, user.user_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/mensajes/leer-todas",
    tag = "messages",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Todas las conversaciones marcadas como leídas", body = OkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn mark_all_read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<OkResponse>, AppError> {
    let _ = MessageRepository::mark_all_read_for_user(&state.pool, user.user_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/mensajes/nueva",
    tag = "messages",
    request_body = StartConversationRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "La conversación ya existía", body = ConversationMutationResponse),
        (status = 201, description = "Conversación creada", body = ConversationMutationResponse),
        (status = 400, description = "usuario_id inválido o self-chat", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "No puedes iniciar conversación con este usuario", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn start_conversation(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<StartConversationRequest>,
) -> Result<(StatusCode, Json<ConversationMutationResponse>), AppError> {
    ensure_active_profile(&state, user.user_id, "iniciar conversaciones").await?;
    if body.usuario_id <= 0 {
        return Err(AppError::Validation("usuario_id inválido".into()));
    }
    if body.usuario_id == user.user_id {
        return Err(AppError::Validation(
            "no puedes chatear contigo mismo".into(),
        ));
    }

    ensure_active_profile(&state, body.usuario_id, "recibir mensajes").await?;
    ensure_not_blocked_pair(&state, user.user_id, body.usuario_id).await?;

    let hidden_ids = collect_hidden_user_ids(&state, user.user_id).await?;
    if let Some(existing_id) =
        ConversationRepository::find_between_users(&state.pool, user.user_id, body.usuario_id)
            .await?
    {
        let conversation = ConversationRepository::get_for_user(
            &state.pool,
            user.user_id,
            existing_id,
            &hidden_ids,
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("conversación {existing_id} no existe")))?;
        return Ok((
            StatusCode::OK,
            Json(ConversationMutationResponse {
                data: normalize_conversation_summary(
                    conversation,
                    state.public_base_url.as_deref(),
                ),
            }),
        ));
    }

    let conversation_id =
        ConversationRepository::create(&state.pool, user.user_id, body.usuario_id).await?;
    let conversation = ConversationRepository::get_for_user(
        &state.pool,
        user.user_id,
        conversation_id,
        &hidden_ids,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("conversación {conversation_id} no existe")))?;

    Ok((
        StatusCode::CREATED,
        Json(ConversationMutationResponse {
            data: normalize_conversation_summary(conversation, state.public_base_url.as_deref()),
        }),
    ))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mensajes/conversaciones", get(list_conversations))
        .route("/mensajes/nueva", post(start_conversation))
        .route("/mensajes/leer-todas", post(mark_all_read))
        .route(
            "/mensajes/:conversacion_id",
            get(list_messages)
                .post(send_message)
                .layer(DefaultBodyLimit::max(MAX_MESSAGE_MULTIPART_BODY_BYTES)),
        )
        .route("/mensajes/:conversacion_id/leer", post(mark_read))
}

async fn require_conversation_access(
    state: &AppState,
    user_id: i32,
    conversation_id: i32,
) -> Result<i32, AppError> {
    if !ConversationRepository::verify_participation(&state.pool, conversation_id, user_id).await? {
        return Err(AppError::NotFound(format!(
            "conversación {conversation_id} no existe"
        )));
    }

    ConversationRepository::other_participant_id(&state.pool, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("conversación {conversation_id} no existe")))
}

async fn collect_hidden_user_ids(state: &AppState, user_id: i32) -> Result<Vec<i32>, AppError> {
    let mut ids = ModerationRepository::list_blocked(&state.pool, user_id).await?;
    ids.extend(ModerationRepository::list_blockers(&state.pool, user_id).await?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

async fn ensure_not_blocked_pair(
    state: &AppState,
    user_id: i32,
    other_id: i32,
) -> Result<(), AppError> {
    let hidden_ids = collect_hidden_user_ids(state, user_id).await?;
    if hidden_ids.binary_search(&other_id).is_ok() {
        return Err(AppError::Forbidden(
            "no puedes enviar mensajes a este usuario".into(),
        ));
    }
    Ok(())
}

async fn ensure_active_profile(
    state: &AppState,
    user_id: i32,
    action: &str,
) -> Result<(), AppError> {
    let profile = ProfileRepository::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("usuario {user_id} no existe")))?;
    if profile.estado != "activo" {
        return Err(AppError::Forbidden(format!(
            "la cuenta no está activa para {action}"
        )));
    }
    Ok(())
}

fn ensure_not_spam(content: &str) -> Result<(), AppError> {
    let normalized = content
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let url_count: usize = ["http://", "https://", "www.", "t.me/", "telegram.me/"]
        .iter()
        .map(|needle| normalized.matches(needle).count())
        .sum();
    if url_count >= 2 || SPAM_TERMS.iter().any(|needle| normalized.contains(needle)) {
        return Err(AppError::Validation(
            "el mensaje fue bloqueado por patrón de spam".into(),
        ));
    }
    Ok(())
}

fn normalize_conversation_summary(
    mut conversation: ConversationSummary,
    public_base_url: Option<&str>,
) -> ConversationSummary {
    conversation.participante.avatar_url =
        asset_to_public_url(public_base_url, conversation.participante.avatar_url);
    conversation
}

fn normalize_message(
    mut message: ConversationMessage,
    public_base_url: Option<&str>,
) -> ConversationMessage {
    message.media_url = asset_to_public_url(public_base_url, message.media_url);
    message
}

fn asset_to_public_url(public_base_url: Option<&str>, raw: Option<String>) -> Option<String> {
    let raw = raw?.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw);
    }

    let path = if raw.starts_with('/') {
        raw
    } else {
        format!("/uploads/{raw}")
    };

    Some(match public_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path),
        None => path,
    })
}

const fn default_messages_limit() -> i64 {
    DEFAULT_MESSAGES_LIMIT
}
