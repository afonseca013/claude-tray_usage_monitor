use keyring::Entry;

const SERVICE: &str = "tray-usage-monitor";
const CLAUDE_ACCOUNT: &str = "claude-oauth-token";
const OPENAI_ACCOUNT: &str = "openai-admin-key";

/// `Ok(None)` means no credential was ever saved. `Err` means the OS
/// credential store itself couldn't be read right now (locked session,
/// Credential Manager hiccup, etc.) — that's a transient condition, not
/// "not configured", and callers should surface it differently so it
/// doesn't look like the user needs to re-paste a token.
fn get(account: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE, account).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn set(account: &str, value: &str) -> Result<(), String> {
    Entry::new(SERVICE, account)
        .map_err(|e| e.to_string())?
        .set_password(value)
        .map_err(|e| e.to_string())
}

fn clear(account: &str) -> Result<(), String> {
    match Entry::new(SERVICE, account) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        },
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_claude_token() -> Result<Option<String>, String> {
    get(CLAUDE_ACCOUNT)
}

pub fn set_claude_token(token: &str) -> Result<(), String> {
    set(CLAUDE_ACCOUNT, token)
}

pub fn clear_claude_token() -> Result<(), String> {
    clear(CLAUDE_ACCOUNT)
}

pub fn get_openai_admin_key() -> Option<String> {
    get(OPENAI_ACCOUNT).ok().flatten()
}

pub fn set_openai_admin_key(key: &str) -> Result<(), String> {
    set(OPENAI_ACCOUNT, key)
}

pub fn clear_openai_admin_key() -> Result<(), String> {
    clear(OPENAI_ACCOUNT)
}
