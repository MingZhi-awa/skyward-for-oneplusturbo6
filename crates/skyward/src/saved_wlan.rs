//! The networks whose passwords the recovery keeps, so that joining one of
//! them again takes no typing. They sit on persist, the one partition a
//! recovery can write before any user's data opens, and nothing on it is
//! encrypted: whoever holds the phone can read what is kept here.

use anyhow::{bail, Context, Result};
use skyward_net::supplicant::Security;
use skyward_net::wlan::Saved;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The folder on persist the file sits in, beside what the phone's own
/// programs keep there.
const FOLDER: &str = "skyward";

/// The file the networks are kept in.
const FILE: &str = "wlan";

/// The first line of the file, naming what it holds and how then, so that
/// one of another kind is left alone.
const TITLE: &str = "skyward wlan 1";

/// A network and the password it takes, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub ssid: Vec<u8>,
    pub security: Security,
    pub password: String,
}

impl Kept {
    /// What the pages are told of it: that its password is kept, not which.
    pub fn seen(&self) -> Saved {
        Saved { ssid: self.ssid.clone(), security: self.security }
    }
}

/// The networks kept, none where the file is not there yet.
pub fn load() -> Result<Vec<Kept>> {
    let Some(root) = persist()? else {
        return Ok(Vec::new());
    };
    let path = root.join(FOLDER).join(FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    Ok(parse(&text))
}

/// Keep `networks` in place of what was kept before. A recovery that cannot
/// reach persist keeps nothing, and says so.
pub fn store(networks: &[Kept]) -> Result<()> {
    let Some(root) = persist()? else {
        bail!("persist is not there to keep the networks on");
    };
    let folder = root.join(FOLDER);
    fs::create_dir_all(&folder).with_context(|| format!("make {}", folder.display()))?;
    let path = folder.join(FILE);
    fs::write(&path, written(networks)).with_context(|| format!("write {}", path.display()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("keep {} to the recovery", path.display()))?;
    Ok(())
}

/// Where persist is mounted, mounted writable where it is not mounted yet.
/// Nothing where the phone has none the recovery can mount.
fn persist() -> Result<Option<PathBuf>> {
    let device = match skyward_block::devnum(Path::new(skyward_sys::clock::PERSIST)) {
        Ok(device) => device,
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("no persist to keep the networks on: {e:#}"));
            return Ok(None);
        }
    };
    if let Some(at) = skyward_block::mount::points_of(device)?.first() {
        return Ok(Some(at.clone()));
    }
    match skyward_block::fstab::mount("persist") {
        Ok(at) => Ok(Some(at)),
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("persist does not mount, so no network is kept: {e:#}"));
            Ok(None)
        }
    }
}

/// The networks as the file holds them: the title, then one line a
/// network, each its name as hex, its protection as a letter, and its
/// password as hex, which leaves out nothing a name or a password may hold
/// and keeps the file to one line of plain text a line.
fn written(networks: &[Kept]) -> String {
    let mut text = String::from(TITLE);
    text.push('\n');
    for n in networks {
        text.push_str(&hex(&n.ssid));
        text.push(' ');
        text.push(letter(n.security));
        text.push(' ');
        text.push_str(&hex(n.password.as_bytes()));
        text.push('\n');
    }
    text
}

/// The networks a file holds; those of the other kind, and those cut short,
/// are left out.
fn parse(text: &str) -> Vec<Kept> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(TITLE) {
        return Vec::new();
    }
    lines.filter_map(kept).collect()
}

/// A line of the file as a network, and nothing where it does not say one
/// whole. A password cut away with the line's end is one there is none of,
/// as a network joined with none has.
fn kept(line: &str) -> Option<Kept> {
    let line = line.trim_end();
    if line.trim_start().is_empty() || line.trim_start().starts_with('#') {
        return None;
    }
    let mut parts = line.split(' ');
    let (ssid, protection) = (parts.next()?, parts.next()?);
    let password = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return None;
    }
    Some(Kept {
        ssid: unhex(ssid)?,
        security: security_of(protection)?,
        password: String::from_utf8(unhex(password)?).ok()?,
    })
}

/// How a protection is written, one letter a kind.
fn letter(security: Security) -> char {
    match security {
        Security::Open => 'o',
        Security::Owe => 'e',
        Security::Wep => 'w',
        Security::Psk => 'p',
        Security::Sae => 's',
        Security::PskSae => 'b',
        Security::Enterprise => 'n',
    }
}

fn security_of(letter: &str) -> Option<Security> {
    Some(match letter {
        "o" => Security::Open,
        "e" => Security::Owe,
        "w" => Security::Wep,
        "p" => Security::Psk,
        "s" => Security::Sae,
        "b" => Security::PskSae,
        "n" => Security::Enterprise,
        _ => return None,
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept_network(ssid: &str, security: Security, password: &str) -> Kept {
        Kept { ssid: ssid.as_bytes().to_vec(), security, password: password.into() }
    }

    #[test]
    fn what_is_written_reads_back_as_it_was_kept() {
        let networks = [
            kept_network("Skyward", Security::Psk, "secret12"),
            kept_network("house of the cat", Security::Sae, "unicode and a space 密码"),
            kept_network("Skyward-Open", Security::Open, ""),
        ];
        assert_eq!(parse(&written(&networks)), networks);
    }

    #[test]
    fn the_title_keeps_a_file_of_another_kind_to_itself() {
        assert!(parse("wpa_supplicant 2\n536b79 70 736563\n").is_empty());
        assert!(parse("").is_empty());
        assert_eq!(parse(&format!("{TITLE}\n")).len(), 0);
    }

    #[test]
    fn a_line_cut_short_or_odd_says_no_network() {
        let text = format!("{TITLE}\n536b79 70 7365\n536b79 71\n536b79 7z 7365\n\n# a note\n");
        assert!(parse(&text).is_empty());
        assert_eq!(parse("skyward wlan 1\n").len(), 0);
    }

    #[test]
    fn a_bad_hex_letter_is_left_out_rather_than_guessed() {
        assert!(unhex("abc").is_none());
        assert!(unhex("zz").is_none());
        assert_eq!(unhex("536b79"), Some(b"Sky".to_vec()));
    }
}
