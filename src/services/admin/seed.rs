use std::collections::BTreeMap;

use rand::seq::SliceRandom;
use rand::Rng;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::errors::AppError;
use crate::repositories::{AdminSeedRepository, InsertSeedUserInput};

const ADJECTIVES: [&str; 50] = [
    "Cool", "Deep", "Sonic", "Raw", "Crisp", "Warm", "Bright", "Dark", "Heavy", "Smooth", "Funky",
    "Fresh", "Swift", "Bold", "Lazy", "Wild", "Silent", "Loud", "Sharp", "Mellow", "Dusty",
    "Golden", "Rusty", "Cosmic", "Urban", "Neon", "Retro", "Analog", "Digital", "Faded", "Wavy",
    "Chill", "Hazy", "Vivid", "Gritty", "Lucid", "Crispy", "Velvet", "Amber", "Jade", "Mystic",
    "Rapid", "Steady", "Hollow", "Dense", "Polar", "Solar", "Misty", "Stark", "Noble",
];

const NOUNS: [&str; 50] = [
    "Beat", "Vinyl", "Groove", "Pulse", "Chord", "Tone", "Wave", "Bass", "Drum", "Keys", "Synth",
    "Loop", "Track", "Flux", "Echo", "Pitch", "Verse", "Riff", "Fade", "Drop", "Mix", "Stem",
    "Snap", "Clap", "Kick", "Snare", "Hat", "Sample", "Crate", "Deck", "Tape", "Wax", "Spin",
    "Dub", "Filter", "Boost", "Gain", "Pan", "Clip", "Stack", "Layer", "Slice", "Chop", "Scratch",
    "Blend", "Node", "Grid", "Drift", "Spark", "Vibe",
];

const RELATIONS_PER_USER_MIN: i64 = 50;
const RELATIONS_PER_USER_MAX: i64 = 100;
const SYSTEM_USER_ID_FALLBACK: i32 = 7;

pub struct AdminSeedService;

#[derive(Clone, Copy)]
struct DistributionGroup {
    count: usize,
    share: usize,
    jitter: i64,
}

impl AdminSeedService {
    /* [254A-2] Seed real para el tab Procesos: porta el batch PHP SeedUsuarios
     * a PostgreSQL Rust. Mantiene el reparto Pareto para relaciones sin
     * contribuidor y reasigna samples auto-extraidos desde usuario sistema. */
    pub async fn run(pool: &PgPool) -> Result<BTreeMap<String, Value>, AppError> {
        let users_result = Self::ensure_seed_users(pool).await?;
        let relations_result = Self::assign_relations(pool).await?;
        let samples_result = Self::reassign_samples(pool).await?;

        let mut result = BTreeMap::new();
        result.insert("usuarios_generados".to_string(), json!(users_result));
        result.insert("relaciones".to_string(), json!(relations_result));
        result.insert("samples".to_string(), json!(samples_result));
        Ok(result)
    }

    async fn ensure_seed_users(pool: &PgPool) -> Result<Value, AppError> {
        let total_relations =
            AdminSeedRepository::count_relations(pool)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("No se pudieron contar relaciones: {error}"))
                })?;
        let existing = AdminSeedRepository::count_seed_users(pool)
            .await
            .map_err(|error| {
                AppError::Internal(format!("No se pudieron contar seed users: {error}"))
            })?;
        let required = Self::required_seed_users(total_relations);

        if existing >= required {
            return Ok(json!({
                "creados": 0,
                "existentes": existing,
                "requeridos": required,
                "errores": 0,
            }));
        }

        let missing = required - existing;
        let mut created = 0;
        let mut errors = 0;
        for _ in 0..missing {
            match Self::create_unique_seed_user(pool).await {
                Ok(true) => created += 1,
                Ok(false) | Err(_) => errors += 1,
            }
        }

        Ok(json!({
            "creados": created,
            "existentes": existing,
            "requeridos": required,
            "errores": errors,
        }))
    }

    async fn create_unique_seed_user(pool: &PgPool) -> Result<bool, AppError> {
        for _ in 0..20 {
            let username = Self::generate_username();
            let exists = AdminSeedRepository::username_exists(pool, &username)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("No se pudo verificar username seed: {error}"))
                })?;
            if exists {
                continue;
            }

            let display_name = Self::username_to_display_name(&username);
            let email = format!("seed_{}@kamples.internal", username.to_lowercase());
            let wp_user_id = Self::generate_internal_wp_user_id();
            return AdminSeedRepository::insert_seed_user(
                pool,
                InsertSeedUserInput {
                    wp_user_id,
                    username: &username,
                    email: &email,
                    nombre_visible: &display_name,
                },
            )
            .await
            .map_err(|error| AppError::Internal(format!("No se pudo crear seed user: {error}")));
        }

        Ok(false)
    }

    async fn assign_relations(pool: &PgPool) -> Result<Value, AppError> {
        let seed_users = AdminSeedRepository::list_seed_user_ids(pool)
            .await
            .map_err(|error| {
                AppError::Internal(format!("No se pudieron listar seed users: {error}"))
            })?;
        if seed_users.is_empty() {
            return Ok(json!({
                "asignadas": 0,
                "total_relaciones": 0,
                "error": "No hay seed users",
            }));
        }

        let mut relation_ids = AdminSeedRepository::list_relation_ids_without_contributor(pool)
            .await
            .map_err(|error| {
                AppError::Internal(format!(
                    "No se pudieron listar relaciones sin contribuidor: {error}"
                ))
            })?;
        let total_relations = relation_ids.len();
        if total_relations == 0 {
            return Ok(json!({ "asignadas": 0, "total_relaciones": 0 }));
        }

        relation_ids.shuffle(&mut rand::thread_rng());
        let distribution = Self::pareto_distribution(&seed_users, total_relations);
        let mut assigned = 0;
        let mut index = 0;

        for (seed_user_id, relation_count) in distribution {
            for _ in 0..relation_count {
                let Some(relation_id) = relation_ids.get(index).copied() else {
                    break;
                };
                if AdminSeedRepository::assign_relation_contributor(pool, relation_id, seed_user_id)
                    .await
                    .map_err(|error| {
                        AppError::Internal(format!("No se pudo asignar contribuidor seed: {error}"))
                    })?
                {
                    assigned += 1;
                }
                index += 1;
            }
            if index >= total_relations {
                break;
            }
        }

        Ok(json!({ "asignadas": assigned, "total_relaciones": total_relations }))
    }

    async fn reassign_samples(pool: &PgPool) -> Result<Value, AppError> {
        let system_user_id = std::env::var("KAMPLES_SISTEMA_USUARIO_ID")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(SYSTEM_USER_ID_FALLBACK);
        let reassigned = AdminSeedRepository::reassign_seed_samples(pool, system_user_id)
            .await
            .map_err(|error| {
                AppError::Internal(format!("No se pudieron reasignar samples seed: {error}"))
            })?;
        Ok(json!({ "reasignados": reassigned, "sistema_user_id": system_user_id }))
    }

    fn required_seed_users(total_relations: i64) -> i64 {
        let average = i64::midpoint(RELATIONS_PER_USER_MIN, RELATIONS_PER_USER_MAX);
        ((total_relations + average - 1) / average).max(1)
    }

    fn generate_username() -> String {
        let mut rng = rand::thread_rng();
        let adjective = ADJECTIVES.choose(&mut rng).copied().unwrap_or("Sonic");
        let noun = NOUNS.choose(&mut rng).copied().unwrap_or("Beat");
        let number = rng.gen_range(10..=99);
        format!("{adjective}{noun}{number}")
    }

    fn username_to_display_name(username: &str) -> String {
        let without_number =
            username.trim_end_matches(|character: char| character.is_ascii_digit());
        let mut display = String::new();
        for (index, character) in without_number.chars().enumerate() {
            if index > 0 && character.is_uppercase() {
                display.push(' ');
            }
            display.push(character);
        }
        display
    }

    fn generate_internal_wp_user_id() -> i64 {
        let random_suffix = rand::thread_rng().gen_range(10_000..=9_999_999);
        -1_000_000_000_i64 - random_suffix
    }

    fn pareto_distribution(seed_users: &[i32], total_relations: usize) -> Vec<(i32, usize)> {
        let mut ids = seed_users.to_vec();
        ids.shuffle(&mut rand::thread_rng());
        let total_users = ids.len();
        if total_users == 0 {
            return Vec::new();
        }

        let top_count = Self::rounded_percent(total_users, 20).max(1);
        let mid_count = Self::rounded_percent(total_users, 30).max(1);
        let top_share = Self::rounded_percent(total_relations, 60);
        let mid_share = Self::rounded_percent(total_relations, 25);
        let tail_share = total_relations.saturating_sub(top_share + mid_share);

        let mut distribution = Vec::new();
        let mut offset = 0;
        let mut accumulated = 0;
        Self::push_distribution_group(
            &ids,
            &mut distribution,
            &mut offset,
            &mut accumulated,
            total_relations,
            DistributionGroup {
                count: top_count,
                share: top_share,
                jitter: 5,
            },
        );
        Self::push_distribution_group(
            &ids,
            &mut distribution,
            &mut offset,
            &mut accumulated,
            total_relations,
            DistributionGroup {
                count: mid_count,
                share: mid_share,
                jitter: 3,
            },
        );
        let tail_count = total_users.saturating_sub(offset);
        Self::push_distribution_group(
            &ids,
            &mut distribution,
            &mut offset,
            &mut accumulated,
            total_relations,
            DistributionGroup {
                count: tail_count,
                share: tail_share,
                jitter: 2,
            },
        );
        distribution
    }

    fn rounded_percent(value: usize, percent: usize) -> usize {
        (value.saturating_mul(percent) + 50) / 100
    }

    fn push_distribution_group(
        ids: &[i32],
        distribution: &mut Vec<(i32, usize)>,
        offset: &mut usize,
        accumulated: &mut usize,
        total_relations: usize,
        group: DistributionGroup,
    ) {
        if group.count == 0 || *accumulated >= total_relations {
            return;
        }
        let per_user = ((group.share + (group.count / 2)) / group.count).max(1);
        for _ in 0..group.count {
            let Some(seed_user_id) = ids.get(*offset).copied() else {
                break;
            };
            let delta = rand::thread_rng().gen_range(-group.jitter..=group.jitter);
            let amount = (i64::try_from(per_user).unwrap_or(i64::MAX) + delta).max(1);
            let amount = usize::try_from(amount).unwrap_or(usize::MAX);
            let remaining = total_relations.saturating_sub(*accumulated);
            let amount = amount.min(remaining);
            if amount == 0 {
                break;
            }
            distribution.push((seed_user_id, amount));
            *accumulated += amount;
            *offset += 1;
        }
    }
}
