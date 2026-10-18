/* [174A-52] Puntuación del MotorRecomendacion: score por fila (señales),
 * conversión a RankedSample y diversidad post-scoring. */

use std::collections::HashSet;

use chrono::Utc;

use crate::algorithm::profile::UserProfile;
use crate::algorithm::signals::{
    AlgorithmSignalConfig, AlgorithmSignalInput, BehaviorSignalInput, ContextSignalInput,
    SocialSignalInput, TrendSignalInput,
};

use super::contratos::{RankedSample, RecommenderConfig, SampleRow};

pub(super) fn score_row(
    row: SampleRow,
    profile: &UserProfile,
    config: AlgorithmSignalConfig,
) -> RankedSample {
    let dias_desde_pub = row.publicado_at.map(|ts| {
        let secs = (Utc::now() - ts).num_seconds().max(0) as f64;
        secs / 86_400.0
    });

    let bpm_match = row.bpm.map(f64::from);
    let key_match = match (&row.key, &profile.key_fav) {
        (Some(a), Some(b)) => Some(a.eq_ignore_ascii_case(b)),
        _ => None,
    };
    let escala_match = match (&row.escala, &profile.scale_fav) {
        (Some(a), Some(b)) => Some(a.eq_ignore_ascii_case(b)),
        _ => None,
    };
    let tipo_match = profile
        .type_fav
        .as_deref()
        .map(|t| t.eq_ignore_ascii_case(&row.tipo));
    let creador_afin = profile.favorite_creators.contains(&row.creador_id);

    let genero_match = if profile.declared_genres.is_empty() {
        0.0
    } else {
        let row_lower: HashSet<String> = row.tags.iter().map(|t| t.to_lowercase()).collect();
        let hits = profile
            .declared_genres
            .iter()
            .filter(|g| row_lower.contains(&g.to_lowercase()))
            .count();
        (hits as f64) / (profile.declared_genres.len() as f64)
    };

    let signal_input = AlgorithmSignalInput {
        distancia_coseno_contenido: row.embedding_distance,
        comportamiento: BehaviorSignalInput {
            likes_dados: row.likes_dados,
            reproducciones: row.reproducciones,
            tiempo_escucha: row.tiempo_escucha,
            descargas: row.descargas_propias,
            completadas: row.completadas,
            dislike_penalty: row.dislike_penalty,
        },
        contexto: ContextSignalInput {
            bpm_candidato: bpm_match,
            bpm_promedio_usuario: profile.bpm_avg.map(f64::from),
            key_match,
            escala_match,
            genero_match,
            tipo_match,
            creador_afin,
        },
        tendencias: TrendSignalInput {
            likes_24h: row.likes_24h,
            reproducciones_24h: row.reproducciones_24h,
            descargas_7d: row.descargas_7d,
            follows_creador_7d: row.follows_creador_7d,
        },
        grafo_social: SocialSignalInput {
            creador_seguido: row.creador_seguido,
            puntos_reacciones_seguidos: 0.0, /* se conectará en 174A-54 */
        },
        dias_desde_publicacion: dias_desde_pub,
    };
    let breakdown = config.score(signal_input);

    /* Multiplicadores que en el legado iban inline en SQL: verificado, novedad
     * (parte del breakdown) y boost para no reproducidos. */
    let mult_verificado = if row.verificado { 1.15 } else { 1.0 };
    let mult_no_reproducido = if row.nunca_reproducido { 1.20 } else { 1.0 };
    let mut total = breakdown.total * mult_verificado * mult_no_reproducido;

    /* Garantizar score positivo (signals puede dar negativo en dislike). */
    if total.is_nan() || total.is_infinite() {
        total = 0.0;
    }

    row_to_ranked(row, total, false)
}

pub(super) fn row_to_ranked(row: SampleRow, score: f64, force_es_nuevo: bool) -> RankedSample {
    let es_nuevo = force_es_nuevo || row.nunca_reproducido;
    RankedSample {
        id: row.id,
        creador_id: row.creador_id,
        titulo: row.titulo,
        slug: row.slug,
        tipo: row.tipo,
        bpm: row.bpm,
        key: row.key,
        escala: row.escala,
        tags: row.tags,
        publicado_at: row.publicado_at,
        total_likes: row.total_likes,
        total_reproducciones: row.total_reproducciones,
        total_descargas: row.total_descargas,
        verificado: row.verificado,
        es_nuevo,
        score,
    }
}

/// Diversidad post-scoring: penaliza creador/género/tipo repetidos en el
/// orden actual (paridad con `aplicarDiversidadPHP` legacy, simplificado).
pub(super) fn apply_diversity(
    mut ranked: Vec<RankedSample>,
    config: &RecommenderConfig,
) -> Vec<RankedSample> {
    let mut count_creador: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    let mut count_genero: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut count_tipo: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for item in ranked.iter_mut() {
        let rc = count_creador.entry(item.creador_id).or_insert(0);
        *rc += 1;
        let factor_creador = if *rc <= config.max_por_creador {
            1.0
        } else {
            (1.0 - (*rc - config.max_por_creador) as f64 * 0.15).max(0.3)
        };

        /* Género dominante: primer tag (proxy razonable; el legado leía
         * `metadata->'genero'` JSONB, aquí mantenemos paridad funcional). */
        let genero = item
            .tags
            .first()
            .map(|t| t.to_lowercase())
            .unwrap_or_else(|| "other".to_string());
        let rg = count_genero.entry(genero).or_insert(0);
        *rg += 1;
        let factor_genero = if *rg <= config.max_por_categoria {
            1.0
        } else {
            (1.0 - (*rg - config.max_por_categoria) as f64 * 0.10).max(0.5)
        };

        let rt = count_tipo.entry(item.tipo.clone()).or_insert(0);
        *rt += 1;
        let factor_tipo = if item.tipo == "oneshot" && *rt > config.max_por_tipo {
            (1.0 - (*rt - config.max_por_tipo) as f64 * 0.12).max(0.5)
        } else {
            1.0
        };

        item.score *= factor_creador * factor_genero * factor_tipo;
    }

    ranked.sort_by(|a, b| {
        b.es_nuevo.cmp(&a.es_nuevo).then(
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    ranked
}
