use anyhow::Context;

const SERVICE: &str = "com.lihiko.traderobot";

pub const ALLOWED_SECRET_KEYS: &[&str] = &[
    // Placeholder keys for OpenD/Futu credentials. Actual OpenD protocol integration is a roadmap item.
    "futu.trade_password",
    "futu.api_token",
    "opend.tls_client_key_passphrase",
];

fn validate_key(key: &str) -> anyhow::Result<()> {
    if key.trim().is_empty() {
        anyhow::bail!("secret key is empty");
    }
    if !ALLOWED_SECRET_KEYS.iter().any(|k| k == &key) {
        anyhow::bail!("secret key not allowed: {key}");
    }
    Ok(())
}

fn username(profile: &str, key: &str) -> String {
    format!("{profile}:{key}")
}

pub async fn set_secret(profile: String, key: String, value: String) -> anyhow::Result<()> {
    validate_key(&key)?;
    if value.is_empty() {
        anyhow::bail!("secret value is empty");
    }

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let entry = keyring::Entry::new(SERVICE, &username(&profile, &key))
            .context("keyring entry")?;
        entry.set_password(&value).context("set password")?;
        Ok(())
    })
    .await
    .context("keyring task join")??;

    Ok(())
}

pub async fn clear_secret(profile: String, key: String) -> anyhow::Result<()> {
    validate_key(&key)?;
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let entry = keyring::Entry::new(SERVICE, &username(&profile, &key))
            .context("keyring entry")?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(e) => {
                // Treat missing entry as success to keep idempotent behavior.
                if matches!(e, keyring::Error::NoEntry) {
                    return Ok(());
                }
                Err(anyhow::anyhow!(e)).context("delete password")
            }
        }
    })
    .await
    .context("keyring task join")??;
    Ok(())
}

pub async fn secret_exists(profile: String, key: String) -> anyhow::Result<bool> {
    validate_key(&key)?;
    tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let entry = keyring::Entry::new(SERVICE, &username(&profile, &key))
            .context("keyring entry")?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(e) => {
                if matches!(e, keyring::Error::NoEntry) {
                    return Ok(false);
                }
                Err(anyhow::anyhow!(e)).context("get password")
            }
        }
    })
    .await
    .context("keyring task join")?
}

pub async fn get_secret(profile: String, key: String) -> anyhow::Result<Option<String>> {
    validate_key(&key)?;
    tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, &username(&profile, &key))
            .context("keyring entry")?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                if matches!(e, keyring::Error::NoEntry) {
                    return Ok(None);
                }
                Err(anyhow::anyhow!(e)).context("get password")
            }
        }
    })
    .await
    .context("keyring task join")?
}
