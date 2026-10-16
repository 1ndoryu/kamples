use std::sync::Arc;

use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::AppConfig;

const DEFAULT_SITE_URL: &str = "https://kamples.com";

#[derive(Clone)]
pub struct EmailDeliveryRuntime {
    transport: Arc<AsyncSmtpTransport<Tokio1Executor>>,
    from_mailbox: Mailbox,
    from_email: String,
    secure_mode: EmailSecureMode,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailDeliveryRuntimeError {
    #[error("La dirección SMTP from es inválida: {0}")]
    InvalidFromAddress(String),
    #[error("El modo SMTP_SECURE es inválido: {0}")]
    InvalidSecureMode(String),
    #[error("No se pudo inicializar el transporte SMTP: {0}")]
    TransportInitialization(String),
}

#[derive(Debug, Clone, Copy)]
enum EmailSecureMode {
    Tls,
    Ssl,
}

impl EmailSecureMode {
    fn parse(value: &str) -> Result<Self, EmailDeliveryRuntimeError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "tls" | "starttls" => Ok(Self::Tls),
            "ssl" | "smtps" => Ok(Self::Ssl),
            other => Err(EmailDeliveryRuntimeError::InvalidSecureMode(
                other.to_string(),
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::Ssl => "ssl",
        }
    }
}

struct EmailContent {
    subject: String,
    html: String,
}

#[derive(Debug, Clone)]
pub struct PurchaseConfirmationEmailInput {
    pub to_email: String,
    pub user_name: String,
    pub sample_title: String,
    pub price_label: String,
    pub downloads_url: String,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NotificationOptInEmailInput {
    pub to_email: String,
    pub user_name: Option<String>,
    pub subject: String,
    pub headline: String,
    pub message: String,
    pub action_label: Option<String>,
    pub action_url: Option<String>,
    pub site_url: Option<String>,
}

pub struct EmailNotificationService;

/* [174A-77] Canal SMTP y plantillas de email.
 * - Runtime opcional y fail-fast si la config existe pero es inválida.
 * - Bienvenida conectada al registro real; compra/notificación quedan listas
 *   para Pagos y para el fanout de 174A-78. */

impl EmailDeliveryRuntime {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, EmailDeliveryRuntimeError> {
        let Some(smtp) = config.smtp.as_ref() else {
            return Ok(None);
        };

        let secure_mode = EmailSecureMode::parse(&smtp.secure)?;
        let from_mailbox: Mailbox = format!("{} <{}>", smtp.from_name, smtp.from_email)
            .parse::<Mailbox>()
            .map_err(|error| EmailDeliveryRuntimeError::InvalidFromAddress(error.to_string()))?;

        let builder = match secure_mode {
            EmailSecureMode::Ssl => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host),
            EmailSecureMode::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
            }
        }
        .map_err(|error| EmailDeliveryRuntimeError::TransportInitialization(error.to_string()))?;

        let transport = builder
            .port(smtp.port)
            .credentials(Credentials::new(smtp.user.clone(), smtp.password.clone()))
            .build();

        Ok(Some(Self {
            transport: Arc::new(transport),
            from_mailbox,
            from_email: smtp.from_email.clone(),
            secure_mode,
        }))
    }

    pub fn from_email(&self) -> &str {
        &self.from_email
    }

    pub fn secure_mode(&self) -> &'static str {
        self.secure_mode.as_str()
    }

    async fn send_html(&self, to_email: &str, subject: &str, html: String) -> Result<(), String> {
        let recipient: Mailbox = to_email
            .parse::<Mailbox>()
            .map_err(|error| format!("Dirección destino inválida '{to_email}': {error}"))?;

        let email = Message::builder()
            .from(self.from_mailbox.clone())
            .to(recipient)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .map_err(|error| format!("No se pudo construir el email '{subject}': {error}"))?;

        self.transport.send(email).await.map_err(|error| {
            format!("No se pudo enviar el email '{subject}' a {to_email}: {error}")
        })?;

        Ok(())
    }
}

impl EmailNotificationService {
    pub fn spawn_welcome(
        runtime: Arc<EmailDeliveryRuntime>,
        to_email: String,
        user_name: String,
        site_url: Option<String>,
    ) {
        tokio::spawn(async move {
            match Self::send_welcome(&runtime, &to_email, &user_name, site_url.as_deref()).await {
                Ok(()) => tracing::info!(email = %to_email, "email de bienvenida enviado"),
                Err(error) => {
                    tracing::warn!(email = %to_email, error = %error, "falló el email de bienvenida");
                }
            }
        });
    }

    pub async fn send_welcome(
        runtime: &EmailDeliveryRuntime,
        to_email: &str,
        user_name: &str,
        site_url: Option<&str>,
    ) -> Result<(), String> {
        let content = render_welcome_content(user_name, &resolve_site_url(site_url));
        runtime
            .send_html(to_email, &content.subject, content.html)
            .await
    }

    pub async fn send_purchase_confirmation(
        runtime: &EmailDeliveryRuntime,
        input: &PurchaseConfirmationEmailInput,
    ) -> Result<(), String> {
        let content = render_purchase_content(input);
        runtime
            .send_html(&input.to_email, &content.subject, content.html)
            .await
    }

    pub async fn send_notification_opt_in(
        runtime: &EmailDeliveryRuntime,
        input: &NotificationOptInEmailInput,
    ) -> Result<(), String> {
        let content = render_notification_content(input);
        runtime
            .send_html(&input.to_email, &content.subject, content.html)
            .await
    }
}

fn render_welcome_content(user_name: &str, site_url: &str) -> EmailContent {
    let name = escape_html(user_name);
    let site_url = escape_html(site_url);
    let inner = format!(
        concat!(
            "<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Hola {name},</p>",
            "<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Tu cuenta en Kamples ha sido creada exitosamente. Ya puedes explorar, descargar y compartir samples con la comunidad.</p>",
            "<p style=\"margin:0 0 24px;color:#333;font-size:16px;\">Empieza descubriendo samples ahora:</p>",
            "{button}"
        ),
        name = name,
        button = render_button("Explorar Kamples", &site_url),
    );

    EmailContent {
        subject: "Bienvenido a Kamples".to_string(),
        html: render_shell(&inner, &site_url),
    }
}

fn render_purchase_content(input: &PurchaseConfirmationEmailInput) -> EmailContent {
    let site_url = escape_html(&resolve_site_url(input.site_url.as_deref()));
    let name = escape_html(&input.user_name);
    let sample_title = escape_html(&input.sample_title);
    let price = escape_html(&input.price_label);
    let downloads_url = escape_html(&input.downloads_url);
    let subject = format!("Compra confirmada: {}", input.sample_title);
    let inner = format!(
        concat!(
            "<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Hola {name},</p>",
            "<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Tu compra se ha completado exitosamente.</p>",
            "<table width=\"100%\" cellpadding=\"12\" cellspacing=\"0\" style=\"background:#f8f9fa;border-radius:6px;margin:0 0 24px;\">",
            "<tr><td style=\"color:#666;font-size:14px;\">Sample</td><td style=\"color:#333;font-size:14px;font-weight:bold;text-align:right;\">{sample_title}</td></tr>",
            "<tr><td style=\"color:#666;font-size:14px;border-top:1px solid #e9ecef;\">Precio</td><td style=\"color:#333;font-size:14px;font-weight:bold;text-align:right;border-top:1px solid #e9ecef;\">{price}</td></tr>",
            "</table>",
            "<p style=\"margin:0 0 24px;color:#333;font-size:16px;\">Ya puedes descargar tu sample desde la seccion Comprados en tu biblioteca:</p>",
            "{button}"
        ),
        name = name,
        sample_title = sample_title,
        price = price,
        button = render_button("Ir a Mis Descargas", &downloads_url),
    );

    EmailContent {
        subject,
        html: render_shell(&inner, &site_url),
    }
}

fn render_notification_content(input: &NotificationOptInEmailInput) -> EmailContent {
    let site_url = escape_html(&resolve_site_url(input.site_url.as_deref()));
    let greeting_name = input
        .user_name
        .as_deref()
        .map(escape_html)
        .unwrap_or_default();
    let headline = escape_html(&input.headline);
    let message = escape_html(&input.message);
    let action = input
        .action_url
        .as_deref()
        .zip(input.action_label.as_deref())
        .map(|(url, label)| render_button(label, &escape_html(url)))
        .unwrap_or_default();
    let greeting = if greeting_name.is_empty() {
        "<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Hola,</p>".to_string()
    } else {
        format!("<p style=\"margin:0 0 16px;color:#333;font-size:16px;\">Hola {greeting_name},</p>")
    };
    let inner = format!(
        concat!(
            "{greeting}",
            "<p style=\"margin:0 0 16px;color:#333;font-size:16px;font-weight:bold;\">{headline}</p>",
            "<p style=\"margin:0 0 24px;color:#333;font-size:16px;\">{message}</p>",
            "{action}",
            "<p style=\"margin:24px 0 0;color:#666;font-size:13px;\">Recibes este correo porque activaste notificaciones por email en Kamples.</p>"
        ),
        greeting = greeting,
        headline = headline,
        message = message,
        action = action,
    );

    EmailContent {
        subject: input.subject.clone(),
        html: render_shell(&inner, &site_url),
    }
}

fn render_shell(inner: &str, site_url: &str) -> String {
    format!(
        concat!(
            "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"UTF-8\"></head>",
            "<body style=\"margin:0;padding:0;background:#f5f5f5;font-family:Arial,Helvetica,sans-serif;\">",
            "<table width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:#f5f5f5;padding:24px 0;\"><tr><td align=\"center\">",
            "<table width=\"560\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:#ffffff;border-radius:8px;overflow:hidden;\">",
            "<tr><td style=\"background:#1a1a2e;padding:24px;text-align:center;\"><h1 style=\"margin:0;color:#ffffff;font-size:22px;\">Kamples</h1></td></tr>",
            "<tr><td style=\"padding:32px 24px;\">{inner}</td></tr>",
            "<tr><td style=\"padding:16px 24px;border-top:1px solid #e9ecef;text-align:center;\"><p style=\"margin:0;color:#999;font-size:12px;\"><a href=\"{site_url}\" style=\"color:#999;text-decoration:none;\">Kamples</a></p></td></tr>",
            "</table></td></tr></table></body></html>"
        ),
        inner = inner,
        site_url = site_url,
    )
}

fn render_button(label: &str, url: &str) -> String {
    format!(
        concat!(
            "<table width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\">",
            "<a href=\"{url}\" style=\"display:inline-block;background:#4a665b;color:#fff;text-decoration:none;padding:14px 32px;border-radius:6px;font-size:16px;font-weight:bold;\">{label}</a>",
            "</td></tr></table>"
        ),
        url = url,
        label = escape_html(label),
    )
}

fn resolve_site_url(site_url: Option<&str>) -> String {
    site_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SITE_URL)
        .to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::{
        render_notification_content, render_purchase_content, render_welcome_content,
        EmailDeliveryRuntime, EmailDeliveryRuntimeError, NotificationOptInEmailInput,
        PurchaseConfirmationEmailInput,
    };
    use crate::config::{AppConfig, SmtpConfig};

    #[test]
    fn welcome_template_contains_name_and_site_url() {
        let content = render_welcome_content("Ada", "https://kamples.com");

        assert_eq!(content.subject, "Bienvenido a Kamples");
        assert!(content.html.contains("Hola Ada"));
        assert!(content.html.contains("https://kamples.com"));
        assert!(content.html.contains("Explorar Kamples"));
    }

    #[test]
    fn purchase_template_contains_sample_and_price() {
        let content = render_purchase_content(&PurchaseConfirmationEmailInput {
            to_email: "buyer@example.com".into(),
            user_name: "Ada".into(),
            sample_title: "Lo-Fi Dreams".into(),
            price_label: "$9.99".into(),
            downloads_url: "https://kamples.com/descargas/".into(),
            site_url: Some("https://kamples.com".into()),
        });

        assert!(content.subject.contains("Lo-Fi Dreams"));
        assert!(content.html.contains("Lo-Fi Dreams"));
        assert!(content.html.contains("$9.99"));
        assert!(content.html.contains("Ir a Mis Descargas"));
    }

    #[test]
    fn notification_template_mentions_opt_in() {
        let content = render_notification_content(&NotificationOptInEmailInput {
            to_email: "user@example.com".into(),
            user_name: Some("Ada".into()),
            subject: "Nuevo follower".into(),
            headline: "Tienes una nueva notificacion".into(),
            message: "@beatmaker empezo a seguirte".into(),
            action_label: Some("Ver perfil".into()),
            action_url: Some("https://kamples.com/perfil/beatmaker".into()),
            site_url: Some("https://kamples.com".into()),
        });

        assert!(content.html.contains("activaste notificaciones por email"));
        assert!(content.html.contains("Ver perfil"));
    }

    #[test]
    fn runtime_accepts_valid_smtp_config() {
        let config = test_config(Some(SmtpConfig {
            host: "smtp-relay.brevo.com".into(),
            port: 587,
            user: "smtp-user".into(),
            password: "smtp-pass".into(),
            from_email: "noreply@kamples.com".into(),
            from_name: "Kamples".into(),
            secure: "tls".into(),
        }));

        let runtime = EmailDeliveryRuntime::from_config(&config)
            .expect("runtime valido")
            .expect("runtime habilitado");

        assert_eq!(runtime.from_email(), "noreply@kamples.com");
        assert_eq!(runtime.secure_mode(), "tls");
    }

    #[test]
    fn runtime_rejects_invalid_secure_mode() {
        let config = test_config(Some(SmtpConfig {
            host: "smtp-relay.brevo.com".into(),
            port: 587,
            user: "smtp-user".into(),
            password: "smtp-pass".into(),
            from_email: "noreply@kamples.com".into(),
            from_name: "Kamples".into(),
            secure: "weird".into(),
        }));

        let error = EmailDeliveryRuntime::from_config(&config)
            .err()
            .expect("debe fallar");
        assert!(matches!(
            error,
            EmailDeliveryRuntimeError::InvalidSecureMode(_)
        ));
    }

    fn test_config(smtp: Option<SmtpConfig>) -> AppConfig {
        AppConfig {
            database_url: "postgres://unused".into(),
            redis_url: None,
            jwt_secret: "secret".into(),
            host: "127.0.0.1".into(),
            port: 3000,
            db_max_connections: 10,
            db_min_connections: 2,
            google_client_ids: Vec::new(),
            storage_root: "./uploads".into(),
            storage_backend: "local".into(),
            s3_bucket: None,
            s3_endpoint_url: None,
            public_base_url: Some("https://kamples.com".into()),
            ws_secret: "secret".into(),
            ws_public_url: None,
            ws_ticket_ttl_secs: 60,
            vapid_public_key: None,
            vapid_private_key: None,
            vapid_subject: None,
            fcm_service_account_json: None,
            smtp,
            scraper_secret: None,
            stripe: crate::config::StripeConfig::default(),
            allow_duplicate_uploads: false,
            rate_limit: crate::config::RateLimitConfig {
                peticiones_por_segundo: 10.0,
                rafaga: 40.0,
            },
        }
    }
}
