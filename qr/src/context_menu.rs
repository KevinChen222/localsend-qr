//! Current-user Explorer integration, independent of PowerShell execution policy.
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHChangeNotify};
use winreg::{HKCU, RegKey};

const CLASSES: [&str; 3] = ["*", "Directory", r"Directory\Background"];

pub fn install(core_path: Option<PathBuf>) -> Result<()> {
    let core = std::path::absolute(core_path.unwrap_or(std::env::current_exe()?))?;
    if !core.is_file() {
        bail!("Core executable not found: {}", core.display());
    }
    let path = core.to_str().context("Core path is not valid Unicode")?;
    let (classes, _) = HKCU.create_subkey(r"Software\Classes")?;
    install_entries(&classes, path)?;
    notify_explorer();
    println!("Registered for the current user: {path}");
    println!("Windows 11: right-click > Show more options > 由二维码发送 / 由二维码接收");
    println!("Run Register-ContextMenu.cmd again after moving the portable folder.");
    Ok(())
}

fn install_entries(classes: &RegKey, path: &str) -> Result<()> {
    for class in CLASSES {
        let argument = if class == r"Directory\Background" {
            "%V"
        } else {
            "%1"
        };
        for (verb, label, action) in [
            ("LocalSendQR", "由二维码发送", "share"),
            ("LocalSendQRReceive", "由二维码接收", "receive"),
        ] {
            if class == "*" && action == "receive" {
                continue;
            }
            // A trailing slash keeps a filesystem-root argument from ending
            // in a backslash immediately before the closing Windows quote.
            let argument = if action == "receive" || class == r"Directory\Background" {
                format!("{argument}/")
            } else {
                argument.to_owned()
            };
            let command = format!("\"{path}\" {action} --pause-on-error -- \"{argument}\"");
            let (key, _) = classes.create_subkey(format!(r"{class}\shell\{verb}"))?;
            key.set_value("", &label)?;
            key.set_value("Icon", &format!("\"{path}\",0"))?;
            if class == r"Directory\Background" {
                // Background menus have no selected item. "Single" makes
                // Explorer hide the verb; remove it on upgrades as well.
                match key.delete_value("MultiSelectModel") {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            } else {
                key.set_value("MultiSelectModel", &"Single")?;
            }
            let (command_key, _) = key.create_subkey("command")?;
            command_key.set_value("", &command)?;
            let actual: String = command_key.get_value("")?;
            if actual != command {
                bail!("Context menu verification failed for {class}/{verb}");
            }
        }
    }
    Ok(())
}

pub fn remove() -> Result<()> {
    let (classes, _) = HKCU.create_subkey(r"Software\Classes")?;
    remove_entries(&classes)?;
    notify_explorer();
    println!("LocalSend QR context menu removed for the current user.");
    Ok(())
}

fn notify_explorer() {
    // Association changes carry no item pointers. Flush before returning so
    // already-open Explorer windows can refresh their cached menus.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_FLUSH,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

fn remove_entries(classes: &RegKey) -> Result<()> {
    for class in CLASSES {
        for verb in ["LocalSendQR", "LocalSendQRReceive"] {
            let path = format!(r"{class}\shell\{verb}");
            match classes.delete_subkey_all(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).with_context(|| format!("Cannot remove {path}")),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_menus_register_relocate_and_remove_in_isolated_registry() -> Result<()> {
        let scratch = format!(r"Software\LocalSendQR-Test-{}", uuid::Uuid::new_v4());
        let (root, _) = HKCU.create_subkey(&scratch)?;
        let result = (|| -> Result<()> {
            for verb in ["LocalSendQR", "LocalSendQRReceive"] {
                let (key, _) = root.create_subkey(format!(r"Directory\Background\shell\{verb}"))?;
                key.set_value("MultiSelectModel", &"Single")?;
            }
            for executable in [
                r"C:\中文 first folder\localsend-qr.exe",
                r"C:\moved [folder] & space\localsend-qr.exe",
            ] {
                install_entries(&root, executable)?;
                for (class, verb, action, argument, label) in [
                    ("*", "LocalSendQR", "share", "%1", "由二维码发送"),
                    ("Directory", "LocalSendQR", "share", "%1", "由二维码发送"),
                    (
                        r"Directory\Background",
                        "LocalSendQR",
                        "share",
                        "%V/",
                        "由二维码发送",
                    ),
                    (
                        "Directory",
                        "LocalSendQRReceive",
                        "receive",
                        "%1/",
                        "由二维码接收",
                    ),
                    (
                        r"Directory\Background",
                        "LocalSendQRReceive",
                        "receive",
                        "%V/",
                        "由二维码接收",
                    ),
                ] {
                    let key = root.open_subkey(format!(r"{class}\shell\{verb}"))?;
                    assert_eq!(key.get_value::<String, _>("")?, label);
                    if class == r"Directory\Background" {
                        assert_eq!(
                            key.get_value::<String, _>("MultiSelectModel")
                                .unwrap_err()
                                .kind(),
                            std::io::ErrorKind::NotFound
                        );
                    } else {
                        assert_eq!(key.get_value::<String, _>("MultiSelectModel")?, "Single");
                    }
                    let command = key.open_subkey("command")?.get_value::<String, _>("")?;
                    assert_eq!(
                        command,
                        format!("\"{executable}\" {action} --pause-on-error -- \"{argument}\"")
                    );
                }
                assert!(root.open_subkey(r"*\shell\LocalSendQRReceive").is_err());
            }
            remove_entries(&root)?;
            remove_entries(&root)?;
            for class in CLASSES {
                for verb in ["LocalSendQR", "LocalSendQRReceive"] {
                    assert!(root.open_subkey(format!(r"{class}\shell\{verb}")).is_err());
                }
            }
            Ok(())
        })();
        drop(root);
        HKCU.delete_subkey_all(&scratch)?;
        result
    }
}
