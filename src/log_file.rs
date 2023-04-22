use std::{
    error::Error,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub(crate) struct LogFile(PathBuf);

impl AsRef<Path> for LogFile {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Default for LogFile {
    fn default() -> Self {
        use std::env::temp_dir;
        use std::time::{SystemTime, UNIX_EPOCH};

        let start = SystemTime::now();
        let dir = temp_dir();
        let filename = format!(
            "git-disjoint-{:?}",
            start.duration_since(UNIX_EPOCH).unwrap()
        );
        Self(dir.join(filename))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct DeleteError {
    path: PathBuf,
    kind: DeleteErrorKind,
}

impl Display for DeleteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            DeleteErrorKind::Delete(_) => write!(f, "unable to delete file {:?}", self.path),
        }
    }
}

impl Error for DeleteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DeleteErrorKind::Delete(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum DeleteErrorKind {
    #[non_exhaustive]
    Delete(io::Error),
}

impl LogFile {
    pub fn delete(self) -> Result<(), DeleteError> {
        if self.0.exists() {
            return Ok(fs::remove_file(&self.0).map_err(|err| DeleteError {
                path: self.0,
                kind: DeleteErrorKind::Delete(err),
            })?);
        }
        Ok(())
    }
}
