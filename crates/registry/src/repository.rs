use sigstore_tuf::{
    Error as TufError,
    transport::{FetchFuture, Repository},
};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub(crate) struct DirectoryRepository {
    root: PathBuf,
}

impl DirectoryRepository {
    pub(crate) fn open(root: &Path) -> Result<Self, TufError> {
        let metadata = fs::symlink_metadata(root)
            .map_err(|_| TufError::transport("local repository is unavailable"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(TufError::transport(
                "local repository must be a real directory",
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    fn read_file(
        &self,
        area: &str,
        relative: &str,
        max_length: u64,
    ) -> Result<Option<Vec<u8>>, TufError> {
        if !safe_relative_path(relative) {
            return Err(TufError::transport("unsafe repository path"));
        }
        let mut path = self.root.clone();
        path.push(area);
        let area_metadata = fs::symlink_metadata(&path)
            .map_err(|_| TufError::transport("repository area is unavailable"))?;
        if area_metadata.file_type().is_symlink() || !area_metadata.is_dir() {
            return Err(TufError::transport(
                "repository area must be a real directory",
            ));
        }
        let components: Vec<_> = Path::new(relative).components().collect();
        if components.is_empty() {
            return Err(TufError::transport("repository path is empty"));
        }
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(TufError::transport("unsafe repository path"));
            };
            path.push(name);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => return Err(TufError::transport("repository file cannot be inspected")),
            };
            if metadata.file_type().is_symlink() {
                return Err(TufError::transport("repository symlinks are not accepted"));
            }
            let is_last = index + 1 == components.len();
            if is_last && !metadata.is_file() || !is_last && !metadata.is_dir() {
                return Err(TufError::transport(
                    "repository path has an unexpected file type",
                ));
            }
            if is_last && metadata.len() > max_length {
                return Err(TufError::transport(
                    "repository file exceeds its configured size limit",
                ));
            }
        }
        let file = fs::File::open(&path)
            .map_err(|_| TufError::transport("repository file cannot be opened"))?;
        let opened = file
            .metadata()
            .map_err(|_| TufError::transport("repository file metadata is unavailable"))?;
        if !opened.is_file() || opened.len() > max_length {
            return Err(TufError::transport(
                "repository file exceeds its configured size limit",
            ));
        }
        let bounded = max_length.saturating_add(1);
        let mut bytes = Vec::with_capacity(opened.len().min(max_length) as usize);
        file.take(bounded)
            .read_to_end(&mut bytes)
            .map_err(|_| TufError::transport("repository file cannot be read"))?;
        if bytes.len() as u64 > max_length {
            return Err(TufError::transport(
                "repository file exceeds its configured size limit",
            ));
        }
        Ok(Some(bytes))
    }
}

impl Repository for DirectoryRepository {
    fn fetch_metadata<'a>(&'a self, name: &'a str, max_length: u64) -> FetchFuture<'a> {
        let result = if safe_component(name) {
            self.read_file("metadata", name, max_length)
        } else {
            Err(TufError::transport("unsafe metadata filename"))
        };
        Box::pin(async move { result })
    }

    fn fetch_target<'a>(&'a self, path: &'a str, max_length: u64) -> FetchFuture<'a> {
        let result = self.read_file("targets", path, max_length);
        Box::pin(async move { result })
    }
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.is_ascii()
        && value.split('/').all(safe_component)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn repository_root() -> tempfile::TempDir {
        let root = tempdir().expect("repository root is created");
        fs::create_dir(root.path().join("targets")).expect("targets directory is created");
        root
    }

    #[tokio::test]
    async fn rejects_unsafe_target_paths() {
        let root = repository_root();
        let repository = DirectoryRepository::open(root.path()).expect("repository opens");

        for path in [
            "../outside",
            "/absolute",
            "nested/../../outside",
            "C:\\outside",
            "",
        ] {
            assert!(
                repository.fetch_target(path, 1024).await.is_err(),
                "unsafe target path {path:?} must be rejected"
            );
        }
    }

    #[tokio::test]
    async fn rejects_symlink_targets() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let root = repository_root();
            let outside = root.path().join("outside");
            fs::write(&outside, b"external target").expect("outside target is written");
            symlink(&outside, root.path().join("targets/registry-v3.json"))
                .expect("target symlink is created");
            let repository = DirectoryRepository::open(root.path()).expect("repository opens");

            assert!(
                repository
                    .fetch_target("registry-v3.json", 1024)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn rejects_symlink_repository_root_and_area() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let parent = tempdir().expect("test parent is created");
            let actual_root = parent.path().join("actual-root");
            fs::create_dir(&actual_root).expect("actual repository root is created");
            fs::create_dir(actual_root.join("targets")).expect("targets directory is created");
            let root_link = parent.path().join("root-link");
            symlink(&actual_root, &root_link).expect("root symlink is created");
            assert!(DirectoryRepository::open(&root_link).is_err());

            let repository_root = parent.path().join("repository");
            fs::create_dir(&repository_root).expect("repository root is created");
            fs::create_dir(repository_root.join("real-targets"))
                .expect("real target directory is created");
            symlink(
                repository_root.join("real-targets"),
                repository_root.join("targets"),
            )
            .expect("targets area symlink is created");
            let repository = DirectoryRepository::open(&repository_root).expect("root opens");

            assert!(
                repository
                    .fetch_target("registry-v3.json", 1024)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn rejects_symlink_metadata_files() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let root = repository_root();
            fs::create_dir(root.path().join("metadata")).expect("metadata directory is created");
            let outside = root.path().join("outside-metadata");
            fs::write(&outside, b"external metadata").expect("outside metadata is written");
            symlink(&outside, root.path().join("metadata/timestamp.json"))
                .expect("metadata symlink is created");
            let repository = DirectoryRepository::open(root.path()).expect("repository opens");

            assert!(
                repository
                    .fetch_metadata("timestamp.json", 1024)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn rejects_targets_over_the_requested_limit() {
        let root = repository_root();
        fs::write(root.path().join("targets/registry-v3.json"), b"four")
            .expect("target is written");
        let repository = DirectoryRepository::open(root.path()).expect("repository opens");

        assert!(
            repository
                .fetch_target("registry-v3.json", 3)
                .await
                .is_err()
        );
    }
}
