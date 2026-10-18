use super::*;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreatePostRequest {
    #[serde(default)]
    pub contenido: String,
    #[serde(default)]
    pub imagenes: Vec<String>,
    #[serde(default, alias = "samplesAdjuntos")]
    pub samples_adjuntos: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdatePostRequest {
    #[serde(default)]
    pub contenido: String,
    #[serde(default)]
    pub imagenes: Vec<String>,
    #[serde(default, alias = "samplesAdjuntos")]
    pub samples_adjuntos: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PostListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default = "default_filter")]
    pub filtro: String,
    pub author_id: Option<i32>,
    /* [254A-6] El frontend legacy (apiSocial.listarPublicacionesUsuario) envia
     * `?autor=<username>` en vez de `?author_id=<int>`. Aceptamos ambos para no
     * acoplar el backend al wpJsonStub: si llega autor, lo resolvemos al id real. */
    pub autor: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostListResponse {
    pub items: Vec<PostDetail>,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostMutationResponse {
    pub ok: bool,
    pub post: PostDetail,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RepostResponse {
    pub ok: bool,
    pub already_exists: bool,
    pub post: PostDetail,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}


fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}
fn default_filter() -> String {
    "todos".to_string()
}
