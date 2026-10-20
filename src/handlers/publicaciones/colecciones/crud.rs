use super::*;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateColeccionRequest {
    pub nombre: String,
    pub descripcion: Option<String>,
    #[serde(default = "default_true")]
    pub publica: bool,
    pub parent_id: Option<i64>,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(clippy::option_option)] // serde necesita Option<Option<T>> para distinguir ausente vs null
pub struct UpdateColeccionRequest {
    pub nombre: Option<String>,
    /* Triple-state: ausente = no cambiar; null = poner NULL; valor = setear. */
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub descripcion: Option<Option<String>>,
    pub publica: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub imagen_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub parent_id: Option<Option<i64>>,
    /* [18AA-2] Bloqueo optimista opcional: la `version` que el cliente leyó.
     * Si se omite, el PUT pisa sin comprobar (compatible con clientes viejos);
     * si se envía y no coincide con la fila, responde 409. */
    pub version: Option<i32>,
}

/* Helper para distinguir "campo ausente" vs "campo presente con null".
 * Sin esto, serde colapsa ambos a None y no podemos hacer "set to null". */
#[allow(clippy::option_option)]
fn deserialize_optional_field<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSampleRequest {
    #[serde(alias = "sampleId")]
    pub sample_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ColeccionListResponse {
    pub items: Vec<Coleccion>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ColeccionSamplesResponse {
    pub items: Vec<ColeccionSample>,
}

#[utoipa::path(
    post, path = "/api/colecciones", tag = "colecciones",
    request_body = CreateColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, body = Coleccion),
        (status = 400, description = "Datos inválidos"),
        (status = 409, description = "Nombre duplicado en la misma ubicación"),
    )
)]
pub async fn create_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateColeccionRequest>,
) -> Result<(StatusCode, Json<Coleccion>), AppError> {
    let nombre = body.nombre.trim();
    if nombre.is_empty() || nombre.len() > 200 {
        return Err(AppError::BadRequest(
            "nombre requerido (1..200 chars)".into(),
        ));
    }
    if let Some(d) = &body.descripcion {
        if d.len() > 2000 {
            return Err(AppError::BadRequest(
                "descripcion demasiado larga (>2000)".into(),
            ));
        }
    }
    let col = ColeccionesRepository::create(
        &state.pool,
        user.user_id,
        nombre,
        body.descripcion.as_deref(),
        body.publica,
        body.parent_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(col)))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/colecciones", tag = "colecciones",
    security(("bearer_auth" = [])),
    responses((status = 200, body = ColeccionListResponse))
)]
pub async fn list_my_colecciones(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ColeccionListResponse>, AppError> {
    let items = ColeccionesRepository::list_by_user(&state.pool, user.user_id).await?;
    Ok(Json(ColeccionListResponse { items }))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = Coleccion),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn get_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<Coleccion>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    Ok(Json(col))
}

#[utoipa::path(
    put, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    request_body = UpdateColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "No encontrada"),
        (status = 409, description = "Nombre duplicado o version desactualizada"),
    )
)]
pub async fn update_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateColeccionRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    if let Some(n) = &body.nombre {
        let n = n.trim();
        if n.is_empty() || n.len() > 200 {
            return Err(AppError::BadRequest("nombre invalido".into()));
        }
    }
    /* parent_id propio = circular */
    if let Some(Some(pid)) = body.parent_id {
        if pid == id {
            return Err(AppError::BadRequest(
                "una coleccion no puede ser su propio padre".into(),
            ));
        }
        ColeccionesRepository::check_parent_valid(&state.pool, user.user_id, pid).await?;
    }
    let nombre = body.nombre.as_deref().map(str::trim);
    let descripcion = body.descripcion.as_ref().map(|o| o.as_deref());
    let imagen_url = body.imagen_url.as_ref().map(|o| o.as_deref());
    let cambios = CambiosColeccion {
        nombre,
        descripcion,
        publica: body.publica,
        imagen_url,
        parent_id: body.parent_id,
    };
    let ok = ColeccionesRepository::update(&state.pool, id, cambios, body.version).await?;
    if !ok {
        /* [18AA-2] Sin filas puede ser que no exista o que la version no
         * cuadre. Se distingue con un fetch: solo si sigue viva hubo choque. */
        if ColeccionesRepository::fetch(&state.pool, id).await?.is_some() {
            return Err(AppError::Conflict(
                "la coleccion cambio desde que la leiste; recarga y reintenta".into(),
            ));
        }
        return Err(AppError::NotFound(format!("coleccion {id} no encontrada")));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn delete_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    let ok = ColeccionesRepository::soft_delete(&state.pool, id).await?;
    if !ok {
        return Err(AppError::NotFound(format!("coleccion {id} no encontrada")));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    post, path = "/api/colecciones/{id}/samples", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    request_body = AddSampleRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "Coleccion no encontrada"),
    )
)]
pub async fn add_sample(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(coleccion_id): Path<i64>,
    Json(body): Json<AddSampleRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, coleccion_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    if body.sample_id <= 0 {
        return Err(AppError::BadRequest("sample_id requerido".into()));
    }
    let _op = ColeccionesRepository::add_sample(
        &state.pool,
        coleccion_id,
        body.sample_id,
        user.user_id,
    )
    .await?;
    /* [296A-1] Legacy: 1 sample = 1 coleccion. Si ya existia en otra, se mueve. */
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}/samples/{sample_id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion"), ("sample_id" = i32, Path, description = "ID del sample")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
    )
)]
pub async fn remove_sample(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((coleccion_id, sample_id)): Path<(i64, i32)>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, coleccion_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    let _removed =
        ColeccionesRepository::remove_sample(&state.pool, coleccion_id, sample_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    get, path = "/api/colecciones/{id}/samples", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = ColeccionSamplesResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn list_samples(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ColeccionSamplesResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    let items = ColeccionesRepository::list_samples(&state.pool, id).await?;
    Ok(Json(ColeccionSamplesResponse { items }))
}
