//! Copy the complete LAN URL, including its per-window access credential.
use anyhow::{Result, bail};

#[cfg(windows)]
pub fn copy(text: &str) -> Result<()> {
    use std::ptr;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };

    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: allocate enough movable global memory for the NUL-terminated
    // UTF-16 string. Ownership transfers to Windows only on SetClipboardData
    // success; otherwise the allocation is freed here. Every successful open
    // is paired with CloseClipboard, using the console window as the owner.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>());
        if memory.is_null() {
            bail!("Cannot allocate clipboard text");
        }
        let destination = GlobalLock(memory).cast::<u16>();
        if destination.is_null() {
            GlobalFree(memory);
            bail!("Cannot access clipboard memory");
        }
        ptr::copy_nonoverlapping(wide.as_ptr(), destination, wide.len());
        GlobalUnlock(memory);
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(GetConsoleWindow()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            GlobalFree(memory);
            bail!("Clipboard is busy");
        }
        // CF_UNICODETEXT is the predefined clipboard format 13.
        let success = EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null();
        CloseClipboard();
        if !success {
            GlobalFree(memory);
            bail!("Cannot copy LAN URL");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn copy(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let mut programs: Vec<(&str, &[&str])> = Vec::new();
    if wayland {
        programs.push(("wl-copy", &["--type", "text/plain;charset=utf-8"]));
    }
    if x11 {
        programs.push(("xclip", &["-selection", "clipboard", "-in"]));
        programs.push(("xsel", &["--clipboard", "--input"]));
    }
    for (program, args) in programs {
        if let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            let written = child
                .stdin
                .take()
                .is_some_and(|mut input| input.write_all(text.as_bytes()).is_ok());
            if child.wait().is_ok_and(|status| status.success()) && written {
                return Ok(());
            }
        }
    }
    bail!("Clipboard unavailable; install wl-clipboard (Wayland), xclip or xsel (X11)")
}
