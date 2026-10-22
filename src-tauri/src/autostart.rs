//! Starting Kankulirchi when the user logs in.
//!
//! Desktop sessions, GNOME included, start the programs listed in the XDG
//! autostart folder (`~/.config/autostart`). Kankulirchi adds an entry there
//! that starts it in the background. The entry itself is the setting, so
//! removing it in another tool, such as GNOME Tweaks, turns the setting off.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::background::BACKGROUND_ARG;
use crate::error::{AppError, AppResult};

const ENTRY_FILE: &str = "kankulirchi.desktop";

/// The autostart entry under `config_dir`, normally `~/.config`.
pub fn entry_path(config_dir: &Path) -> PathBuf {
    config_dir.join("autostart").join(ENTRY_FILE)
}

pub fn is_enabled(config_dir: &Path) -> bool {
    entry_path(config_dir).is_file()
}

/// Writes an entry that starts `executable` in the background at login.
pub fn enable(config_dir: &Path, executable: &Path) -> AppResult<()> {
    let entry = desktop_entry(executable)?;
    let path = entry_path(config_dir);
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, entry)?;
    Ok(())
}

pub fn disable(config_dir: &Path) -> AppResult<()> {
    match fs::remove_file(entry_path(config_dir)) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// The program to start at login: the one that is running.
pub fn current_executable() -> AppResult<PathBuf> {
    Ok(program_to_start(std::env::var_os("APPIMAGE"), std::env::current_exe()?))
}

/// An AppImage runs from a temporary folder that changes at every launch,
/// so the AppImage file itself, whose path its runtime puts in `APPIMAGE`,
/// is started instead. Otherwise it is the running program; Linux appends
/// " (deleted)" to its path once the file has been replaced, as reinstalling
/// does, so the suffix is dropped to name the new file at the same place.
fn program_to_start(appimage: Option<OsString>, running: PathBuf) -> PathBuf {
    if let Some(appimage) = appimage.map(PathBuf::from).filter(|path| path.is_absolute()) {
        return appimage;
    }
    match running.to_str().and_then(|text| text.strip_suffix(" (deleted)")) {
        Some(original) => PathBuf::from(original),
        None => running,
    }
}

fn desktop_entry(executable: &Path) -> AppResult<String> {
    let executable = executable
        .to_str()
        .filter(|text| !text.chars().any(char::is_control))
        .ok_or_else(|| AppError::Validation("Kankulirchi can't start at login from a folder with this name.".into()))?;
    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kankulirchi\n\
         Comment=Keeps your wallpaper rotation running\n\
         Exec={} {BACKGROUND_ARG}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        quote_exec_argument(executable)
    ))
}

/// Quotes one argument of an `Exec` key, as the Desktop Entry Specification
/// asks: `"`, `` ` `` and `$` are escaped with a backslash, a backslash
/// needs four (the value is unescaped once as a string, then as an
/// argument), and `%` is doubled so it is not read as a field code.
fn quote_exec_argument(argument: &str) -> String {
    let mut quoted = String::from('"');
    for character in argument.chars() {
        match character {
            '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(character);
            }
            '\\' => quoted.push_str(r"\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabling_writes_a_background_entry_and_disabling_removes_it() {
        let config_dir = tempfile::tempdir().unwrap();
        assert!(!is_enabled(config_dir.path()));

        enable(config_dir.path(), Path::new("/home/me/.local/lib/kankulirchi/kankulirchi")).unwrap();
        assert!(is_enabled(config_dir.path()));
        let entry = fs::read_to_string(entry_path(config_dir.path())).unwrap();
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("\nExec=\"/home/me/.local/lib/kankulirchi/kankulirchi\" --background\n"));

        disable(config_dir.path()).unwrap();
        assert!(!is_enabled(config_dir.path()));
        // Turning it off twice is fine.
        disable(config_dir.path()).unwrap();
    }

    #[test]
    fn appimages_start_the_appimage_file() {
        let mounted = PathBuf::from("/tmp/.mount_KankulXyZ/usr/bin/kankulirchi");
        let appimage = Some(OsString::from("/home/me/Apps/Kankulirchi_0.1.0_amd64.AppImage"));
        assert_eq!(
            program_to_start(appimage, mounted.clone()),
            PathBuf::from("/home/me/Apps/Kankulirchi_0.1.0_amd64.AppImage")
        );
        assert_eq!(program_to_start(Some(OsString::from("relative.AppImage")), mounted.clone()), mounted);
        assert_eq!(program_to_start(None, mounted.clone()), mounted);
        assert_eq!(
            program_to_start(None, PathBuf::from("/usr/bin/kankulirchi (deleted)")),
            PathBuf::from("/usr/bin/kankulirchi")
        );
    }

    #[test]
    fn exec_arguments_are_quoted() {
        assert_eq!(quote_exec_argument("/opt/My Apps/kankulirchi"), r#""/opt/My Apps/kankulirchi""#);
        assert_eq!(quote_exec_argument(r#"/a"b`c$d"#), r#""/a\"b\`c\$d""#);
        assert_eq!(quote_exec_argument(r"/a\b"), r#""/a\\\\b""#);
        assert_eq!(quote_exec_argument("/100%/app"), r#""/100%%/app""#);
    }

    #[test]
    fn paths_that_cannot_be_written_are_refused() {
        assert!(desktop_entry(Path::new("/bad\nname/kankulirchi")).is_err());
    }
}
