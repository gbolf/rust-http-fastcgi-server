use std::{
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    BadEncoding,
    Forbidden,
    NotFound,
    Io(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadEncoding => write!(formatter, "URL path has invalid percent encoding"),
            Self::Forbidden => write!(formatter, "requested path is forbidden"),
            Self::NotFound => write!(formatter, "requested file was not found"),
            Self::Io(message) => write!(formatter, "filesystem error: {message}"),
        }
    }
}

impl std::error::Error for PathError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPath {
    pub filesystem_path: PathBuf,
    pub decoded_url_path: String,
}

pub fn canonicalize_document_root(path: &Path) -> Result<PathBuf, io::Error> {
    let canonical = fs::canonicalize(path)?;
    if !canonical.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "document root is not a directory",
        ));
    }
    Ok(canonical)
}

pub fn resolve_path(canonical_root: &Path, url_path: &str) -> Result<ResolvedPath, PathError> {
    let decoded = percent_decode(url_path)?;
    if !decoded.starts_with('/') || decoded.as_bytes().contains(&0) {
        return Err(PathError::Forbidden);
    }

    let relative_text = decoded.trim_start_matches('/');
    let relative_path = Path::new(relative_text);
    for component in relative_path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::Forbidden);
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    let mut candidate = canonical_root.join(relative_path);
    if decoded.ends_with('/') {
        candidate.push("index.html");
    }

    let canonical_target = match fs::canonicalize(candidate) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(PathError::NotFound),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Err(PathError::Forbidden)
        }
        Err(error) => return Err(PathError::Io(error.to_string())),
    };

    if !canonical_target.starts_with(canonical_root) {
        return Err(PathError::Forbidden);
    }
    let metadata = fs::metadata(&canonical_target).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => PathError::NotFound,
        io::ErrorKind::PermissionDenied => PathError::Forbidden,
        _ => PathError::Io(error.to_string()),
    })?;
    if !metadata.is_file() {
        return Err(PathError::NotFound);
    }

    Ok(ResolvedPath {
        filesystem_path: canonical_target,
        decoded_url_path: decoded,
    })
}

fn percent_decode(input: &str) -> Result<String, PathError> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(PathError::BadEncoding);
            }
            let high = hex_value(bytes[index + 1]).ok_or(PathError::BadEncoding)?;
            let low = hex_value(bytes[index + 2]).ok_or(PathError::BadEncoding)?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(output).map_err(|_| PathError::BadEncoding)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
