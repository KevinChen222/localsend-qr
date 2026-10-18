use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

pub struct SharedFile {
    pub path: PathBuf,
    pub name: String,
    // Keep the archive alive until the server has stopped.
    pub _temporary: Option<TempDir>,
}

pub fn prepare(paths: &[PathBuf]) -> Result<SharedFile> {
    let mut roots = Vec::new();
    let mut names = HashSet::new();
    for path in paths {
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("Cannot read {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("Symbolic links are not shared: {}", path.display());
        }
        let path = fs::canonicalize(path)?;
        let name = path
            .file_name()
            .context("Select a file or folder, not a filesystem root")?
            .to_str()
            .context("File name is not valid UTF-8")?
            .to_owned();
        if !names.insert(name.clone()) {
            bail!("Selected items have the same name: {name}. Share their parent folder instead.");
        }
        roots.push((path, name));
    }
    if roots.is_empty() {
        bail!("Select at least one file or folder");
    }
    if roots.len() == 1 && roots[0].0.is_file() {
        let (path, name) = roots.remove(0);
        File::open(&path).context("Cannot open the selected file")?;
        return Ok(SharedFile {
            path,
            name,
            _temporary: None,
        });
    }
    // Enumerate before creating the archive so selecting a temp folder cannot
    // cause the archive to include itself. Links and special files fail closed.
    let mut entries = Vec::new();
    for (root, name) in &roots {
        for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
            let entry = entry?;
            let kind = entry.file_type();
            if !kind.is_file() && !kind.is_dir() {
                bail!(
                    "Links and special files are not shared: {}",
                    entry.path().display()
                );
            }
            let relative = entry.path().strip_prefix(root)?;
            let mut archive_name = name.clone();
            for component in relative.components() {
                let part = component
                    .as_os_str()
                    .to_str()
                    .context("File name is not valid UTF-8")?;
                if part.contains('\\') {
                    bail!("ZIP name contains a backslash: {part}");
                }
                archive_name.push('/');
                archive_name.push_str(part);
            }
            if archive_name.contains('\\') {
                bail!("ZIP name contains a backslash");
            }
            entries.push((entry.path().to_path_buf(), archive_name, kind.is_dir()));
        }
    }
    let temporary = tempfile::Builder::new().prefix("localsend-qr-").tempdir()?;
    let path = temporary.path().join("share.zip");
    let mut archive = zip::ZipWriter::new(File::create(&path)?);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    for (source, name, is_dir) in entries {
        if is_dir {
            archive.add_directory(format!("{name}/"), options)?;
        } else {
            archive.start_file(&name, options)?;
            io::copy(
                &mut File::open(&source)
                    .with_context(|| format!("Cannot open {}", source.display()))?,
                &mut archive,
            )?;
        }
    }
    archive.finish()?;
    let name = if roots.len() == 1 {
        format!("{}.zip", roots[0].1)
    } else {
        "LocalSendQR-files.zip".into()
    };
    Ok(SharedFile {
        path,
        name,
        _temporary: Some(temporary),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zip_preserves_unicode_empty_folders_and_bytes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("测试 folder");
        fs::create_dir_all(root.join("empty"))?;
        fs::write(root.join("data.txt"), b"hello\0world")?;
        let shared = prepare(&[root])?;
        let archive_path = shared.path.clone();
        let mut zip = zip::ZipArchive::new(File::open(&shared.path)?)?;
        assert!(zip.by_name("测试 folder/empty/")?.is_dir());
        let mut bytes = Vec::new();
        io::Read::read_to_end(&mut zip.by_name("测试 folder/data.txt")?, &mut bytes)?;
        assert_eq!(bytes, b"hello\0world");
        drop(zip);
        drop(shared);
        assert!(!archive_path.exists());
        Ok(())
    }
    #[test]
    fn single_file_is_served_without_copy() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("test.txt");
        fs::write(&path, b"hello")?;
        let shared = prepare(std::slice::from_ref(&path))?;
        assert_eq!(shared.path, fs::canonicalize(&path)?);
        assert!(shared._temporary.is_none());
        drop(shared);
        assert!(path.exists());
        Ok(())
    }
    #[test]
    fn duplicate_root_names_are_rejected() -> Result<()> {
        let temp = tempfile::tempdir()?;
        fs::create_dir_all(temp.path().join("a"))?;
        fs::create_dir_all(temp.path().join("b"))?;
        let a = temp.path().join("a/test");
        let b = temp.path().join("b/test");
        fs::write(&a, b"a")?;
        fs::write(&b, b"b")?;
        assert!(prepare(&[a, b]).is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn links_do_not_expose_other_files() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("selected");
        fs::create_dir(&root)?;
        fs::write(temp.path().join("secret"), b"secret")?;
        std::os::unix::fs::symlink(temp.path().join("secret"), root.join("link"))?;
        assert!(prepare(&[root]).is_err());
        Ok(())
    }
}
