/* sentinel-disable-file directory-size — raiz legacy de servicios; los nuevos
 * dominios se agregan en subdirectorios (`services/contribuciones`, `services/dev`). */
pub mod admin_automation;
pub mod admin_duplicates;
pub mod admin_experiments;
pub mod admin_moderation;
mod admin_processes;
pub mod admin_samples;
mod admin_seed;
pub mod algo_timing;
mod audio_pipeline;
mod auth;
pub mod contribuciones;
pub mod dev;
pub mod download_token;
mod email;
pub mod extension_recorte;
pub mod extraccion_publisher;
mod fcm;
mod google_oauth;
mod ia_queue;
mod ia_service;
mod idempotency;
pub mod image_processing;
mod moderation;
mod notification;
mod notification_fanout;
mod push;
#[cfg(feature = "s3")]
pub mod s3_storage;
mod sample_catalog;
mod search;
pub mod storage;
mod stripe_service;
mod token_store;

pub use admin_automation::{AdminAutomationService, AutomationBatchReport};
pub use admin_duplicates::AdminDuplicatesService;
pub use admin_experiments::AdminExperimentsService;
pub use admin_processes::AdminProcessService;
pub use admin_samples::AdminSamplesService;
pub use admin_seed::AdminSeedService;
pub use audio_pipeline::{
    AudioPipelineError, AudioPipelineRequest, AudioPipelineResult, AudioPipelineService,
    AudioPipelineStage, AudioTechnicalAnalysis, GeneratedAudioAssets,
};
pub use auth::{AuthService, Claims, PkceExchange};
pub use email::{
    EmailDeliveryRuntime, EmailDeliveryRuntimeError, EmailNotificationService,
    NotificationOptInEmailInput, PurchaseConfirmationEmailInput,
};
pub use fcm::{
    FcmDeliveryRuntime, FcmDeliveryRuntimeError, FcmNotificationPayload, FcmNotificationService,
    FcmSendSummary, FcmTokenPlatform,
};
pub use google_oauth::{GoogleIdClaims, GoogleVerifier};
pub use ia_queue::{
    IaQueueProcessRequest, IaQueueProcessResult, IaQueueService, IaQueueServiceError,
};
pub use ia_service::{
    AudioIaAnalysisRequest, AudioIaAnalysisResult, AudioIaFailure, AudioIaProvider, AudioIaService,
    AudioIaServiceError, MetadataCorrectionRequest, OpenAiAttemptFailure,
};
pub use idempotency::IdempotencyStore;
pub use moderation::{
    ModerationAdminPanel, ModerationAiAssessment, ModerationCategory, ModerationDecision,
    ModerationEntityKind, ModerationLocalFinding, ModerationOpenAiFailure, ModerationParseFailure,
    ModerationProvider, ModerationProviderFailure, ModerationRequest, ModerationResult,
    ModerationService, ModerationServiceError, ModerationVerdict,
};
pub use notification::{
    CreateNotificationInput, NotificationService, DEFAULT_NOTIFICATION_PAGE_SIZE,
};
pub use notification_fanout::NotificationFanoutService;
pub use push::{
    PushDeliveryRuntime, PushDeliveryRuntimeError, PushNotificationPayload,
    PushNotificationService, PushSendSummary, PushSubscriptionPlatform,
};
#[cfg(feature = "s3")]
pub use s3_storage::S3Storage;
pub(crate) use sample_catalog::build_sample_summary;
pub use sample_catalog::{correct_sample_metadata, CorrectionOutcome, SampleCatalogService};
pub use search::SearchService;
pub use storage::{FileStorage, LocalFs};
pub use stripe_service::{
    StripeCheckoutSessionSummary, StripeConnectAccountSummary, StripeConnectBalanceSummary,
    StripeConnectLinkSummary, StripeConnectPayoutSummary, StripePortalSessionSummary,
    StripePriceCatalog, StripeRuntime, StripeRuntimeError, StripeSampleCheckoutRequest,
    StripeService, StripeWebhookSecretKind,
};
pub use token_store::TokenStore;
