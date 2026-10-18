/* [274A-6] Scoring agregado para sugerencias "Más Ideas" del usuario.
 *
 * Replica `SamplesRepository::buscarPorScoring` del legacy PHP. Recibe un
 * contexto pre-procesado (top tags, BPM promedio, key dominante) construido
 * por el handler a partir de las descargas + coleccionados del usuario, y
 * devuelve samples ordenados por relevancia excluyendo los IDs ya vistos.
 *
 * Fórmula (copiada del legacy):
 *   score = sum(CASE WHEN tag = ANY(s.tags) THEN 1 ELSE 0 END for tag in topTags)
 *         + (key matches dominant ? 3 : 0)
 *         + GREATEST(0, 5 - ABS(s.bpm - avgBpm) / 10)   [si bpm no es null]
 * Orden secundario: total_likes DESC, publicado_at DESC, id DESC.
 */

use sqlx::{PgPool, Postgres, QueryBuilder};

use super::query::{push_auto_hide_filter, push_guardado_en_coleccion_select, push_reaccion_select, SampleSummaryRow, SAMPLE_SUMMARY_SELECT};
use super::{SampleCatalogSummaryRecord, SampleRepository};

const MAX_SCORING_TAGS: usize = 10;

/// Perfil agregado (tags, BPM y tonalidad dominantes) contra el que se puntúan los samples.
#[derive(Clone, Copy)]
pub struct CriterioScoring<'a> {
    pub top_tags: &'a [String],
    pub avg_bpm: i32,
    pub dominant_key: Option<&'a str>,
    pub exclude_ids: &'a [i32],
}

impl SampleRepository {
    pub async fn find_samples_by_aggregated_scoring(
        pool: &PgPool,
        criterio: CriterioScoring<'_>,
        limit: i64,
        offset: i64,
        viewer_id: Option<i32>,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, sqlx::Error> {
        let CriterioScoring {
            top_tags,
            avg_bpm,
            dominant_key,
            exclude_ids,
        } = criterio;
        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);
        push_guardado_en_coleccion_select(&mut builder, viewer_id);
        push_reaccion_select(&mut builder, viewer_id);
        builder.push(
            " FROM samples s
              INNER JOIN usuarios_ext u ON u.id = s.creador_id
              WHERE s.eliminado_en IS NULL
                AND s.estado = 'activo'",
        );

        if !exclude_ids.is_empty() {
            builder.push(" AND s.id != ALL(");
            builder.push_bind(exclude_ids.to_vec());
            builder.push(")");
        }

        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);

        builder.push(" ORDER BY (");

        /* Tag scoring: una expresión CASE por cada tag (limitado a MAX_SCORING_TAGS). */
        let tags = &top_tags[..top_tags.len().min(MAX_SCORING_TAGS)];
        if tags.is_empty() {
            builder.push("0");
        } else {
            for (i, tag) in tags.iter().enumerate() {
                if i > 0 {
                    builder.push(" + ");
                }
                builder.push("CASE WHEN ");
                builder.push_bind(tag.clone());
                builder.push(" = ANY(s.tags) THEN 1 ELSE 0 END");
            }
        }

        /* Key scoring: +3 si coincide con la key dominante. */
        if let Some(key) = dominant_key {
            builder.push(" + CASE WHEN s.key = ");
            builder.push_bind(key.to_string());
            builder.push(" THEN 3 ELSE 0 END");
        }

        /* BPM scoring: hasta 5 puntos cuanto más cerca esté del promedio. */
        builder.push(" + CASE WHEN s.bpm IS NOT NULL THEN GREATEST(0, 5 - ABS(s.bpm - ");
        builder.push_bind(avg_bpm);
        builder.push(") / 10) ELSE 0 END");

        builder.push(") DESC, s.total_likes DESC, s.publicado_at DESC NULLS LAST, s.id DESC");
        builder.push(" LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(SampleCatalogSummaryRecord::from)
            .collect())
    }
}
