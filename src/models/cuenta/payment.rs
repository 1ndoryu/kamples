use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::KamplesPlanId;

/* [174A-80] Modelos públicos de planes de pago.
 * Mantienen el contrato en español para el frontend, pero exponen centavos y
 * strings decimales para evitar floats como fuente de verdad. */

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPlanPublic {
    pub id: KamplesPlanId,
    pub nombre: String,
    pub precio_mensual_cents: i64,
    pub precio_mensual: String,
    pub precio_anual_cents: i64,
    pub precio_anual: String,
    pub ahorro_anual_cents: i64,
    pub ahorro_anual: String,
    pub descargas_dia: i32,
    pub subidas_mes: i32,
    pub max_samples: i32,
    pub transferencia_gb: i32,
    pub revenue_share_bps: u16,
    pub revenue_share_label: String,
    pub price_id_configurado: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prueba_gratuita_dias: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descargas_prueba: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPlansResponse {
    pub stripe_habilitado: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publishable_key: Option<String>,
    pub moneda: String,
    pub planes: Vec<PaymentPlanPublic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PaymentPlanPeriod {
    Mensual,
    Anual,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionCheckoutRequest {
    pub plan: KamplesPlanId,
    #[serde(default)]
    pub periodo: Option<PaymentPlanPeriod>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSampleCheckoutRequest {
    pub sample_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaymentRedirectResponse {
    pub ok: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaymentWebhookResponse {
    pub recibido: bool,
    pub procesado: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreatorConnectState {
    NoConfigurado,
    Pendiente,
    Activo,
    Restringido,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatorConnectStatus {
    pub estado: CreatorConnectState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_id: Option<String>,
    pub cargos_activos: bool,
    pub payouts_activos: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requerimientos_pendientes: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatorConnectBalance {
    pub disponible: f64,
    pub pendiente: f64,
    pub moneda: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatorPayoutResponse {
    pub monto: f64,
    pub estado: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FreeCodeTargetType {
    Sample,
    Coleccion,
}

impl FreeCodeTargetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Coleccion => "coleccion",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "sample" => Some(Self::Sample),
            "coleccion" => Some(Self::Coleccion),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadGrantRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codigo_gratis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateFreeCodeRequest {
    pub tipo: FreeCodeTargetType,
    pub target_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateFreeCodeResponse {
    pub ok: bool,
    pub codigo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyFreeCodeResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tipo: Option<FreeCodeTargetType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nombre_item: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimFreeCodeRequest {
    pub codigo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimFreeCodeResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tipo: Option<FreeCodeTargetType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compensado: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nombre_item: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvalidateFreeCodeResponse {
    pub ok: bool,
    pub invalidados: i64,
}
