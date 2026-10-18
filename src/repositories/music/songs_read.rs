use sqlx::types::Json;
use sqlx::PgPool;

use super::support::{
    map_relation_chain_node, map_relation_summary, map_song_artist_link, RelationChainNodeRecord,
    SampleRelationSummaryRecord, SongArtistLinkRecord,
};
use super::MusicRepository;
use crate::errors::AppError;
use crate::models::{MusicSong, RelationChainNode, SampleRelationSummary, SongArtistLink};

impl MusicRepository {
    pub async fn count_songs(pool: &PgPool) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM canciones"#)
            .fetch_one(pool)
            .await?;
        Ok(total)
    }

    pub async fn list_songs(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<MusicSong>, AppError> {
        let rows = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               ORDER BY c.created_at DESC, c.id DESC
               LIMIT $1 OFFSET $2"#,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn search_songs(
        pool: &PgPool,
        query: &str,
        like_query: &str,
        limit: i64,
    ) -> Result<Vec<MusicSong>, AppError> {
        let rows = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               WHERE to_tsvector('simple', COALESCE(c.titulo, '') || ' ' || COALESCE(a.nombre, '') || ' ' || COALESCE(c.album, ''))
                        @@ plainto_tsquery('simple', $1)
                  OR c.titulo ILIKE $2
                  OR a.nombre ILIKE $2
               ORDER BY (COALESCE(c.total_sampleada, 0) + COALESCE(c.total_samplea, 0)) DESC, c.id DESC
               LIMIT $3"#,
            query,
            like_query,
            limit,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn top_songs(pool: &PgPool, limit: i64) -> Result<Vec<MusicSong>, AppError> {
        let rows = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               ORDER BY COALESCE(c.total_sampleada, 0) DESC, c.id DESC
               LIMIT $1"#,
            limit,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_song_by_slug(
        pool: &PgPool,
        slug: &str,
    ) -> Result<Option<MusicSong>, AppError> {
        let row = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               WHERE c.slug = $1
               LIMIT 1"#,
            slug,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_song_by_id(pool: &PgPool, id: i32) -> Result<Option<MusicSong>, AppError> {
        let row = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               WHERE c.id = $1
               LIMIT 1"#,
            id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_song_artists(
        pool: &PgPool,
        song_id: i32,
    ) -> Result<Vec<SongArtistLink>, AppError> {
        let rows = sqlx::query_as!(
            SongArtistLinkRecord,
            r#"SELECT ca.artista_id,
                      a.nombre AS "nombre!",
                      a.slug AS "slug!",
                      ca.rol AS "rol!"
               FROM canciones_artistas ca
               INNER JOIN artistas_musicales a ON a.id = ca.artista_id
               WHERE ca.cancion_id = $1
               ORDER BY CASE ca.rol
                    WHEN 'principal' THEN 1
                    WHEN 'featuring' THEN 2
                    WHEN 'producer' THEN 3
                    ELSE 4
               END, a.nombre ASC"#,
            song_id,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_song_artist_link).collect()
    }

    pub async fn list_song_relations_as_destino(
        pool: &PgPool,
        song_id: i32,
        limit: i64,
    ) -> Result<Vec<SampleRelationSummary>, AppError> {
        let rows = sqlx::query_as!(
            SampleRelationSummaryRecord,
            r#"SELECT r.id,
                      r.cancion_destino_id,
                      r.cancion_fuente_id,
                      r.whosampled_id,
                      r.tipo_relacion AS "tipo_relacion!",
                      r.tipo_elemento,
                      COALESCE(r.timings_destino, '[]'::jsonb) AS "timings_destino!: Json<Vec<i32>>",
                      COALESCE(r.timings_fuente, '[]'::jsonb) AS "timings_fuente!: Json<Vec<i32>>",
                      COALESCE(r.aparece_en_todo, FALSE) AS "aparece_en_todo!",
                      r.sample_id,
                      r.sample_fuente_id,
                      r.sample_destino_id,
                      COALESCE(r.votos_total, 0) AS "votos_total!",
                      COALESCE(r.votos_promedio, 0)::double precision AS "votos_promedio!",
                      r.fuente AS "fuente!",
                      r.contribuidor_id,
                      u.username AS "contribuidor_username?: String",
                      COALESCE(r.verificada, FALSE) AS "verificada!",
                      COALESCE(r.total_likes, 0) AS "total_likes!",
                      COALESCE(r.total_comentarios, 0) AS "total_comentarios!",
                      r.created_at AS "created_at!",
                      r.updated_at AS "updated_at!",
                      c.titulo AS "cancion_titulo!",
                      c.slug AS "cancion_slug!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!",
                      c.anio AS cancion_anio,
                      c.imagen_url AS cancion_imagen_url
               FROM relaciones_sample r
               INNER JOIN canciones c ON c.id = r.cancion_fuente_id
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               LEFT JOIN usuarios_ext u ON u.id = r.contribuidor_id
               WHERE r.cancion_destino_id = $1
               ORDER BY r.created_at DESC, r.id DESC
               LIMIT $2"#,
            song_id,
            limit,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_relation_summary).collect()
    }

    pub async fn list_song_relations_as_fuente(
        pool: &PgPool,
        song_id: i32,
        limit: i64,
    ) -> Result<Vec<SampleRelationSummary>, AppError> {
        let rows = sqlx::query_as!(
            SampleRelationSummaryRecord,
            r#"SELECT r.id,
                      r.cancion_destino_id,
                      r.cancion_fuente_id,
                      r.whosampled_id,
                      r.tipo_relacion AS "tipo_relacion!",
                      r.tipo_elemento,
                      COALESCE(r.timings_destino, '[]'::jsonb) AS "timings_destino!: Json<Vec<i32>>",
                      COALESCE(r.timings_fuente, '[]'::jsonb) AS "timings_fuente!: Json<Vec<i32>>",
                      COALESCE(r.aparece_en_todo, FALSE) AS "aparece_en_todo!",
                      r.sample_id,
                      r.sample_fuente_id,
                      r.sample_destino_id,
                      COALESCE(r.votos_total, 0) AS "votos_total!",
                      COALESCE(r.votos_promedio, 0)::double precision AS "votos_promedio!",
                      r.fuente AS "fuente!",
                      r.contribuidor_id,
                      u.username AS "contribuidor_username?: String",
                      COALESCE(r.verificada, FALSE) AS "verificada!",
                      COALESCE(r.total_likes, 0) AS "total_likes!",
                      COALESCE(r.total_comentarios, 0) AS "total_comentarios!",
                      r.created_at AS "created_at!",
                      r.updated_at AS "updated_at!",
                      c.titulo AS "cancion_titulo!",
                      c.slug AS "cancion_slug!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!",
                      c.anio AS cancion_anio,
                      c.imagen_url AS cancion_imagen_url
               FROM relaciones_sample r
               INNER JOIN canciones c ON c.id = r.cancion_destino_id
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               LEFT JOIN usuarios_ext u ON u.id = r.contribuidor_id
               WHERE r.cancion_fuente_id = $1
               ORDER BY r.created_at DESC, r.id DESC
               LIMIT $2"#,
            song_id,
            limit,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_relation_summary).collect()
    }

    pub async fn relation_chain(
        pool: &PgPool,
        song_id: i32,
        depth: i32,
    ) -> Result<Vec<RelationChainNode>, AppError> {
        let rows = sqlx::query_as!(
            RelationChainNodeRecord,
            r#"WITH RECURSIVE cadena AS (
                    SELECT r.id,
                           r.cancion_fuente_id,
                           r.cancion_destino_id,
                           r.tipo_relacion,
                           1::int4 AS nivel
                    FROM relaciones_sample r
                    WHERE r.cancion_fuente_id = $1
                      AND r.tipo_relacion = 'sample'
                    UNION ALL
                    SELECT r.id,
                           r.cancion_fuente_id,
                           r.cancion_destino_id,
                           r.tipo_relacion,
                           (c.nivel + 1)::int4 AS nivel
                    FROM relaciones_sample r
                    INNER JOIN cadena c ON r.cancion_fuente_id = c.cancion_destino_id
                    WHERE c.nivel < $2
                      AND r.tipo_relacion = 'sample'
               )
               SELECT DISTINCT ON (ca.cancion_destino_id)
                 ca.id AS "id!",
                 ca.cancion_fuente_id AS "cancion_fuente_id!",
                 ca.cancion_destino_id AS "cancion_destino_id!",
                      ca.tipo_relacion AS "tipo_relacion!",
                      ca.nivel AS "nivel!",
                      cf.titulo AS "fuente_titulo!",
                      cf.slug AS "fuente_slug!",
                      af.nombre AS "fuente_artista!",
                      cd.titulo AS "destino_titulo!",
                      cd.slug AS "destino_slug!",
                      ad.nombre AS "destino_artista!"
               FROM cadena ca
               INNER JOIN canciones cf ON cf.id = ca.cancion_fuente_id
               INNER JOIN artistas_musicales af ON af.id = cf.artista_id
               INNER JOIN canciones cd ON cd.id = ca.cancion_destino_id
               INNER JOIN artistas_musicales ad ON ad.id = cd.artista_id
               ORDER BY ca.cancion_destino_id, ca.nivel ASC"#,
            song_id,
            depth,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_relation_chain_node).collect()
    }

    pub async fn list_genres(pool: &PgPool) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"SELECT DISTINCT genero
                 FROM canciones
                WHERE genero IS NOT NULL AND genero != ''
                ORDER BY genero"#
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().flatten().collect())
    }

    pub async fn find_random_song(
        pool: &PgPool,
        generos: &[String],
        decadas: &[i32],
    ) -> Result<Option<MusicSong>, AppError> {
        // Cada década es un rango [inicio, fin): basta con que la canción caiga en uno.
        // Un array vacío desactiva su filtro, igual que la antigua cláusula opcional.
        let inicios: Vec<i16> = decadas
            .iter()
            .map(|&d| i16::try_from(d).unwrap_or(0))
            .collect();
        let finales: Vec<i16> = decadas
            .iter()
            .map(|&d| i16::try_from(d + 10).unwrap_or(0))
            .collect();

        let row = sqlx::query_as!(
            MusicSong,
            r#"SELECT c.id,
                      c.titulo,
                      c.slug,
                      c.artista_id,
                      c.album,
                      c.sello,
                      c.anio,
                      c.duracion_segundos,
                      c.genero,
                      c.youtube_id,
                      c.spotify_id,
                      c.imagen_url,
                      c.whosampled_url,
                      c.bpm,
                      c.tonalidad,
                      COALESCE(c.metadata, '{}'::jsonb) AS "metadata!: serde_json::Value",
                      COALESCE(c.total_sampleada, 0) AS "total_sampleada!",
                      COALESCE(c.total_samplea, 0) AS "total_samplea!",
                      COALESCE(c.total_likes, 0) AS "total_likes!",
                      COALESCE(c.total_comentarios, 0) AS "total_comentarios!",
                      c.created_at AS "created_at!",
                      c.updated_at AS "updated_at!",
                      a.nombre AS "artista_nombre!",
                      a.slug AS "artista_slug!"
               FROM canciones c
               INNER JOIN artistas_musicales a ON a.id = c.artista_id
               WHERE (cardinality($1::text[]) = 0 OR c.genero = ANY($1::text[]))
                 AND (cardinality($2::smallint[]) = 0 OR EXISTS (
                       SELECT 1 FROM unnest($2::smallint[], $3::smallint[]) AS r(ini, fin)
                        WHERE c.anio >= r.ini AND c.anio < r.fin))
               ORDER BY RANDOM()
               LIMIT 1"#,
            generos,
            &inicios,
            &finales
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
