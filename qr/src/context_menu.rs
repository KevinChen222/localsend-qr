//! Current-user Explorer integration, independent of PowerShell execution policy.
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use winreg::HKCU;

const CLASSES: [&str; 2] = ["*", "Directory"];

pub fn install(core_path: Option<PathBuf>) -> Result<()> {
    let core = std::path::absolute(core_path.unwrap_or(std::env::current_exe()?))?;
    if !core.is_file() {
        bail!("Core executable not found: {}", core.display());
    }
    let path = core.to_str().context("Core path is not valid Unicode")?;
    let command = format!("\"{path}\" share --pause-on-error -- \"%1\"");
    for class in CLASSES {
        let (key, _) =
            HKCU.create_subkey(format!(r"Software\Classes\{class}\shell\LocalSendQR"))?;
        key.set_value("", &"通过二维码传送")?;
        key.set_value("Icon", &format!("\"{path}\",0"))?;
        key.set_value("MultiSelectModel", &"Single")?;
        let (command_key, _) = key.create_subkey("command")?;
        command_key.set_value("", &command)?;
        let actual: String = command_key.get_value("")?;
        if actual != command {
            bail!("Context menu verification failed for {class}");
        }
    }
    println!("Registered for the current user: {path}");
    println!("Windows 11: right-click > Show more options > 通过二维码传送");
    println!("Run Register-ContextMenu.cmd again after moving the portable folder.");
    Ok(())
}

pub fn remove() -> Result<()> {
    for class in CLASSES {
        let path = format!(r"Software\Classes\{class}\shell\LocalSendQR");
        match HKCU.delete_subkey_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("Cannot remove {path}")),
        }
    }
    println!("LocalSend QR context menu removed for the current user.");
    Ok(())
}
