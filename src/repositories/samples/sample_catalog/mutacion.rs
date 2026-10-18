/* Mutaciones de metadata y borrado logico; separado de sample_catalog.rs por tamano. */
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::super::sample::SampleRepository;
use super::UpdateSamplePatch;

impl SampleRepository {
    pub async fn update_sample_metadata(
        pool: &PgPool,
        sample_id: i32,
        patch: &UpdateSamplePatch,
    ) -> Result<(), sqlx::Error> {
        /* [254A-8a] Bug pre-existente: Separated::push_bind inserta el separador
         * antes del bind, lo que generaba `col = , $1`. Se usa push_bind_unseparated
         * y push_unseparated para mantener `col = $1` dentro de cada asignacion. */
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE samples SET ");
        {
            let mut separated = builder.separated(", ");

            if let Some(titulo) = &patch.titulo {
                separated.push("titulo = ");
                separated.push_bind_unseparated(titulo.clone());
            }

            if let Some(descripcion) = &patch.descripcion {
                separated.push("descripcion = ");
                separated.push_bind_unseparated(descripcion.clone());
            }

            if let Some(tags) = &patch.tags {
                separated.push("tags = ");
                separated.push_bind_unseparated(tags.clone());
                separated.push_unseparated("::text[]");
            }

            if let Some(sample_type) = &patch.sample_type {
                separated.push("tipo = ");
                separated.push_bind_unseparated(sample_type.clone());
            }

            if let Some(es_premium) = patch.es_premium {
                separated.push("es_premium = ");
                separated.push_bind_unseparated(es_premium);
            }

            if let Some(precio) = &patch.precio {
                match precio {
                    Some(value) => {
                        separated.push("precio = ");
                        separated.push_bind_unseparated(*value);
                    }
                    None => {
                        separated.push("precio = NULL");
                    }
                }
            }

            if let Some(permitir_descarga) = patch.permitir_descarga {
                separated.push("permitir_descarga = ");
                separated.push_bind_unseparated(permitir_descarga);
            }

            if let Some(licencia_libre) = patch.licencia_libre {
                separated.push("licencia_libre = ");
                separated.push_bind_unseparated(licencia_libre);
            }

            if let Some(mostrar_en_comunidad) = patch.mostrar_en_comunidad {
                separated.push("mostrar_en_comunidad = ");
                separated.push_bind_unseparated(mostrar_en_comunidad);
            }

            if let Some(imagen_url) = &patch.imagen_url {
                match imagen_url {
                    Some(value) => {
                        separated.push("imagen_url = ");
                        separated.push_bind_unseparated(value.clone());
                    }
                    None => {
                        separated.push("imagen_url = NULL");
                    }
                }
            }

            separated.push("updated_at = NOW()");
        }

        builder.push(" WHERE id = ");
        builder.push_bind(sample_id);

        builder.build().execute(pool).await?;
        Ok(())
    }

    pub async fn soft_delete_owned_sample(
        pool: &PgPool,
        sample_id: i32,
        owner_id: i32,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "UPDATE samples
             SET estado = 'eliminado', eliminado_en = NOW(), updated_at = NOW()
             WHERE id = $1 AND creador_id = $2 AND eliminado_en IS NULL",
            sample_id,
            owner_id,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
