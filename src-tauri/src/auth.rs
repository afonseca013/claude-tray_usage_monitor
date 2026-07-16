use keyring::Entry;

const SERVICE: &str = "tray-usage-monitor";
const CLAUDE_ACCOUNT: &str = "claude-oauth-token";
const OPENAI_ACCOUNT: &str = "openai-admin-key";

fn get(account: &str) -> Option<String> {
    Entry::new(SERVICE, account).ok()?.get_password().ok()
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

pub fn get_claude_token() -> Option<String> {
    get(CLAUDE_ACCOUNT)
}

pub fn set_claude_token(token: &str) -> Result<(), String> {
    set(CLAUDE_ACCOUNT, token)
}

pub fn clear_claude_token() -> Result<(), String> {
    clear(CLAUDE_ACCOUNT)
}

pub fn get_openai_admin_key() -> Option<String> {
    get(OPENAI_ACCOUNT)
}

pub fn set_openai_admin_key(key: &str) -> Result<(), String> {
    set(OPENAI_ACCOUNT, key)
}

pub fn clear_openai_admin_key() -> Result<(), String> {
    clear(OPENAI_ACCOUNT)
}
