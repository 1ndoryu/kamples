use super::*;

/* [174A-68] Comentarios polimórficos con replies, likes y media sobre el mismo schema social.
 * Se soporta creación JSON o multipart en un único endpoint para mantener paridad con desktop/legacy.
 * Gotcha: el cleanup de media contempla todo el subárbol porque delete hace cascade en respuestas. */

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CommentListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CommentRepliesQuery {
    #[serde(default = "default_replies_limit")]
    pub limit: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCommentJsonRequest {
    #[serde(default)]
    pub contenido: String,
    pub parent_id: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCommentMultipartRequestDoc {
    #[schema(value_type = String, format = Binary)]
    pub media: Option<Vec<u8>>,
    pub contenido: Option<String>,
    pub parent_id: Option<i32>,
    pub tipo_contenido: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateCommentRequest {
    #[serde(default)]
    pub contenido: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CommentLikeRequest {
    #[serde(default)]
    pub reaccion: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentListResponse {
    pub items: Vec<CommentDetail>,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentRepliesResponse {
    pub items: Vec<CommentDetail>,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentMutationResponse {
    pub ok: bool,
    pub comment: CommentDetail,
}

const fn default_page() -> i64 {
    DEFAULT_PAGE
}

const fn default_per_page() -> i64 {
    DEFAULT_PER_PAGE
}

const fn default_replies_limit() -> i64 {
    DEFAULT_REPLIES_LIMIT
}
