use crate::db::Database;
use chrono::Utc;
use std::process::Command;

/// Sends or drafts the account activation notification email when a user is created or granted access to an instance.
/// Title / Subject: "Il tuo account per <panel_name> è attivo"
pub fn send_account_active_email(
    db: &Database,
    to_email: &str,
    username: &str,
    role: &str,
    scope_description: &str,
) -> String {
    let clean_email = to_email.trim();
    if clean_email.is_empty() || !clean_email.contains('@') {
        return "Nessuna email valida fornita (notifica saltata)".into();
    }

    let settings = db.get_panel_settings();
    let panel_name = if settings.panel_name.trim().is_empty() {
        "GOLDSMP".to_string()
    } else {
        settings.panel_name
    };

    let subject = format!("Il tuo account per {} è attivo", panel_name);
    let body = format!(
        "# Il tuo account per {} è attivo!\n\nCiao **{}**,\n\nIl tuo account sul pannello **{}** (Greatest Optimized Lightweight Dedicated Server Management Panel) è stato attivato.\n\n- **Username:** `{}`\n- **Ruolo:** `{}`\n- **Accesso Assegnato:** {}\n- **Data Attivazione:** {}\n\nPuoi ora accedere al pannello con le tue credenziali.",
        panel_name,
        username,
        panel_name,
        username,
        role,
        scope_description,
        Utc::now().format("%Y-%m-%d %H:%M UTC")
    );

    // Try sending via connected Gmail CLI or creating a Gmail draft if in unattended mode,
    // and always log to audit_logs so the admin has full visibility.
    let mut delivery_status = "Registrata nel sistema".to_string();

    if let Ok(output) = Command::new("gmail")
        .args([
            "mutate",
            "send",
            "--to",
            clean_email,
            "--subject",
            &subject,
            "--md",
            "--body",
            &body,
        ])
        .output()
    {
        if output.status.success() {
            delivery_status = format!("Email inviata a {} via Gmail", clean_email);
        } else if let Ok(draft_out) = Command::new("gmail")
            .args([
                "mutate",
                "create-draft",
                "--to",
                clean_email,
                "--subject",
                &subject,
                "--md",
                "--body",
                &body,
            ])
            .output()
        {
            if draft_out.status.success() {
                delivery_status = format!("Bozza email preparata su Gmail per {}", clean_email);
            }
        }
    }

    // Optional webhook / SMTP relay if configured via GOLDSMP_EMAIL_WEBHOOK
    if let Ok(hook_url) = std::env::var("GOLDSMP_EMAIL_WEBHOOK") {
        if !hook_url.trim().is_empty() {
            let client = reqwest::Client::new();
            let payload = serde_json::json!({
                "to": clean_email,
                "subject": subject,
                "body": body,
                "username": username
            });
            tokio::spawn(async move {
                let _ = client.post(hook_url).json(&payload).send().await;
            });
            delivery_status = format!("Inviata a {} via Email Webhook", clean_email);
        }
    }

    db.log_audit(
        "system-mailer",
        "EMAIL_ACCOUNT_ACTIVE",
        clean_email,
        &format!("Oggetto: '{}' ({})", subject, delivery_status),
        "local",
    );

    delivery_status
}
