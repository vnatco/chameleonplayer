//! "Default player" support: is Chameleon registered with Windows, which of
//! its audio types does it open, and opening Windows Settings on its own
//! page (where Windows shows a "Set default" button). Windows 10/11 don't let
//! apps make themselves the default; the user confirms it in Settings.

use serde::Serialize;

pub const APP_NAME: &str = "Chameleon Player";
const PROGID_PREFIX: &str = "ChameleonPlayer.";
/// The types the installer registers.
pub const EXTENSIONS: &[&str] = &[".mp3", ".m4a", ".m4b", ".flac", ".ogg", ".oga", ".wav", ".aac", ".aif", ".aiff", ".aifc"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Installed for all users (registered under HKLM).
    Machine,
    /// Installed for the current user only (HKCU).
    User,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultStatus {
    /// Registered with Windows by the installer. False for a dev build or a
    /// copy that was never installed; Settings has nothing to show then.
    pub registered: bool,
    pub scope: Option<Scope>,
    /// Extensions Chameleon currently opens.
    pub ours: Vec<String>,
    /// Extensions some other app opens.
    pub others: Vec<String>,
    /// Chameleon opens MP3 (the one that matters most for the prompt).
    pub is_default: bool,
}

#[cfg(windows)]
pub fn status() -> DefaultStatus {
    let scope = registered_scope();
    let mut ours = Vec::new();
    let mut others = Vec::new();
    for ext in EXTENSIONS {
        match current_default(ext) {
            Some(p) if p.starts_with(PROGID_PREFIX) => ours.push(ext.to_string()),
            _ => others.push(ext.to_string()),
        }
    }
    let is_default = ours.iter().any(|e| e == ".mp3");
    DefaultStatus { registered: scope.is_some(), scope, ours, others, is_default }
}

#[cfg(not(windows))]
pub fn status() -> DefaultStatus {
    DefaultStatus { registered: false, scope: None, ours: vec![], others: EXTENSIONS.iter().map(|e| e.to_string()).collect(), is_default: false }
}

/// The ProgID Windows uses to open `ext` for this user (respects the user's
/// own choice in Settings), or None if nothing is set.
#[cfg(windows)]
fn current_default(ext: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Shell::{ApplicationAssociationRegistration, IApplicationAssociationRegistration, AL_EFFECTIVE, AT_FILEEXTENSION};
    unsafe {
        // Command threads may not have COM yet; "already initialized" is fine.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let reg: IApplicationAssociationRegistration = CoCreateInstance(&ApplicationAssociationRegistration, None, CLSCTX_INPROC_SERVER).ok()?;
        let p = reg.QueryCurrentDefault(&HSTRING::from(ext), AT_FILEEXTENSION, AL_EFFECTIVE).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s
    }
}

/// Where the installer registered the app, if it did.
#[cfg(windows)]
fn registered_scope() -> Option<Scope> {
    use windows::core::{w, HSTRING};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let has = |root: HKEY| unsafe {
        let mut len = 0u32;
        RegGetValueW(root, w!("Software\\RegisteredApplications"), &HSTRING::from(APP_NAME), RRF_RT_REG_SZ, None, None, Some(&mut len)).is_ok()
    };
    if has(HKEY_LOCAL_MACHINE) {
        Some(Scope::Machine)
    } else if has(HKEY_CURRENT_USER) {
        Some(Scope::User)
    } else {
        None
    }
}

/// The Settings deep link that opens Chameleon's own Default Apps page.
pub fn settings_url(scope: Scope) -> String {
    let key = match scope {
        Scope::Machine => "registeredAppMachine",
        Scope::User => "registeredAppUser",
    };
    format!("ms-settings:defaultapps?{key}={}", urlencoding::encode(APP_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_links() {
        assert_eq!(settings_url(Scope::Machine), "ms-settings:defaultapps?registeredAppMachine=Chameleon%20Player");
        assert_eq!(settings_url(Scope::User), "ms-settings:defaultapps?registeredAppUser=Chameleon%20Player");
    }

    #[test]
    fn status_runs() {
        // Exercises the real Windows calls; the dev machine isn't installed,
        // so this only checks they don't fail or panic.
        let s = status();
        eprintln!("{s:?}");
        assert_eq!(s.ours.len() + s.others.len(), EXTENSIONS.len());
    }
}
