//! Browser uploads use the core protocol; only save policy lives here.
use anyhow::{Context, Result, bail};
use bytes::Bytes;
use localsend::http::server::common::save::FileUploadTarget;
use localsend::http::server::v2::{PrepareUploadDecisionV2, ServerEventV2};
use localsend::http::server::web::{WebConfig, WebMode, WebPages};
use localsend::http::server::{ServerConfigV2, start_with_port};
use localsend::http::state::ClientInfo;
use localsend::model::transfer::FileDto;
use sha2::{Digest, Sha256};
use std::io;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

pub async fn receive(
    directory: PathBuf,
    port: u16,
    ip: Option<Ipv4Addr>,
    expires: u32,
    print_url: bool,
) -> Result<()> {
    let directory = std::fs::canonicalize(&directory)
        .with_context(|| format!("Cannot open receive folder: {}", directory.display()))?;
    if !directory.is_dir() {
        bail!("Select a folder to receive files");
    }
    // Check write access before displaying a QR code.
    drop(tempfile::NamedTempFile::new_in(&directory).context("Receive folder is not writable")?);
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let running = start(directory.clone(), port, &secret).await?;
    super::run_window(running, &secret, ip, expires, true, print_url).await
}

pub(super) async fn start(
    directory: PathBuf,
    port: u16,
    secret: &str,
) -> Result<super::RunningShare> {
    super::port::start(port, |port| start_on_port(directory.clone(), port, secret)).await
}

async fn start_on_port(directory: PathBuf, port: u16, secret: &str) -> Result<super::RunningShare> {
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
        Some(ServerConfigV2 {
            pin: Some(secret.into()),
            verify_checksums: true,
            event_tx,
        }),
        WebConfig {
            mode: WebMode::Upload,
            pages: WebPages {
                upload_html: Some(include_str!("../assets/upload.html").into()),
                ..WebPages::default()
            },
            ..WebConfig::default()
        },
        stop_rx,
    )
    .await
    .with_context(|| format!("Cannot listen on TCP port {port}"))?;
    let events = tokio::spawn(async move {
        // Aborting the event loop also drops its upload tasks and temporary files.
        let mut uploads = JoinSet::new();
        let received = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        loop {
            tokio::select! {
                Some(_) = uploads.join_next(), if !uploads.is_empty() => {},
                event = event_rx.recv() => match event {
                    Some(ServerEventV2::PrepareUpload { files, decision_tx, .. }) => {
                        let decision = if files.values().all(|file| valid_name(&file.file_name)) {
                            PrepareUploadDecisionV2::Accept(files.into_keys().collect())
                        } else {
                            PrepareUploadDecisionV2::Decline
                        };
                        let _ = decision_tx.send(decision);
                    }
                    Some(ServerEventV2::FileUpload { file, target_tx, .. }) => {
                        let (binary_tx, binary_rx) = mpsc::channel(16);
                        let (result_tx, result_rx) = oneshot::channel();
                        if target_tx.send(FileUploadTarget::Stream { binary_tx, result_rx }).is_ok() {
                            let directory = directory.clone();
                            let received = received.clone();
                            uploads.spawn(async move {
                                let result = save(&directory, &file, binary_rx).await;
                                match &result {
                                    Ok(_) => {
                                        let count = received.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                                        super::set_status(&format!("已接收 {count} 个文件 · 可继续发送 · Ctrl+C 关闭"));
                                    }
                                    Err(_) => super::set_status("接收失败，请在手机上重试 · Ctrl+C 关闭"),
                                }
                                // HTTP 200 is sent only after the final file has been saved.
                                let _ = result_tx.send(result.map(|_| ()).map_err(|error| format!("{error:#}")));
                            });
                        }
                    }
                    Some(ServerEventV2::ListenerFailed { error }) => {
                        eprintln!("Receive server stopped: {error}");
                        break;
                    }
                    Some(_) => {},
                    None => break,
                }
            }
        }
    });
    Ok(super::RunningShare {
        server,
        stop,
        events,
    })
}

// Apply Windows filename restrictions on both systems so uploads are portable.
fn valid_name(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 200
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return false;
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

async fn save(
    directory: &Path,
    metadata: &FileDto,
    mut chunks: mpsc::Receiver<Bytes>,
) -> Result<PathBuf> {
    if !valid_name(&metadata.file_name) {
        bail!("Invalid upload filename");
    }
    let temporary = tempfile::Builder::new()
        .prefix(".localsend-qr-")
        .suffix(".part")
        .tempfile_in(directory)?;
    let mut writer = tokio::fs::File::from_std(temporary.as_file().try_clone()?);
    let mut received = 0u64;
    let mut checksum = metadata.sha256.as_ref().map(|_| Sha256::new());
    while let Some(chunk) = chunks.recv().await {
        received = received
            .checked_add(chunk.len() as u64)
            .context("Upload is too large")?;
        if received > metadata.size {
            bail!("Upload exceeds the declared file size");
        }
        if let Some(checksum) = &mut checksum {
            checksum.update(&chunk);
        }
        writer.write_all(&chunk).await?;
    }
    if received != metadata.size {
        bail!(
            "Upload interrupted: expected {} bytes, received {received}",
            metadata.size
        );
    }
    if let (Some(checksum), Some(expected)) = (checksum, &metadata.sha256) {
        let actual: String = checksum
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("Checksum mismatch");
        }
    }
    writer.flush().await?;
    writer.sync_all().await?;
    drop(writer);
    persist_unique(temporary, directory, &metadata.file_name)
}

fn persist_unique(
    mut temporary: tempfile::NamedTempFile,
    directory: &Path,
    name: &str,
) -> Result<PathBuf> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .context("Missing filename")?
        .to_string_lossy();
    let extension = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 0..10_000 {
        let name = if index == 0 {
            name.to_owned()
        } else {
            format!("{stem} ({index}){extension}")
        };
        let target = directory.join(name);
        match temporary.persist_noclobber(&target) {
            Ok(_) => return Ok(target),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                temporary = error.file
            }
            Err(error) => return Err(error.error).context("Cannot save received file"),
        }
    }
    bail!("Too many files with the same name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use localsend::reqwest::Client;
    use serde_json::{Value, json};

    fn offer(name: &str, size: usize) -> Value {
        json!({
            "info": {"alias":"phone", "version":"2.2", "fingerprint":"browser", "port":0, "protocol":"http"},
            "files": {"one": {"id":"one", "fileName":name, "size":size, "fileType":"application/octet-stream"}}
        })
    }

    async fn prepare(client: &Client, base: &str, files: &Value) -> Result<Value> {
        Ok(client
            .post(format!(
                "{base}/api/localsend/v2/prepare-upload?pin=test-key"
            ))
            .json(files)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    fn upload_url(base: &str, session: &Value, id: &str) -> String {
        format!(
            "{base}/api/localsend/v2/upload?sessionId={}&fileId={id}&token={}",
            session["sessionId"].as_str().unwrap(),
            session["files"][id].as_str().unwrap()
        )
    }

    #[tokio::test]
    async fn browser_upload_auth_multiple_files_collisions_and_failures() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let occupied = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
        let preferred_port = occupied.local_addr()?.port();
        let running = start(temp.path().into(), preferred_port, "test-key").await?;
        assert_ne!(running.server.port(), preferred_port);
        let base = format!("http://127.0.0.1:{}", running.server.port());
        let client = Client::builder().no_proxy().build()?;
        let page = client.get(&base).send().await?.text().await?;
        assert!(page.contains("location.hash"));
        assert!(page.contains("XMLHttpRequest"));
        let endpoint = format!("{base}/api/localsend/v2/prepare-upload");
        for query in ["", "?pin=wrong"] {
            assert_eq!(
                client
                    .post(format!("{endpoint}{query}"))
                    .json(&offer("a.txt", 3))
                    .send()
                    .await?
                    .status()
                    .as_u16(),
                401
            );
        }
        for name in [
            "../escape.txt",
            "a/b",
            "a\\b",
            "C:escape",
            "NUL.txt",
            "trailing.",
        ] {
            assert_eq!(
                client
                    .post(format!("{endpoint}?pin=test-key"))
                    .json(&offer(name, 3))
                    .send()
                    .await?
                    .status()
                    .as_u16(),
                403,
                "{name}"
            );
        }
        assert_eq!(std::fs::read_dir(temp.path())?.count(), 0);
        std::fs::write(temp.path().join("中文 file.txt"), b"original")?;
        let bytes: Vec<u8> = (0..1_048_576).map(|n| (n % 256) as u8).collect();
        let mut files = offer("中文 file.txt", bytes.len());
        files["files"]["two"] =
            json!({"id":"two", "fileName":"empty.txt", "size":0, "fileType":"text/plain"});
        let session = prepare(&client, &base, &files).await?;
        assert_eq!(
            client
                .post(format!("{endpoint}?pin=test-key"))
                .json(&offer("busy", 0))
                .send()
                .await?
                .status()
                .as_u16(),
            409
        );
        assert_eq!(
            client
                .post(format!(
                    "{base}/api/localsend/v2/upload?sessionId=bad&fileId=one&token=bad"
                ))
                .body(Vec::new())
                .send()
                .await?
                .status()
                .as_u16(),
            403
        );
        client
            .post(upload_url(&base, &session, "one"))
            .body(bytes.clone())
            .send()
            .await?
            .error_for_status()?;
        client
            .post(upload_url(&base, &session, "two"))
            .body(Vec::new())
            .send()
            .await?
            .error_for_status()?;
        // HTTP success means the final file is already present, not merely buffered.
        assert_eq!(
            std::fs::read(temp.path().join("中文 file.txt"))?,
            b"original"
        );
        assert_eq!(std::fs::read(temp.path().join("中文 file (1).txt"))?, bytes);
        assert_eq!(std::fs::read(temp.path().join("empty.txt"))?, b"");
        for (name, size, body) in [("short.txt", 10, "short"), ("long.txt", 1, "too long")] {
            let session = prepare(&client, &base, &offer(name, size)).await?;
            assert!(
                !client
                    .post(upload_url(&base, &session, "one"))
                    .body(body)
                    .send()
                    .await?
                    .status()
                    .is_success()
            );
            assert!(!temp.path().join(name).exists());
        }
        let mut checksum = offer("bad-hash.txt", 3);
        checksum["files"]["one"]["sha256"] = json!("00".repeat(32));
        let session = prepare(&client, &base, &checksum).await?;
        assert!(
            !client
                .post(upload_url(&base, &session, "one"))
                .body("abc")
                .send()
                .await?
                .status()
                .is_success()
        );
        assert!(!temp.path().join("bad-hash.txt").exists());
        let session = prepare(&client, &base, &offer("cancelled.txt", 3)).await?;
        client
            .post(format!(
                "{base}/api/localsend/v2/cancel?sessionId={}",
                session["sessionId"].as_str().unwrap()
            ))
            .send()
            .await?
            .error_for_status()?;
        assert!(
            !client
                .post(upload_url(&base, &session, "one"))
                .body("abc")
                .send()
                .await?
                .status()
                .is_success()
        );
        // A new session is possible after both errors and cancellation.
        let session = prepare(&client, &base, &offer("again.txt", 3)).await?;
        client
            .post(upload_url(&base, &session, "one"))
            .body("abc")
            .send()
            .await?
            .error_for_status()?;
        assert_eq!(std::fs::read(temp.path().join("again.txt"))?, b"abc");
        assert_eq!(std::fs::read_dir(temp.path())?.count(), 4);
        let _ = running.stop.send(());
        running.server.wait_stopped().await;
        running.events.abort();
        let _ = running.events.await;
        Ok(())
    }

    #[test]
    fn filenames_are_safe_on_windows_and_linux() {
        for name in [
            "../x",
            "/tmp/x",
            "..",
            ".",
            "",
            "CON",
            "COM1.txt",
            "LPT².txt",
            "a:b",
            "a\0b",
            "bad?",
            "x ",
        ] {
            assert!(!valid_name(name), "{name:?}");
        }
        for name in [
            "中文 空格.txt",
            "photo.jpg",
            "a 'quoted'.txt",
            ".hidden",
            "COM10.txt",
        ] {
            assert!(valid_name(name), "{name}");
        }
    }
}
