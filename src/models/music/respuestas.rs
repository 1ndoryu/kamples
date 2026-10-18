use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use super::entidades::{
    MusicArtist, MusicSong, RelationSampleSide, SampleRelationElementType, SampleRelationSource,
    SampleRelationType, SongArtistLink,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SampleRelationSummary {
    pub id: i32,
    pub cancion_destino_id: i32,
    pub cancion_fuente_id: i32,
    pub whosampled_id: Option<i32>,
    pub tipo_relacion: SampleRelationType,
    pub tipo_elemento: Option<SampleRelationElementType>,
    pub timings_destino: Vec<i32>,
    pub timings_fuente: Vec<i32>,
    pub aparece_en_todo: bool,
    pub sample_id: Option<i32>,
    pub sample_fuente_id: Option<i32>,
    pub sample_destino_id: Option<i32>,
    pub votos_total: i32,
    pub votos_promedio: f64,
    pub fuente: SampleRelationSource,
    pub contribuidor_id: Option<i32>,
    pub contribuidor_username: Option<String>,
    pub verificada: bool,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cancion_titulo: String,
    pub cancion_slug: String,
    pub artista_nombre: String,
    pub artista_slug: String,
    pub cancion_anio: Option<i16>,
    pub cancion_imagen_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SampleRelationDetail {
    pub id: i32,
    pub cancion_destino_id: i32,
    pub cancion_fuente_id: i32,
    pub whosampled_id: Option<i32>,
    pub tipo_relacion: SampleRelationType,
    pub tipo_elemento: Option<SampleRelationElementType>,
    pub timings_destino: Vec<i32>,
    pub timings_fuente: Vec<i32>,
    pub aparece_en_todo: bool,
    pub sample_id: Option<i32>,
    pub sample_fuente_id: Option<i32>,
    pub sample_destino_id: Option<i32>,
    pub votos_total: i32,
    pub votos_promedio: f64,
    pub fuente: SampleRelationSource,
    pub contribuidor_id: Option<i32>,
    pub contribuidor_username: Option<String>,
    pub verificada: bool,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub total_samples: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub fuente_titulo: String,
    pub fuente_slug: String,
    pub fuente_anio: Option<i16>,
    pub fuente_imagen_url: Option<String>,
    pub fuente_youtube_id: Option<String>,
    pub fuente_spotify_id: Option<String>,
    pub fuente_album: Option<String>,
    pub fuente_genero: Option<String>,
    pub fuente_artista: String,
    pub fuente_artista_slug: String,
    pub destino_titulo: String,
    pub destino_slug: String,
    pub destino_anio: Option<i16>,
    pub destino_imagen_url: Option<String>,
    pub destino_youtube_id: Option<String>,
    pub destino_spotify_id: Option<String>,
    pub destino_album: Option<String>,
    pub destino_genero: Option<String>,
    pub destino_artista: String,
    pub destino_artista_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destino_samples_de: Option<Vec<SampleRelationSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destino_sampleada_en: Option<Vec<SampleRelationSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuente_samples_de: Option<Vec<SampleRelationSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuente_sampleada_en: Option<Vec<SampleRelationSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lado_extraccion: Option<RelationSampleSide>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicPagination {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicSongsResponse {
    pub data: Vec<MusicSong>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SongListResponse {
    pub data: Vec<MusicSong>,
    pub pagination: MusicPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicArtistsResponse {
    pub data: Vec<MusicArtist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SongDetailResponse {
    pub cancion: MusicSong,
    pub artistas: Vec<SongArtistLink>,
    pub samples_de: Vec<SampleRelationSummary>,
    pub sampleada_en: Vec<SampleRelationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArtistStats {
    pub total_sampleado_por: usize,
    pub total_samplea_a: usize,
    pub generos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArtistDetailResponse {
    pub artista: MusicArtist,
    pub canciones: Vec<MusicSong>,
    pub sampleado_por: Vec<SampleRelationSummary>,
    pub samplea_a: Vec<SampleRelationSummary>,
    pub estadisticas: ArtistStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationTypeCount {
    pub tipo_relacion: SampleRelationType,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationStatsResponse {
    pub relaciones_por_tipo: Vec<RelationTypeCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SampleRelationLookupResponse {
    pub data: Option<SampleRelationDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationChainNode {
    pub id: i32,
    pub cancion_fuente_id: i32,
    pub cancion_destino_id: i32,
    pub tipo_relacion: SampleRelationType,
    pub nivel: i32,
    pub fuente_titulo: String,
    pub fuente_slug: String,
    pub fuente_artista: String,
    pub destino_titulo: String,
    pub destino_slug: String,
    pub destino_artista: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationChainResponse {
    pub cancion_raiz: MusicSong,
    pub cadena: Vec<RelationChainNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicMutationResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationVerificationResponse {
    pub ok: bool,
    pub verificada: bool,
}
