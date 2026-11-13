//! Retry unavailable preferred ports without probing and releasing a socket.
use anyhow::{Context, Result};
use std::future::Future;
use std::io;

pub async fn start<T, F, Fut>(port: u16, mut start: F) -> Result<T>
where
    F: FnMut(u16) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    match start(port).await {
        Err(error)
            if port != 0
                && error.downcast_ref::<io::Error>().is_some_and(|error| {
                    matches!(
                        error.kind(),
                        io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied
                    )
                }) =>
        {
            // Windows excluded port ranges report PermissionDenied (10013).
            // Let the OS bind an available port and keep the listener alive.
            start(0)
                .await
                .context("Cannot start on an available TCP port")
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retries_only_unavailable_fixed_ports() {
        for (port, kind, expected) in [
            (53318, io::ErrorKind::PermissionDenied, vec![53318, 0]),
            (53318, io::ErrorKind::AddrInUse, vec![53318, 0]),
            (53318, io::ErrorKind::InvalidInput, vec![53318]),
            (0, io::ErrorKind::PermissionDenied, vec![0]),
        ] {
            let mut attempts = Vec::new();
            let result: Result<()> = start(port, |candidate| {
                attempts.push(candidate);
                std::future::ready(Err(
                    anyhow::Error::new(io::Error::from(kind)).context("Listener bind failed")
                ))
            })
            .await;
            assert!(result.is_err());
            assert_eq!(attempts, expected);
        }
    }
}
