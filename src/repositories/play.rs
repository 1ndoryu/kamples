use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-58] Repositorio de reproducciones (`reproducciones` + contador en
 * `samples.total_reproducciones`). Port directo de `ReproduccionesRepository.php`.
 *
 * Política de debounce: si existe un play del mismo (usuario, sample) en los
 * últimos N segundos (default 3), `register_with_debounce` actualiza la fila
 * existente en vez de insertar una nueva. Esto evita inflar contadores si el
 * cliente reenvía la señal de reproducción accidentalmente. */

pub struct PlayRepository;

#[derive(Debug, Clone)]
pub struct RegisterPlayOutcome {
    pub debounced: bool,
    pub completed_now: bool,
}

impl PlayRepository {
    /// `true` si el sample existe y no está eliminado. Evita que el INSERT de
    /// `reproducciones` falle por la FK y devuelva 500 (ver handlers/plays.rs).
    pub async fn existe_activo(pool: &PgPool, sample_id: i32) -> Result<bool, AppError> {
        let id = sqlx::query_scalar!(
            "SELECT id FROM samples WHERE id = $1 AND eliminado_en IS NULL",
            sample_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(id.is_some())
    }

    /// Devuelve los IDs únicos de samples reproducidos por el usuario.
    /// Se usa para pintar indicadores de "ya escuchado" en la UI legacy.
    pub async fn list_played_ids(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar::<_, i32>(
            "SELECT DISTINCT sample_id FROM reproducciones WHERE usuario_id = $1 ORDER BY sample_id DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }

    /// Inserta una reproducción nueva o actualiza la existente si el mismo
    /// usuario reprodujo el mismo sample en los últimos `debounce_seconds`.
    /// Devuelve `debounced=true` cuando el insert se evitó.
    pub async fn register_with_debounce(
        pool: &PgPool,
        user_id: i32,
        sample_id: i32,
        duration_listened: f32,
        completed: bool,
        debounce_seconds: i64,
    ) -> Result<RegisterPlayOutcome, AppError> {
        #[allow(clippy::cast_precision_loss)]
        let debounce_secs_f64 = debounce_seconds as f64;
        let recent = sqlx::query!(
            r#"
            SELECT id AS "id!", completada AS "completada!"
              FROM reproducciones
             WHERE usuario_id = $1
               AND sample_id  = $2
               AND created_at >= NOW() - make_interval(secs => $3::double precision)
             ORDER BY created_at DESC
             LIMIT 1
            "#,
            user_id,
            sample_id,
            debounce_secs_f64,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = recent {
            sqlx::query!(
                "UPDATE reproducciones \
                    SET duracion_escuchada = GREATEST(duracion_escuchada, $1), \
                        completada = completada OR $2 \
                  WHERE id = $3",
                duration_listened,
                completed,
                row.id,
            )
            .execute(pool)
            .await?;

            return Ok(RegisterPlayOutcome {
                debounced: true,
                completed_now: completed && !row.completada,
            });
        }

        sqlx::query!(
            "INSERT INTO reproducciones (usuario_id, sample_id, duracion_escuchada, completada) \
             VALUES ($1, $2, $3, $4)",
            user_id,
            sample_id,
            duration_listened,
            completed,
        )
        .execute(pool)
        .await?;

        Ok(RegisterPlayOutcome {
            debounced: false,
            completed_now: completed,
        })
    }

    /// Incrementa `samples.total_reproducciones` en 1 (sin tocar otras columnas).
    pub async fn increment_sample_counter(pool: &PgPool, sample_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE samples SET total_reproducciones = total_reproducciones + 1 WHERE id = $1",
            sample_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
