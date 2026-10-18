use sqlx::{FromRow, PgPool};

use crate::errors::AppError;

pub struct SearchRepository;

#[derive(Debug, Clone, FromRow)]
pub struct SearchSongRecord {
    pub id: i32,
    pub titulo: String,
    pub slug: String,
    pub artista_nombre: String,
    pub imagen_url: Option<String>,
    pub total_sampleada: i32,
}

#[derive(Debug, Clone, FromRow)]
pub struct SearchSampleRecord {
    pub id: i32,
    pub titulo: String,
    pub slug: String,
    pub imagen_url: Option<String>,
    pub creator_id: i32,
    pub creator_username: String,
    pub creator_nombre_visible: Option<String>,
    pub creator_avatar_url: Option<String>,
    pub creator_verificado: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct SearchUserRecord {
    pub id: i32,
    pub username: String,
    pub nombre_visible: String,
    pub avatar_url: Option<String>,
    pub verificado: bool,
    pub total_seguidores: i32,
}

#[derive(Debug, Clone, FromRow)]
pub struct SearchCollectionRecord {
    pub id: i64,
    pub nombre: String,
    pub slug: Option<String>,
    pub portada_url: Option<String>,
    pub total_samples: i32,
    pub creator_username: String,
    pub creator_nombre_visible: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SearchSampleRelationRecord {
    pub id: i32,
    pub fuente_titulo: String,
    pub fuente_slug: String,
    pub fuente_imagen_url: Option<String>,
    pub fuente_artista: String,
    pub destino_titulo: String,
    pub destino_slug: String,
    pub destino_imagen_url: Option<String>,
    pub destino_artista: String,
}

impl SearchRepository {
    pub async fn search_songs(
        pool: &PgPool,
        query: &str,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<SearchSongRecord>, AppError> {
        let rows = sqlx::query_as!(
            SearchSongRecord,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      a.nombre AS artista_nombre,
                      c.imagen_url,
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               WHERE to_tsvector('simple',
                       COALESCE(c.titulo, '') || ' ' || COALESCE(a.nombre, '') || ' ' || COALESCE(c.album, '')
                     ) @@ plainto_tsquery('simple', $1)
                  OR c.titulo ILIKE $2
                  OR a.nombre ILIKE $2
               ORDER BY COALESCE(c.total_sampleada, 0) DESC, c.id DESC
               LIMIT $3"#,
            query,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn search_samples(
        pool: &PgPool,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<SearchSampleRecord>, AppError> {
        let rows = sqlx::query_as!(
            SearchSampleRecord,
            r#"SELECT s.id,
                      s.titulo,
                      s.slug,
                      s.imagen_url,
                      u.id AS creator_id,
                      u.username AS creator_username,
                      NULLIF(u.nombre_visible, '') AS creator_nombre_visible,
                      u.avatar_url AS creator_avatar_url,
                      COALESCE(u.verificado, FALSE) AS "creator_verificado!"
               FROM samples s
               INNER JOIN usuarios_ext u ON u.id = s.creador_id
               WHERE s.eliminado_en IS NULL
                 AND s.estado = 'activo'
                 AND s.titulo ILIKE $1
               ORDER BY COALESCE(s.total_likes, 0) DESC, s.id DESC
               LIMIT $2"#,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn search_users(
        pool: &PgPool,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<SearchUserRecord>, AppError> {
        let rows = sqlx::query_as!(
            SearchUserRecord,
            r#"SELECT id,
                      username,
                      nombre_visible,
                      avatar_url,
                      COALESCE(verificado, FALSE) AS "verificado!",
                      COALESCE(total_seguidores, 0) AS "total_seguidores!"
               FROM usuarios_ext
               WHERE estado = 'activo'
                 AND es_seed = FALSE
                 AND (username ILIKE $1 OR nombre_visible ILIKE $1)
               ORDER BY COALESCE(total_seguidores, 0) DESC, id DESC
               LIMIT $2"#,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn search_collections(
        pool: &PgPool,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<SearchCollectionRecord>, AppError> {
        let rows = sqlx::query_as!(
            SearchCollectionRecord,
            r#"SELECT c.id::bigint AS id,
                      c.nombre,
                      NULL::text AS slug,
                      c.imagen_url AS portada_url,
                      COALESCE(c.total_samples, 0) AS "total_samples!",
                      u.username AS creator_username,
                      u.nombre_visible AS creator_nombre_visible
               FROM colecciones c
               INNER JOIN usuarios_ext u ON u.id = c.usuario_id
               WHERE c.publica = TRUE
                 AND COALESCE(c.total_samples, 0) > 0
                 AND c.eliminado_en IS NULL
                 AND (c.nombre ILIKE $1 OR COALESCE(c.descripcion, '') ILIKE $1)
               ORDER BY COALESCE(c.total_samples, 0) DESC, c.id DESC
               LIMIT $2"#,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn search_sample_relations(
        pool: &PgPool,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<SearchSampleRelationRecord>, AppError> {
        let rows = sqlx::query_as!(
            SearchSampleRelationRecord,
            r#"SELECT r.id,
                      cf.titulo AS fuente_titulo,
                      cf.slug AS fuente_slug,
                      cf.imagen_url AS fuente_imagen_url,
                      af.nombre AS fuente_artista,
                      cd.titulo AS destino_titulo,
                      cd.slug AS destino_slug,
                      cd.imagen_url AS destino_imagen_url,
                      ad.nombre AS destino_artista
               FROM relaciones_sample r
               INNER JOIN canciones cf ON cf.id = r.cancion_fuente_id
               INNER JOIN canciones cd ON cd.id = r.cancion_destino_id
               INNER JOIN artistas_musicales af ON af.id = cf.artista_id
               INNER JOIN artistas_musicales ad ON ad.id = cd.artista_id
               WHERE cf.titulo ILIKE $1
                  OR cd.titulo ILIKE $1
                  OR af.nombre ILIKE $1
                  OR ad.nombre ILIKE $1
               ORDER BY COALESCE(r.votos_total, 0) DESC, r.id DESC
               LIMIT $2"#,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
