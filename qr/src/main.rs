mod archive;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use localsend::http::server::web::{
    WebConfig, WebDownloadConfig, WebDownloadEvent, WebMode, WebPages,
};
use localsend::http::server::{ServerHandle, start_with_port};
use localsend::http::state::ClientInfo;
use localsend::model::transfer::{FileContent, FileDto};
use qrcode::{QrCode, render::unicode};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

#[derive(Parser)]
#[command(
    version,
    about = "Share files to a phone browser over your LAN using a terminal QR code"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Share a file, a folder (ZIP), or multiple items (one ZIP)
    Share {
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
        /// TCP port; 0 chooses a free port. Default: settings.json or 53318
        #[arg(long)]
        port: Option<u16>,
        /// QR address when there are multiple network adapters
        #[arg(long)]
        ip: Option<Ipv4Addr>,
        /// Stop after this many minutes; 0 disables the timer
        #[arg(long)]
        expires: Option<u32>,
        /// Keep an Explorer-launched console open on errors
        #[arg(long)]
        pause_on_error: bool,
    },
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    port: Option<u16>,
    ip: Option<Ipv4Addr>,
    expires_minutes: Option<u32>,
}

fn qr_colors_enabled() -> bool {
    if !io::stdout().is_terminal() {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE,
            SetConsoleMode,
        };
        // SAFETY: the standard output handle is borrowed, and mode points to
        // initialized writable storage. No handle ownership is transferred.
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return false;
            }
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn settings() -> Result<Settings> {
    let path = std::env::current_exe()?
        .parent()
        .context("Executable has no directory")?
        .join("settings.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("Invalid {}", path.display()))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(err).with_context(|| format!("Cannot read {}", path.display())),
    }
}

fn main() {
    let Args {
        command:
            Command::Share {
                paths,
                port,
                ip,
                expires,
                pause_on_error,
            },
    } = Args::parse();
    let result = tokio::runtime::Runtime::new()
        .context("Cannot start runtime")
        .and_then(|runtime| {
            runtime.block_on(async {
                let config = settings()?;
                share(
                    paths,
                    port.or(config.port).unwrap_or(53318),
                    ip.or(config.ip),
                    expires.or(config.expires_minutes).unwrap_or(60),
                )
                .await
            })
        });
    if let Err(error) = result {
        eprintln!("\nError: {error:#}");
        if pause_on_error {
            eprint!("Press Enter to close...");
            let _ = io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}

struct RunningShare {
    server: ServerHandle,
    stop: oneshot::Sender<()>,
    events: tokio::task::JoinHandle<()>,
}

async fn start(file: &archive::SharedFile, port: u16, secret: &str) -> Result<RunningShare> {
    let file_id = uuid::Uuid::new_v4().to_string();
    let files = HashMap::from([(
        file_id.clone(),
        FileDto {
            id: file_id.clone(),
            file_name: file.name.clone(),
            size: std::fs::metadata(&file.path)?.len(),
            file_type: "application/octet-stream".into(),
            sha256: None,
            preview: None,
            metadata: None,
        },
    )]);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let (stop, stop_rx) = oneshot::channel();
    let server = start_with_port(
        port,
        None,
        ClientInfo {
            alias: "LocalSend QR".into(),
            version: "2.2".into(),
            device_model: Some(std::env::consts::OS.into()),
            device_type: None,
            token: uuid::Uuid::new_v4().to_string(),
        },
        None,
        None,
        WebConfig {
            mode: WebMode::Download(WebDownloadConfig {
                files,
                pin: Some(secret.into()),
                event_tx,
            }),
            pages: WebPages {
                download_html: Some(include_str!("../assets/download.html").into()),
                ..WebPages::default()
            },
            ..WebConfig::default()
        },
        stop_rx,
    )
    .await
    .with_context(|| {
        format!("Cannot listen on port {port}. Close another sharing window or use --port 0")
    })?;
    let path = file.path.clone();
    let events = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                WebDownloadEvent::PrepareDownload { decision_tx, .. } => {
                    let _ = decision_tx.send(true);
                }
                WebDownloadEvent::FileDownload {
                    file_id: requested,
                    content_tx,
                    ..
                } if requested == file_id => {
                    println!(
                        "Download requested by a browser (this does not confirm it has finished)."
                    );
                    let _ = content_tx.send(FileContent::Path(path.clone()));
                }
                _ => {}
            }
        }
    });
    Ok(RunningShare {
        server,
        stop,
        events,
    })
}

async fn share(paths: Vec<PathBuf>, port: u16, ip: Option<Ipv4Addr>, expires: u32) -> Result<()> {
    println!("Preparing selected files; folders and multiple items become one ZIP...");
    io::stdout().flush()?;
    let file = tokio::task::spawn_blocking(move || archive::prepare(&paths)).await??;
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let running = start(&file, port, &secret).await?;
    let mut addresses: Vec<Ipv4Addr> = running
        .server
        .local_addresses()
        .into_iter()
        .filter_map(|address| match address.ip() {
            std::net::IpAddr::V4(ip) => Some(ip),
            _ => None,
        })
        .collect();
    addresses.sort_by_key(|address| (!address.is_private(), address.octets()));
    addresses.dedup();
    let selected = if let Some(ip) = ip {
        if !addresses.contains(&ip) {
            let _ = running.stop.send(());
            running.server.wait_stopped().await;
            running.events.abort();
            bail!("--ip {ip} is not an active non-loopback IPv4 address on this computer");
        }
        ip
    } else {
        match addresses.as_slice() {
            [] => {
                let _ = running.stop.send(());
                running.server.wait_stopped().await;
                running.events.abort();
                bail!("No LAN IPv4 address found. Connect to Wi-Fi or Ethernet first");
            }
            [one] => *one,
            many => {
                println!("Choose the Wi-Fi/Ethernet address reachable by your phone:");
                for (index, address) in many.iter().enumerate() {
                    println!("  {}: {}", index + 1, address);
                }
                print!("Address [1]: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let index = if input.trim().is_empty() {
                    1
                } else {
                    input
                        .trim()
                        .parse::<usize>()
                        .context("Enter an address number")?
                };
                *many
                    .get(index.wrapping_sub(1))
                    .context("Address number is out of range")?
            }
        }
    };
    let url = format!("http://{selected}:{}/#key={secret}", running.server.port());
    // Force a light quiet zone and dark modules, independent of terminal theme.
    let qr = QrCode::new(url.as_bytes())?
        .render::<unicode::Dense1x2>()
        .quiet_zone(true)
        .build();
    let qr = if qr_colors_enabled() {
        format!(
            "\x1b[30;107m{}\x1b[0m",
            qr.replace('\n', "\x1b[0m\n\x1b[30;107m")
        )
    } else {
        qr
    };
    let display_name: String = file
        .name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    println!("\nLocalSend QR | {display_name}\n\n{qr}\n\n{url}\n");
    println!("Scan with the phone camera and open in Safari/Chrome. No phone app needed.");
    println!(
        "HTTP / unencrypted. Share only on a trusted LAN. Anyone with this link can download."
    );
    if expires > 0 {
        println!("Sharing expires in {expires} minutes. Use --expires 0 for no timer.");
    }
    println!(
        "Keep this window open. Ctrl+C stops sharing. Downloads may need a browser confirmation."
    );
    io::stdout().flush()?;
    let timer = async {
        if expires == 0 {
            std::future::pending::<()>().await;
        } else {
            tokio::time::sleep(Duration::from_secs(u64::from(expires) * 60)).await;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => println!("\nStopping sharing..."),
        _ = timer => println!("\nShare expired; stopping active downloads..."),
    }
    let _ = running.stop.send(());
    running.server.wait_stopped().await;
    running.events.abort();
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn browser_flow_requires_key_and_streams_exact_bytes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("中文 test.txt");
        std::fs::write(&path, b"real file contents\0")?;
        let file = archive::prepare(&[path])?;
        let running = start(&file, 0, "secret-test-key").await?;
        let base = format!("http://127.0.0.1:{}", running.server.port());
        let client = localsend::reqwest::Client::builder().no_proxy().build()?;
        assert!(
            client
                .get(&base)
                .send()
                .await?
                .text()
                .await?
                .contains("location.hash")
        );
        let prepare = format!("{base}/api/localsend/v2/prepare-download");
        assert_eq!(client.post(&prepare).send().await?.status().as_u16(), 401);
        assert_eq!(
            client
                .post(format!("{prepare}?pin=wrong"))
                .send()
                .await?
                .status()
                .as_u16(),
            401
        );
        let response: serde_json::Value = client
            .post(format!("{prepare}?pin=secret-test-key"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let session = response["sessionId"].as_str().unwrap();
        let files = response["files"].as_object().unwrap();
        assert_eq!(files.len(), 1);
        let id = files.keys().next().unwrap();
        let mut download_url =
            localsend::reqwest::Url::parse(&format!("{base}/api/localsend/v2/download"))?;
        download_url
            .query_pairs_mut()
            .append_pair("sessionId", session)
            .append_pair("fileId", id);
        let response = client.get(download_url).send().await?.error_for_status()?;
        assert!(
            response.headers()["content-disposition"]
                .to_str()?
                .contains("filename*=UTF-8''%E4%B8%AD%E6%96%87%20test%2Etxt")
        );
        assert_eq!(response.bytes().await?.as_ref(), b"real file contents\0");
        assert_eq!(
            client
                .get(format!(
                    "{base}/api/localsend/v2/download?sessionId=bad&fileId={id}"
                ))
                .send()
                .await?
                .status()
                .as_u16(),
            403
        );
        assert_eq!(
            client
                .post(format!("{base}/api/localsend/v2/prepare-upload"))
                .send()
                .await?
                .status()
                .as_u16(),
            403
        );
        let port = running.server.port();
        let _ = running.stop.send(());
        running.server.wait_stopped().await;
        running.events.abort();
        assert!(
            tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_err()
        );
        Ok(())
    }
}
