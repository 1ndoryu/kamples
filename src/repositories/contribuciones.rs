/* [274A-23..26+48] Repositorio de contribuciones comunitarias.
 * Mantiene SQL fuera de handlers y replica el contrato legacy de contribuciones_pendientes.
 * Las consultas usan macros sqlx (comprobadas al compilar; .sqlx se regenera en la 89). */

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};

use crate::errors::AppError;

pub struct ContribucionesRepository;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, FromRow)]
pub struct ContribucionPendiente {
    pub id: i32,
    pub contribuidor_id: i32,
    pub contribuidor_username: String,
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub cancion_nueva_titulo: Option<String>,
    pub cancion_nueva_artista: Option<String>,
    pub cancion_nueva_youtube_url: Option<String>,
    pub cancion_nueva_lado: Option<String>,
    pub tipo_relacion: Option<String>,
    pub tipo_elemento: Option<String>,
    pub tipo_contribucion: Option<String>,
    pub relacion_existente_id: Option<i32>,
    pub cambios_propuestos: Option<Value>,
    pub estado: String,
    pub moderador_nota: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resuelto_at: Option<DateTime<Utc>>,
    pub destino_titulo: Option<String>,
    pub fuente_titulo: Option<String>,
    pub cancion_destino_slug: Option<String>,
    pub cancion_fuente_slug: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ContribucionModeracion {
    pub id: i32,
    pub contribuidor_id: i32,
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub cancion_nueva_titulo: Option<String>,
    pub cancion_nueva_artista: Option<String>,
    pub cancion_nueva_youtube_url: Option<String>,
    pub cancion_nueva_lado: Option<String>,
    pub tipo_relacion: Option<String>,
    pub tipo_elemento: Option<String>,
    pub tipo_contribucion: Option<String>,
    pub relacion_existente_id: Option<i32>,
    pub cambios_propuestos: Option<Value>,
    pub estado: String,
}

pub struct CrearContribucionRecord {
    pub contribuidor_id: i32,
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub cancion_nueva_titulo: Option<String>,
    pub cancion_nueva_artista: Option<String>,
    pub cancion_nueva_youtube_url: Option<String>,
    pub cancion_nueva_lado: Option<String>,
    pub tipo_relacion: String,
    pub tipo_elemento: String,
    pub cambios_propuestos: Option<Value>,
}

#[derive(Default)]
pub struct ActualizarContribucionRecord {
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub cancion_nueva_titulo: Option<String>,
    pub cancion_nueva_artista: Option<String>,
    pub cancion_nueva_youtube_url: Option<String>,
    pub cancion_nueva_lado: Option<String>,
    pub tipo_relacion: Option<String>,
    pub tipo_elemento: Option<String>,
}

impl ActualizarContribucionRecord {
    pub fn has_changes(&self) -> bool {
        self.cancion_destino_id.is_some()
            || self.cancion_fuente_id.is_some()
            || self.cancion_nueva_titulo.is_some()
            || self.cancion_nueva_artista.is_some()
            || self.cancion_nueva_youtube_url.is_some()
            || self.cancion_nueva_lado.is_some()
            || self.tipo_relacion.is_some()
            || self.tipo_elemento.is_some()
    }
}

impl ContribucionesRepository {
    pub async fn crear_nueva(
        pool: &PgPool,
        input: CrearContribucionRecord,
    ) -> Result<i32, AppError> {
        let id = sqlx::query_scalar!(
            "INSERT INTO contribuciones_pendientes (
                contribuidor_id, cancion_destino_id, cancion_fuente_id,
                cancion_nueva_titulo, cancion_nueva_artista, cancion_nueva_youtube_url,
                cancion_nueva_lado, tipo_relacion, tipo_elemento, cambios_propuestos
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id",
            input.contribuidor_id,
            input.cancion_destino_id,
            input.cancion_fuente_id,
            input.cancion_nueva_titulo,
            input.cancion_nueva_artista,
            input.cancion_nueva_youtube_url,
            input.cancion_nueva_lado,
            input.tipo_relacion,
            input.tipo_elemento,
            input.cambios_propuestos,
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    /* Las dos listas repiten el SELECT a propósito: query_as! exige un literal, no una constante concatenada. */
    pub async fn listar_usuario(
        pool: &PgPool,
        contribuidor_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ContribucionPendiente>, AppError> {
        let rows = sqlx::query_as!(
            ContribucionPendiente,
            "SELECT cp.id,
                cp.contribuidor_id,
                u.username AS contribuidor_username,
                cp.cancion_destino_id,
                cp.cancion_fuente_id,
                cp.cancion_nueva_titulo,
                cp.cancion_nueva_artista,
                cp.cancion_nueva_youtube_url,
                cp.cancion_nueva_lado,
                cp.tipo_relacion,
                cp.tipo_elemento,
                cp.tipo_contribucion,
                cp.relacion_existente_id,
                cp.cambios_propuestos,
                cp.estado AS \"estado!\",
                cp.moderador_nota,
                cp.created_at AS \"created_at!\",
                cp.resuelto_at,
                cd.titulo AS destino_titulo,
                cf.titulo AS fuente_titulo,
                cd.slug AS cancion_destino_slug,
                cf.slug AS cancion_fuente_slug
             FROM contribuciones_pendientes cp
             JOIN usuarios_ext u ON u.id = cp.contribuidor_id
             LEFT JOIN canciones cd ON cd.id = cp.cancion_destino_id
             LEFT JOIN canciones cf ON cf.id = cp.cancion_fuente_id
             WHERE cp.contribuidor_id = $1 ORDER BY cp.created_at DESC LIMIT $2 OFFSET $3",
            contribuidor_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn listar_pendientes_admin(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ContribucionPendiente>, AppError> {
        let rows = sqlx::query_as!(
            ContribucionPendiente,
            "SELECT cp.id,
                cp.contribuidor_id,
                u.username AS contribuidor_username,
                cp.cancion_destino_id,
                cp.cancion_fuente_id,
                cp.cancion_nueva_titulo,
                cp.cancion_nueva_artista,
                cp.cancion_nueva_youtube_url,
                cp.cancion_nueva_lado,
                cp.tipo_relacion,
                cp.tipo_elemento,
                cp.tipo_contribucion,
                cp.relacion_existente_id,
                cp.cambios_propuestos,
                cp.estado AS \"estado!\",
                cp.moderador_nota,
                cp.created_at AS \"created_at!\",
                cp.resuelto_at,
                cd.titulo AS destino_titulo,
                cf.titulo AS fuente_titulo,
                cd.slug AS cancion_destino_slug,
                cf.slug AS cancion_fuente_slug
             FROM contribuciones_pendientes cp
             JOIN usuarios_ext u ON u.id = cp.contribuidor_id
             LEFT JOIN canciones cd ON cd.id = cp.cancion_destino_id
             LEFT JOIN canciones cf ON cf.id = cp.cancion_fuente_id
             WHERE cp.estado = 'pendiente' ORDER BY cp.created_at DESC LIMIT $1 OFFSET $2",
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn contar_pendientes(pool: &PgPool) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*)::bigint AS \"total!\" FROM contribuciones_pendientes WHERE estado = 'pendiente'"
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn actualizar_pendiente_usuario(
        pool: &PgPool,
        id: i32,
        contribuidor_id: i32,
        input: ActualizarContribucionRecord,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query_scalar!(
            "UPDATE contribuciones_pendientes
             SET cancion_destino_id = COALESCE($3, cancion_destino_id),
                 cancion_fuente_id = COALESCE($4, cancion_fuente_id),
                 cancion_nueva_titulo = COALESCE($5, cancion_nueva_titulo),
                 cancion_nueva_artista = COALESCE($6, cancion_nueva_artista),
                 cancion_nueva_youtube_url = COALESCE($7, cancion_nueva_youtube_url),
                 cancion_nueva_lado = COALESCE($8, cancion_nueva_lado),
                 tipo_relacion = COALESCE($9, tipo_relacion),
                 tipo_elemento = COALESCE($10, tipo_elemento)
             WHERE id = $1 AND contribuidor_id = $2 AND estado = 'pendiente'
             RETURNING id",
            id,
            contribuidor_id,
            input.cancion_destino_id,
            input.cancion_fuente_id,
            input.cancion_nueva_titulo,
            input.cancion_nueva_artista,
            input.cancion_nueva_youtube_url,
            input.cancion_nueva_lado,
            input.tipo_relacion,
            input.tipo_elemento,
        )
        .fetch_optional(pool)
        .await?;
        Ok(updated.is_some())
    }

    pub async fn eliminar_pendiente_usuario(
        pool: &PgPool,
        id: i32,
        contribuidor_id: i32,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM contribuciones_pendientes
             WHERE id = $1 AND contribuidor_id = $2 AND estado = 'pendiente'",
            id,
            contribuidor_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn existe_duplicado_nueva(
        pool: &PgPool,
        destino_id: i32,
        fuente_id: i32,
        tipo_relacion: &str,
    ) -> Result<bool, AppError> {
        let existe = sqlx::query_scalar!(
            "SELECT EXISTS(
                SELECT 1 FROM contribuciones_pendientes
                WHERE cancion_destino_id = $1
                  AND cancion_fuente_id = $2
                  AND tipo_relacion = $3
                  AND estado = 'pendiente'
             ) AS \"existe!\"",
            destino_id,
            fuente_id,
            tipo_relacion
        )
        .fetch_one(pool)
        .await?;
        Ok(existe)
    }

    pub async fn existe_relacion(
        pool: &PgPool,
        destino_id: i32,
        fuente_id: i32,
        tipo_relacion: &str,
    ) -> Result<bool, AppError> {
        let existe = sqlx::query_scalar!(
            "SELECT EXISTS(
                SELECT 1 FROM relaciones_sample
                WHERE cancion_destino_id = $1
                  AND cancion_fuente_id = $2
                  AND tipo_relacion = $3
             ) AS \"existe!\"",
            destino_id,
            fuente_id,
            tipo_relacion
        )
        .fetch_one(pool)
        .await?;
        Ok(existe)
    }

    pub async fn crear_edicion(
        pool: &PgPool,
        contribuidor_id: i32,
        relacion_id: i32,
        cambios: Value,
    ) -> Result<i32, AppError> {
        let id = sqlx::query_scalar!(
            "INSERT INTO contribuciones_pendientes (
                contribuidor_id, relacion_existente_id, tipo_contribucion, cambios_propuestos
             ) VALUES ($1, $2, 'edicion', $3)
             RETURNING id",
            contribuidor_id,
            relacion_id,
            cambios
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn crear_eliminacion(
        pool: &PgPool,
        contribuidor_id: i32,
        relacion_id: i32,
        razon: String,
    ) -> Result<i32, AppError> {
        let id = sqlx::query_scalar!(
            "INSERT INTO contribuciones_pendientes (
                contribuidor_id, relacion_existente_id, tipo_contribucion, moderador_nota
             ) VALUES ($1, $2, 'eliminacion', $3)
             RETURNING id",
            contribuidor_id,
            relacion_id,
            razon
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn existe_propuesta_relacion_usuario(
        pool: &PgPool,
        relacion_id: i32,
        contribuidor_id: i32,
    ) -> Result<bool, AppError> {
        let existe = sqlx::query_scalar!(
            "SELECT EXISTS(
                SELECT 1 FROM contribuciones_pendientes
                WHERE relacion_existente_id = $1
                  AND contribuidor_id = $2
                  AND estado = 'pendiente'
             ) AS \"existe!\"",
            relacion_id,
            contribuidor_id
        )
        .fetch_one(pool)
        .await?;
        Ok(existe)
    }

    pub async fn buscar_para_moderar(
        pool: &PgPool,
        id: i32,
    ) -> Result<Option<ContribucionModeracion>, AppError> {
        let row = sqlx::query_as!(
            ContribucionModeracion,
            "SELECT id, contribuidor_id, cancion_destino_id, cancion_fuente_id,
                    cancion_nueva_titulo, cancion_nueva_artista, cancion_nueva_youtube_url,
                    cancion_nueva_lado, tipo_relacion, tipo_elemento, tipo_contribucion,
                    relacion_existente_id, cambios_propuestos, estado AS \"estado!\"
             FROM contribuciones_pendientes
             WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn marcar_moderada(
        pool: &PgPool,
        id: i32,
        accion: &str,
        moderador_id: i32,
        nota: Option<String>,
        relacion_id: Option<i32>,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query_scalar!(
            "UPDATE contribuciones_pendientes
             SET estado = $2,
                 moderador_id = $3,
                 moderador_nota = $4,
                 relacion_creada_id = $5,
                 resuelto_at = NOW()
             WHERE id = $1 AND estado = 'pendiente'
             RETURNING id",
            id,
            accion,
            moderador_id,
            nota,
            relacion_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(updated.is_some())
    }

    pub async fn upsert_artista_por_nombre(pool: &PgPool, nombre: &str) -> Result<i32, AppError> {
        let slug = slug::slugify(nombre);
        let id = sqlx::query_scalar!(
            "INSERT INTO artistas_musicales (nombre, slug)
             VALUES ($1, $2)
             ON CONFLICT (slug) DO UPDATE SET
                nombre = EXCLUDED.nombre,
                updated_at = NOW()
             RETURNING id",
            nombre.trim(),
            slug
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }
}
