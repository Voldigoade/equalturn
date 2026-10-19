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
