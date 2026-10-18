use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MusicArtistRole {
    Principal,
    Featuring,
    Producer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SampleRelationType {
    Sample,
    Cover,
    Remix,
    Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SampleRelationElementType {
    HookRiff,
    VocalsLyrics,
    Drums,
    Bass,
    KeysSynth,
    SoundEffect,
    MultipleElements,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SampleRelationSource {
    Scraping,
    Comunidad,
    Musicbrainz,
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RelationSampleSide {
    Fuente,
    Destino,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MusicArtist {
    pub id: i32,
    pub nombre: String,
    pub slug: String,
    pub imagen_url: Option<String>,
    pub whosampled_slug: Option<String>,
    pub musicbrainz_id: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub prioridad: i16,
    pub total_canciones: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct MusicSong {
    pub id: i32,
    pub titulo: String,
    pub slug: String,
    pub artista_id: i32,
    pub album: Option<String>,
    pub sello: Option<String>,
    pub anio: Option<i16>,
    pub duracion_segundos: Option<i16>,
    pub genero: Option<String>,
    pub youtube_id: Option<String>,
    pub spotify_id: Option<String>,
    pub imagen_url: Option<String>,
    pub whosampled_url: Option<String>,
    pub bpm: Option<i16>,
    pub tonalidad: Option<String>,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
    pub total_sampleada: i32,
    pub total_samplea: i32,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub artista_nombre: String,
    pub artista_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SongArtistLink {
    pub artista_id: i32,
    pub nombre: String,
    pub slug: String,
    pub rol: MusicArtistRole,
}
