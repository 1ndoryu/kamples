use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;
use super::entidades::{
    MusicArtistRole, RelationSampleSide, SampleRelationElementType, SampleRelationSource,
    SampleRelationType,
};

#[derive(Debug, Clone, Deserialize, IntoParams, Validate, ToSchema)]
pub struct ListSongsQuery {
    #[serde(default)]
    #[validate(range(min = 1, max = 10_000))]
    pub page: Option<i64>,
    #[serde(default)]
    #[validate(range(min = 1, max = 100))]
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, Validate, ToSchema)]
pub struct SearchSongsQuery {
    #[validate(length(min = 1, max = 120))]
    pub q: String,
    #[serde(default, alias = "limit")]
    #[validate(range(min = 1, max = 100))]
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, Validate, ToSchema)]
pub struct LimitQuery {
    #[serde(default)]
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, Validate, ToSchema)]
pub struct RelationChainQuery {
    #[serde(default)]
    #[validate(range(min = 1, max = 10))]
    pub profundidad: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateArtistRequest {
    #[validate(length(min = 1, max = 300))]
    pub nombre: String,
    #[validate(length(min = 1, max = 350))]
    pub slug: Option<String>,
    #[validate(length(max = 2_000))]
    pub imagen_url: Option<String>,
    #[validate(length(max = 350))]
    pub whosampled_slug: Option<String>,
    #[validate(length(max = 36))]
    pub musicbrainz_id: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: Option<serde_json::Value>,
    #[validate(range(min = 0, max = 999))]
    pub prioridad: Option<i16>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateArtistRequest {
    #[validate(length(min = 1, max = 300))]
    pub nombre: Option<String>,
    #[validate(length(min = 1, max = 350))]
    pub slug: Option<String>,
    #[validate(length(max = 2_000))]
    pub imagen_url: Option<String>,
    #[validate(length(max = 350))]
    pub whosampled_slug: Option<String>,
    #[validate(length(max = 36))]
    pub musicbrainz_id: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: Option<serde_json::Value>,
    #[validate(range(min = 0, max = 999))]
    pub prioridad: Option<i16>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct SongArtistInput {
    #[validate(range(min = 1))]
    pub artista_id: i32,
    pub rol: MusicArtistRole,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateSongRequest {
    #[validate(length(min = 1, max = 500))]
    pub titulo: String,
    #[validate(length(min = 1, max = 550))]
    pub slug: Option<String>,
    #[validate(range(min = 1))]
    pub artista_id: i32,
    #[validate(length(max = 500))]
    pub album: Option<String>,
    #[validate(length(max = 200))]
    pub sello: Option<String>,
    #[validate(range(min = 0, max = 3000))]
    pub anio: Option<i32>,
    #[validate(range(min = 0, max = 7200))]
    pub duracion_segundos: Option<i32>,
    #[validate(length(max = 100))]
    pub genero: Option<String>,
    #[validate(length(max = 20))]
    pub youtube_id: Option<String>,
    #[validate(length(max = 30))]
    pub spotify_id: Option<String>,
    #[validate(length(max = 2_000))]
    pub imagen_url: Option<String>,
    #[validate(length(max = 500))]
    pub whosampled_url: Option<String>,
    #[validate(range(min = 0, max = 300))]
    pub bpm: Option<i32>,
    #[validate(length(max = 5))]
    pub tonalidad: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub artistas: Vec<SongArtistInput>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateSongRequest {
    #[validate(length(min = 1, max = 500))]
    pub titulo: Option<String>,
    #[validate(length(min = 1, max = 550))]
    pub slug: Option<String>,
    #[validate(range(min = 1))]
    pub artista_id: Option<i32>,
    #[validate(length(max = 500))]
    pub album: Option<String>,
    #[validate(length(max = 200))]
    pub sello: Option<String>,
    #[validate(range(min = 0, max = 3000))]
    pub anio: Option<i32>,
    #[validate(range(min = 0, max = 7200))]
    pub duracion_segundos: Option<i32>,
    #[validate(length(max = 100))]
    pub genero: Option<String>,
    #[validate(length(max = 20))]
    pub youtube_id: Option<String>,
    #[validate(length(max = 30))]
    pub spotify_id: Option<String>,
    #[validate(length(max = 2_000))]
    pub imagen_url: Option<String>,
    #[validate(length(max = 500))]
    pub whosampled_url: Option<String>,
    #[validate(range(min = 0, max = 300))]
    pub bpm: Option<i32>,
    #[validate(length(max = 5))]
    pub tonalidad: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: Option<serde_json::Value>,
    pub artistas: Option<Vec<SongArtistInput>>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateRelationRequest {
    #[validate(range(min = 1))]
    pub cancion_destino_id: i32,
    #[validate(range(min = 1))]
    pub cancion_fuente_id: i32,
    pub whosampled_id: Option<i32>,
    pub tipo_relacion: SampleRelationType,
    pub tipo_elemento: Option<SampleRelationElementType>,
    #[serde(default)]
    pub timings_destino: Vec<i32>,
    #[serde(default)]
    pub timings_fuente: Vec<i32>,
    pub aparece_en_todo: Option<bool>,
    pub sample_id: Option<i32>,
    pub sample_fuente_id: Option<i32>,
    pub sample_destino_id: Option<i32>,
    pub votos_total: Option<i32>,
    pub votos_promedio: Option<f64>,
    pub fuente: Option<SampleRelationSource>,
    pub contribuidor_id: Option<i32>,
    pub verificada: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateRelationRequest {
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub whosampled_id: Option<i32>,
    pub tipo_relacion: Option<SampleRelationType>,
    pub tipo_elemento: Option<SampleRelationElementType>,
    pub timings_destino: Option<Vec<i32>>,
    pub timings_fuente: Option<Vec<i32>>,
    pub aparece_en_todo: Option<bool>,
    pub sample_id: Option<i32>,
    pub sample_fuente_id: Option<i32>,
    pub sample_destino_id: Option<i32>,
    pub votos_total: Option<i32>,
    pub votos_promedio: Option<f64>,
    pub fuente: Option<SampleRelationSource>,
    pub contribuidor_id: Option<i32>,
    pub verificada: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct SampleLinkRequest {
    #[validate(range(min = 1))]
    pub sample_id: i32,
    pub lado: RelationSampleSide,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct VerifyRelationRequest {
    pub verificada: bool,
}
