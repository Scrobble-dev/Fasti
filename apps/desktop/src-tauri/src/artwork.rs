use crate::network_config::replace_file;
use crate::setup::DesktopProblem;
use fasti_application::{NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy};
use fasti_provider_runtime::{
    bounded_body, GovernedTransport, ProviderCandidate, GOOGLE_BOOKS_PROVIDER, TMDB_PROVIDER,
};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

const ARTWORK_CAPABILITY: &str = "metadata.artwork";
const TMDB_IMAGE_HOST: &str = "image.tmdb.org";
const GOOGLE_IMAGE_HOSTS: &[&str] = &["books.google.com", "books.googleusercontent.com"];
const ARTWORK_LIMIT: usize = 2_000_000;
const ARTWORK_HEADER_LIMIT: u64 = 64 * 1024;
const ARTWORK_DIMENSION_LIMIT: u32 = 4096;
const ARTWORK_PIXEL_LIMIT: u64 = 16_000_000;
const CACHE_FILE_LIMIT: usize = 128;
static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

const TMDB_ARTWORK_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: TMDB_PROVIDER,
    capabilities: &[ARTWORK_CAPABILITY],
    hosts: &[TMDB_IMAGE_HOST],
    networks: &[NetworkClass::Public],
};

const GOOGLE_ARTWORK_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: GOOGLE_BOOKS_PROVIDER,
    capabilities: &[ARTWORK_CAPABILITY],
    hosts: GOOGLE_IMAGE_HOSTS,
    networks: &[NetworkClass::Public],
};

struct ArtworkTarget {
    url: reqwest::Url,
    host: &'static str,
    access: OutboundAccessDeclaration<'static>,
}

pub(crate) struct ArtworkCache {
    root: PathBuf,
    gate: Mutex<()>,
}

impl ArtworkCache {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            gate: Mutex::new(()),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn prepare(&self) -> Result<(), DesktopProblem> {
        ensure_private_directory(&self.root)
    }

    pub(crate) async fn cache_candidate(
        &self,
        candidate: &ProviderCandidate,
        policy: &OutboundAccessPolicy,
        transport: &GovernedTransport,
    ) -> Result<(), DesktopProblem> {
        let Some(url) = candidate.image_url.as_deref() else {
            return Ok(());
        };
        let target = artwork_target(candidate.provider, url)?;
        let client = transport
            .authorize(target.access, policy, ARTWORK_CAPABILITY, &target.url)
            .await
            .map_err(DesktopProblem::provider)?;
        let response = client
            .get(target.url.clone())
            .map_err(DesktopProblem::provider)?
            .header(ACCEPT, "image/jpeg, image/png, image/webp")
            .send()
            .await
            .map_err(|_| DesktopProblem::provider("The provider artwork could not be reached."))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(DesktopProblem::provider(format!(
                "The provider artwork returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .unwrap_or_default();
        if !["image/jpeg", "image/png", "image/webp"]
            .iter()
            .any(|expected| content_type.eq_ignore_ascii_case(expected))
        {
            return Err(DesktopProblem::provider(
                "The provider artwork returned an unsupported content type.",
            ));
        }
        let body = bounded_body(response, ARTWORK_LIMIT)
            .await
            .map_err(DesktopProblem::provider)?;
        if !has_safe_image_dimensions(&body) {
            return Err(DesktopProblem::provider(
                "The provider artwork did not contain a supported image.",
            ));
        }
        self.store(candidate.provider, url, &body)
    }

    pub(crate) fn local_path(&self, provider: &str, url: &str) -> Option<String> {
        artwork_target(provider, url).ok()?;
        let path = self.path_for(provider, url);
        let metadata = fs::symlink_metadata(&path).ok()?;
        if !metadata.file_type().is_file()
            || metadata.len() == 0
            || metadata.len() > ARTWORK_LIMIT as u64
        {
            return None;
        }
        let mut prefix = Vec::new();
        File::open(&path)
            .ok()?
            .take(ARTWORK_HEADER_LIMIT)
            .read_to_end(&mut prefix)
            .ok()?;
        if !has_safe_image_dimensions(&prefix) {
            return None;
        }
        path.to_str().map(ToOwned::to_owned)
    }

    fn store(&self, provider: &str, url: &str, body: &[u8]) -> Result<(), DesktopProblem> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The artwork cache lock is unavailable."))?;
        ensure_private_directory(&self.root)?;
        let destination = self.path_for(provider, url);
        let temporary = self.root.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_file_create_permissions(&mut options);
        let mut file = options
            .open(&temporary)
            .map_err(|_| DesktopProblem::storage("Fasti could not stage the provider artwork."))?;
        let write_result = file.write_all(body).and_then(|()| file.sync_all());
        drop(file);
        if write_result.is_err() || replace_file(&temporary, &destination).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(DesktopProblem::storage(
                "Fasti could not save the provider artwork.",
            ));
        }
        set_file_permissions(&destination)?;
        // The artwork is already committed; cache cleanup must not turn that success into a failure.
        let _ = prune_cache(&self.root, &destination);
        Ok(())
    }

    fn path_for(&self, provider: &str, url: &str) -> PathBuf {
        let mut digest = Sha256::new();
        digest.update(provider.as_bytes());
        digest.update([0]);
        digest.update(url.as_bytes());
        let digest_bytes: [u8; 32] = digest.finalize().into();
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(64);
        for byte in digest_bytes {
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        self.root.join(value)
    }
}

fn artwork_target(provider: &str, value: &str) -> Result<ArtworkTarget, DesktopProblem> {
    if value.len() > 2048 {
        return Err(unsafe_artwork_url());
    }
    let url = reqwest::Url::parse(value).map_err(|_| unsafe_artwork_url())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(unsafe_artwork_url());
    }
    let parsed_host = url.host_str().ok_or_else(unsafe_artwork_url)?;
    let (host, access) = match provider {
        TMDB_PROVIDER
            if parsed_host == TMDB_IMAGE_HOST
                && url.path().starts_with("/t/p/w500/")
                && url.query().is_none() =>
        {
            (TMDB_IMAGE_HOST, TMDB_ARTWORK_ACCESS)
        }
        GOOGLE_BOOKS_PROVIDER if parsed_host == GOOGLE_IMAGE_HOSTS[0] => {
            (GOOGLE_IMAGE_HOSTS[0], GOOGLE_ARTWORK_ACCESS)
        }
        GOOGLE_BOOKS_PROVIDER if parsed_host == GOOGLE_IMAGE_HOSTS[1] => {
            (GOOGLE_IMAGE_HOSTS[1], GOOGLE_ARTWORK_ACCESS)
        }
        _ => return Err(unsafe_artwork_url()),
    };
    Ok(ArtworkTarget { url, host, access })
}

fn unsafe_artwork_url() -> DesktopProblem {
    DesktopProblem::provider("The provider returned an unsafe artwork URL.")
}

fn has_safe_image_dimensions(bytes: &[u8]) -> bool {
    image_dimensions(bytes).is_some_and(|(width, height)| {
        width > 0
            && height > 0
            && width <= ARTWORK_DIMENSION_LIMIT
            && height <= ARTWORK_DIMENSION_LIMIT
            && u64::from(width) * u64::from(height) <= ARTWORK_PIXEL_LIMIT
    })
}

fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR") {
        return Some((
            u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?),
            u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?),
        ));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        return jpeg_dimensions(bytes);
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return webp_dimensions(bytes);
    }
    None
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2;
    while index + 1 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        while bytes.get(index) == Some(&0xff) {
            index += 1;
        }
        let marker = *bytes.get(index)?;
        index += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes.get(index..index + 2)?.try_into().ok()?,
        ));
        if length < 2 || index.checked_add(length)? > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if length < 7 {
                return None;
            }
            return Some((
                u32::from(u16::from_be_bytes(
                    bytes.get(index + 5..index + 7)?.try_into().ok()?,
                )),
                u32::from(u16::from_be_bytes(
                    bytes.get(index + 3..index + 5)?.try_into().ok()?,
                )),
            ));
        }
        index += length;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match bytes.get(12..16)? {
        b"VP8X" => Some((
            1 + u32::from_le_bytes([*bytes.get(24)?, *bytes.get(25)?, *bytes.get(26)?, 0]),
            1 + u32::from_le_bytes([*bytes.get(27)?, *bytes.get(28)?, *bytes.get(29)?, 0]),
        )),
        b"VP8L" if bytes.get(20) == Some(&0x2f) => {
            let bits = u32::from_le_bytes(bytes.get(21..25)?.try_into().ok()?);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8 " if bytes.get(23..26) == Some(&[0x9d, 0x01, 0x2a]) => Some((
            u32::from(u16::from_le_bytes(bytes.get(26..28)?.try_into().ok()?) & 0x3fff),
            u32::from(u16::from_le_bytes(bytes.get(28..30)?.try_into().ok()?) & 0x3fff),
        )),
        _ => None,
    }
}

fn prune_cache(root: &Path, current: &Path) -> Result<(), DesktopProblem> {
    let entries = fs::read_dir(root)
        .map_err(|_| DesktopProblem::storage("Fasti could not inspect the artwork cache."))?;
    let mut files = Vec::new();
    let own_temporary_prefix = format!(".tmp-{}-", std::process::id());
    for entry in entries {
        let entry = entry
            .map_err(|_| DesktopProblem::storage("Fasti could not inspect the artwork cache."))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(".tmp-") {
            if name.starts_with(&own_temporary_prefix) {
                remove_cache_file(
                    &entry.path(),
                    "Fasti could not clear staged provider artwork.",
                )?;
            }
            continue;
        }
        if name.len() != 64 || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|_| DesktopProblem::storage("Fasti could not inspect the artwork cache."))?;
        if metadata.is_file() {
            files.push((
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                entry.path(),
            ));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let remove_count = files.len().saturating_sub(CACHE_FILE_LIMIT);
    for (_, path) in files
        .into_iter()
        .filter(|(_, path)| path != current)
        .take(remove_count)
    {
        remove_cache_file(&path, "Fasti could not prune the artwork cache.")?;
    }
    Ok(())
}

fn remove_cache_file(path: &Path, message: &'static str) -> Result<(), DesktopProblem> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(DesktopProblem::storage(message)),
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), DesktopProblem> {
    fs::create_dir_all(path)
        .map_err(|_| DesktopProblem::storage("Fasti could not create the artwork cache."))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| DesktopProblem::storage("Fasti could not inspect the artwork cache."))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(DesktopProblem::storage(
            "The artwork cache path is not a private directory.",
        ));
    }
    set_directory_permissions(path)
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), DesktopProblem> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| DesktopProblem::storage("Fasti could not protect the artwork cache."))
}

#[cfg(not(unix))]
fn set_directory_permissions(_path: &Path) -> Result<(), DesktopProblem> {
    Ok(())
}

#[cfg(unix)]
fn set_file_create_permissions(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_file_create_permissions(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> Result<(), DesktopProblem> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| DesktopProblem::storage("Fasti could not protect the provider artwork."))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), DesktopProblem> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_artwork_urls_are_exactly_allowlisted() {
        assert!(
            artwork_target(TMDB_PROVIDER, "https://image.tmdb.org/t/p/w500/poster.jpg").is_ok()
        );
        assert!(artwork_target(
            GOOGLE_BOOKS_PROVIDER,
            "https://books.google.com/books/content?id=abc&printsec=frontcover"
        )
        .is_ok());
        for (provider, url) in [
            (
                TMDB_PROVIDER,
                "https://api.themoviedb.org/t/p/w500/poster.jpg",
            ),
            (
                TMDB_PROVIDER,
                "https://image.tmdb.org/t/p/original/poster.jpg",
            ),
            (
                TMDB_PROVIDER,
                "https://image.tmdb.org/t/p/w500/poster.jpg?redirect=1",
            ),
            (
                GOOGLE_BOOKS_PROVIDER,
                "http://books.google.com/books/content?id=abc",
            ),
            (GOOGLE_BOOKS_PROVIDER, "https://example.com/poster.jpg"),
        ] {
            assert!(artwork_target(provider, url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn local_path_requires_a_bounded_supported_regular_image() {
        let root = tempfile::tempdir().expect("cache root");
        let cache = ArtworkCache::new(root.path());
        cache.prepare().expect("prepare cache");
        let url = "https://image.tmdb.org/t/p/w500/poster.jpg";
        assert!(cache.local_path(TMDB_PROVIDER, url).is_none());
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&500_u32.to_be_bytes());
        png.extend_from_slice(&750_u32.to_be_bytes());
        cache.store(TMDB_PROVIDER, url, &png).expect("store image");
        assert_eq!(
            cache.local_path(TMDB_PROVIDER, url).as_deref(),
            Some(
                cache
                    .path_for(TMDB_PROVIDER, url)
                    .to_str()
                    .expect("UTF-8 path")
            )
        );
        assert!(cache.local_path(GOOGLE_BOOKS_PROVIDER, url).is_none());
        png[16..20].copy_from_slice(&5000_u32.to_be_bytes());
        assert!(!has_safe_image_dimensions(&png));
    }

    #[test]
    fn cache_pruning_keeps_the_current_file_within_the_fixed_limit() {
        let root = tempfile::tempdir().expect("cache root");
        let mut current = PathBuf::new();
        for index in 0..=CACHE_FILE_LIMIT {
            current = root.path().join(format!("{index:064x}"));
            fs::write(&current, b"cached artwork").expect("write cache entry");
        }

        prune_cache(root.path(), &current).expect("prune cache");

        assert!(current.is_file());
        assert_eq!(
            fs::read_dir(root.path())
                .expect("read cache")
                .filter_map(Result::ok)
                .count(),
            CACHE_FILE_LIMIT
        );
    }

    #[test]
    fn cache_pruning_does_not_remove_another_process_temporary_file() {
        let root = tempfile::tempdir().expect("cache root");
        let current = root.path().join(format!("{:064x}", 1));
        let own_temporary = root
            .path()
            .join(format!(".tmp-{}-stale", std::process::id()));
        let foreign_temporary = root.path().join(".tmp-foreign-process-active");
        fs::write(&current, b"cached artwork").expect("write cache entry");
        fs::write(&own_temporary, b"stale").expect("write own temporary");
        fs::write(&foreign_temporary, b"active").expect("write foreign temporary");

        prune_cache(root.path(), &current).expect("prune cache");

        assert!(!own_temporary.exists());
        assert!(foreign_temporary.is_file());
    }
}
