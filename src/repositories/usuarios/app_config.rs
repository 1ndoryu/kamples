/* sentinel-disable-file directory-size — modulo legacy dentro de `repositories/`;
 * mover todo el arbol a subdirectorios es una tarea arquitectonica separada. */
/* [264A-1] Repositorio de configuracion runtime (tabla app_config).
 *
 * El backend Rust y el scraper Python comparten esta tabla como contrato:
 * el backend escribe via endpoints admin, el scraper lee al inicio de cada
 * ciclo. Por eso intencionalmente NO hay cache en memoria — siempre se
 * consulta la tabla — para que un cambio surta efecto en el siguiente ciclo
 * sin necesidad de reiniciar el scraper.
 *
 * Consultas con query_as! (validadas en compilacion contra el esquema y el
 * cache .sqlx). */

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct AppConfigEntry {
    pub clave: String,
    pub valor: serde_json::Value,
    pub descripcion: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct AppConfigRepository;

impl AppConfigRepository {
    /// Lee todas las claves cuyo nombre empieza con `prefix` (ej. "extraccion_").
    pub async fn list_by_prefix(
        pool: &PgPool,
        prefix: &str,
    ) -> Result<Vec<AppConfigEntry>, sqlx::Error> {
        let pattern = format!("{prefix}%");
        sqlx::query_as!(
            AppConfigEntry,
            r"SELECT clave, valor, descripcion, updated_at
               FROM app_config
               WHERE clave LIKE $1
               ORDER BY clave",
            pattern
        )
        .fetch_all(pool)
        .await
    }

    /// Upsert de una clave (crea si no existe, actualiza si existe).
    /// `descripcion` solo se establece en la insercion inicial — los updates
    /// posteriores no la pisan para preservar la documentacion de la migracion.
    pub async fn upsert(
        pool: &PgPool,
        clave: &str,
        valor: &serde_json::Value,
    ) -> Result<AppConfigEntry, sqlx::Error> {
        sqlx::query_as!(
            AppConfigEntry,
            r"INSERT INTO app_config (clave, valor)
               VALUES ($1, $2)
               ON CONFLICT (clave) DO UPDATE
                 SET valor = EXCLUDED.valor,
                     updated_at = NOW()
               RETURNING clave, valor, descripcion, updated_at",
            clave,
            valor
        )
        .fetch_one(pool)
        .await
    }

    /// Lee una clave individual. None si no existe.
    pub async fn get(pool: &PgPool, clave: &str) -> Result<Option<AppConfigEntry>, sqlx::Error> {
        sqlx::query_as!(
            AppConfigEntry,
            r"SELECT clave, valor, descripcion, updated_at
               FROM app_config
               WHERE clave = $1",
            clave
        )
        .fetch_optional(pool)
        .await
    }
}
