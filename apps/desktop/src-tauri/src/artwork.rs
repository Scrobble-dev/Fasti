use crate::network_config::replace_file;
use crate::setup::DesktopProblem;
use fasti_application::{
    NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy, ProviderResponseCachePolicy,
    ProviderResponseReuse, RequestAccessContext,
};
use fasti_domain::RecordId;
use fasti_provider_runtime::{
    bounded_body, GovernedTransport, ProviderCandidate, GOOGLE_BOOKS_PROVIDER, TMDB_PROVIDER,
};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const ARTWORK_CAPABILITY: &str = "metadata.artwork";
const TMDB_IMAGE_HOST: &str = "image.tmdb.org";
const GOOGLE_IMAGE_HOSTS: &[&str] = &["books.google.com", "books.googleusercontent.com"];
const ARTWORK_LIMIT: usize = 2_000_000;
const ARTWORK_DIMENSION_LIMIT: u32 = 4096;
const ARTWORK_PIXEL_LIMIT: u64 = 16_000_000;
const CACHE_FILE_LIMIT: usize = 128;
const ENVELOPE_HEADER_LIMIT: usize = 2048;
const IMAGE_FRESH_CAP: Duration = Duration::from_secs(24 * 60 * 60);
const IMAGE_STALE_CAP: Duration = Duration::from_secs(7 * 24 * 60 * 60);
static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
include!("artwork_policy_tests.rs");

#[cfg(test)]
include!("artwork_native_fixture.rs");

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
    access: OutboundAccessDeclaration<'static>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtworkHeader {
    version: u8,
    key: String,
    policy: ProviderResponseCachePolicy,
    digest: [u8; 32],
    length: u32,
}

struct CachedArtwork {
    header: ArtworkHeader,
    bytes: Vec<u8>,
}

impl CachedArtwork {
    fn reusable_at(&self, now: chrono::DateTime<chrono::Utc>, on_error: bool) -> bool {
        let policy = self.header.policy;
        let Some((fresh, stale)) = policy.deadlines(IMAGE_FRESH_CAP, IMAGE_STALE_CAP) else {
            return false;
        };
        now >= policy.received_at()
            && now
                < if on_error && policy.reuse() == ProviderResponseReuse::Reusable {
                    stale
                } else {
                    fresh
                }
    }
}

pub(crate) struct ArtworkCache {
    root: PathBuf,
    gate: Mutex<()>,
    directory: OnceLock<File>,
}

impl ArtworkCache {
    pub(crate) fn new(root: impl Into<PathBuf>, identity: fasti_store::DataRootIdentity) -> Self {
        Self {
            root: root
                .into()
                .join(crate::secure_storage::account_scope(identity)),
            gate: Mutex::new(()),
            directory: OnceLock::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn prepare(&self) -> Result<(), DesktopProblem> {
        if self.directory.get().is_some() {
            return Ok(());
        }
        fs::create_dir_all(&self.root)
            .map_err(|_| DesktopProblem::storage("Fasti could not create the artwork cache."))?;
        let directory = open_cache_directory(&self.root).map_err(|_| {
            DesktopProblem::storage("Fasti could not open its private artwork directory.")
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            directory
                .set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(|_| {
                    DesktopProblem::storage("Fasti could not protect its artwork directory.")
                })?;
        }
        let _ = self.directory.set(directory);
        Ok(())
    }

    fn cache_root(&self) -> Result<PathBuf, DesktopProblem> {
        self.prepare()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use std::os::fd::AsRawFd;
            // The cache retains this descriptor for its lifetime, including every request.
            Ok(PathBuf::from(format!(
                "/proc/self/fd/{}/.",
                self.directory
                    .get()
                    .expect("prepared directory")
                    .as_raw_fd()
            )))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok(self.root.clone())
        }
    }

    fn cache_path(&self, provider: &str, url: &str) -> Result<PathBuf, DesktopProblem> {
        Ok(self.cache_root()?.join(
            self.path_for(provider, url)
                .file_name()
                .expect("owned hash"),
        ))
    }

    pub(crate) async fn cache_candidate(
        &self,
        candidate: &ProviderCandidate,
        policy: &OutboundAccessPolicy,
        transport: &GovernedTransport,
    ) -> Result<(), DesktopProblem> {
        if candidate.recorded_response_policy()?.reuse()
            == fasti_application::ProviderResponseReuse::NoStore
        {
            return Err(DesktopProblem::provider(
                "The provider response cannot be stored.",
            ));
        }
        let Some(url) = candidate.image_url.as_deref() else {
            return Ok(());
        };
        self.load(candidate.provider, url, policy, transport)
            .await
            .map(|_| ())
    }

    pub(crate) fn locator(
        &self,
        provider: &str,
        url: &str,
        access: RequestAccessContext,
        record: RecordId,
    ) -> Option<String> {
        artwork_target(provider, url).ok()?;
        self.record_locator(access, record)
    }

    fn record_locator(&self, access: RequestAccessContext, record: RecordId) -> Option<String> {
        Some(format!(
            "fasti-artwork.{}.{}.{}.{}",
            self.root.file_name()?.to_str()?,
            access.workspace_id(),
            access.profile_id(),
            record
        ))
    }

    pub(crate) fn locator_record(
        &self,
        locator: &str,
        access: RequestAccessContext,
    ) -> Option<RecordId> {
        if locator.len() > 256 {
            return None;
        }
        let record = locator.rsplit('.').next()?.parse::<RecordId>().ok()?;
        (self.record_locator(access, record)?.as_str() == locator).then_some(record)
    }

    pub(crate) async fn load(
        &self,
        provider: &str,
        url: &str,
        policy: &OutboundAccessPolicy,
        transport: &GovernedTransport,
    ) -> Result<Vec<u8>, DesktopProblem> {
        self.load_with_response(provider, url, async {
            let target = artwork_target(provider, url)?;
            let client = transport
                .authorize(target.access, policy, ARTWORK_CAPABILITY, &target.url)
                .await
                .map_err(|error| DesktopProblem::provider(error.detail()))?;
            let started = Instant::now();
            let response = client
                .get(target.url.clone())
                .map_err(DesktopProblem::provider)?
                .header(ACCEPT, "image/jpeg, image/png, image/webp")
                .send()
                .await
                .map_err(|_| {
                    DesktopProblem::provider("The provider artwork could not be reached.")
                })?;
            Ok((response, started.elapsed(), client))
        })
        .await
    }

    async fn load_with_response<G>(
        &self,
        provider: &str,
        url: &str,
        request: impl std::future::Future<
            Output = Result<(reqwest::Response, Duration, G), DesktopProblem>,
        >,
    ) -> Result<Vec<u8>, DesktopProblem> {
        artwork_target(provider, url)?;
        let cached = self.cached_entry(provider, url);
        if cached
            .as_ref()
            .is_some_and(|entry| entry.reusable_at(chrono::Utc::now(), false))
        {
            return Ok(cached.expect("checked cache entry").bytes);
        }
        // Invalidate durably before observing a replacement. If this fails,
        // no restrictive upstream response can be followed by old disk reuse.
        let response = match self.invalidate(provider, url) {
            Ok(()) => request.await,
            Err(error) => Err(error),
        }
        .and_then(|(response, delay, guard)| {
            if response.status() == reqwest::StatusCode::OK {
                Ok((response, delay, guard))
            } else {
                Err(DesktopProblem::provider(format!(
                    "The provider artwork returned HTTP {}.",
                    response.status().as_u16()
                )))
            }
        });
        // Keep the governed client's permit until the body is consumed and published.
        let (response, delay, _request_guard) = match response {
            Ok(response) => response,
            Err(error) => {
                if let Some(entry) =
                    cached.filter(|entry| entry.reusable_at(chrono::Utc::now(), true))
                {
                    // Retain original evidence after an outage; never renew its deadlines.
                    let _ = self.store(provider, url, &entry.bytes, &entry.header.policy);
                    return Ok(entry.bytes);
                }
                return Err(error);
            }
        };
        // A replacement representation has been observed. Errors below must
        // never restore the prior bytes, even if its headers or body are invalid.
        let response_policy = fasti_provider_runtime::observe_response_cache_policy(
            response.headers(),
            chrono::Utc::now(),
            delay,
        );
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
        // The live validated image remains usable when optional cache storage fails.
        if response_policy.reuse() != ProviderResponseReuse::NoStore {
            let _ = self.store(provider, url, &body, &response_policy);
        }
        Ok(body)
    }

    #[cfg(test)]
    pub(crate) fn local_path(&self, provider: &str, url: &str) -> Option<String> {
        let entry = self.cached_entry(provider, url)?;
        if !entry.reusable_at(chrono::Utc::now(), false) {
            return None;
        }
        self.path_for(provider, url).to_str().map(ToOwned::to_owned)
    }

    fn cached_entry(&self, provider: &str, url: &str) -> Option<CachedArtwork> {
        artwork_target(provider, url).ok()?;
        let path = self.cache_path(provider, url).ok()?;
        let mut file = open_cache_image(&path).ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.file_type().is_file()
            || metadata.len() == 0
            || metadata.len() > (4 + ENVELOPE_HEADER_LIMIT + ARTWORK_LIMIT) as u64
        {
            return None;
        }
        let mut size = [0; 4];
        file.read_exact(&mut size).ok()?;
        let size = u32::from_be_bytes(size) as usize;
        if size == 0 || size > ENVELOPE_HEADER_LIMIT {
            return None;
        }
        let mut header_bytes = vec![0; size];
        file.read_exact(&mut header_bytes).ok()?;
        let header: ArtworkHeader = serde_json::from_slice(&header_bytes).ok()?;
        if header.version != 1
            || header.key != path.file_name()?.to_str()?
            || header.policy.reuse() == ProviderResponseReuse::NoStore
            || serde_json::to_vec(&header).ok()? != header_bytes
            || header.length == 0
            || header.length as usize > ARTWORK_LIMIT
            || metadata.len() != 4 + size as u64 + u64::from(header.length)
        {
            return None;
        }
        let mut bytes = vec![0; header.length as usize];
        file.read_exact(&mut bytes).ok()?;
        let mut extra = [0];
        if file.read(&mut extra).ok()? != 0
            || !has_safe_image_dimensions(&bytes)
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != header.digest
        {
            return None;
        }
        Some(CachedArtwork { header, bytes })
    }

    fn invalidate(&self, provider: &str, url: &str) -> Result<(), DesktopProblem> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The artwork cache lock is unavailable."))?;
        let root = self.cache_root()?;
        remove_cache_file(
            &self.cache_path(provider, url)?,
            "Fasti could not invalidate cached artwork.",
        )?;
        sync_cache_directory(&root)
            .map_err(|_| DesktopProblem::storage("Fasti could not synchronize the artwork cache."))
    }

    fn store(
        &self,
        provider: &str,
        url: &str,
        body: &[u8],
        policy: &ProviderResponseCachePolicy,
    ) -> Result<(), DesktopProblem> {
        artwork_target(provider, url)?;
        if policy.reuse() == ProviderResponseReuse::NoStore || image_content_type(body).is_none() {
            return Err(DesktopProblem::provider("This artwork cannot be cached."));
        }
        let header = serde_json::to_vec(&ArtworkHeader {
            version: 1,
            key: self
                .path_for(provider, url)
                .file_name()
                .and_then(|s| s.to_str())
                .expect("owned hash")
                .to_owned(),
            policy: *policy,
            digest: Sha256::digest(body).into(),
            length: body.len() as u32,
        })
        .map_err(|_| DesktopProblem::storage("Invalid artwork cache evidence."))?;
        if header.len() > ENVELOPE_HEADER_LIMIT {
            return Err(DesktopProblem::storage(
                "Artwork cache evidence exceeds its limit.",
            ));
        }
        let _guard = self
            .gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The artwork cache lock is unavailable."))?;
        let root = self.cache_root()?;
        let destination = self.cache_path(provider, url)?;
        let temporary = root.join(format!(
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
        let write_result = protect_open_cache_file(&file)
            .and_then(|()| file.write_all(&(header.len() as u32).to_be_bytes()))
            .and_then(|()| file.write_all(&header))
            .and_then(|()| file.write_all(body))
            .and_then(|()| file.sync_all());
        drop(file);
        if write_result.is_err() || replace_file(&temporary, &destination).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(DesktopProblem::storage(
                "Fasti could not save the provider artwork.",
            ));
        }
        sync_cache_directory(&root).map_err(|_| {
            DesktopProblem::storage("Fasti could not synchronize the artwork cache.")
        })?;
        // The artwork is already committed; cache cleanup must not turn that success into a failure.
        let _ = prune_cache(&root, &destination);
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

pub(crate) fn image_content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > ARTWORK_LIMIT || !has_safe_image_dimensions(bytes) {
        return None;
    }
    Some(if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        "image/jpeg"
    } else {
        "image/webp"
    })
}

fn open_cache_image(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    let file = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?);
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?
    };
    #[cfg(not(any(unix, windows)))]
    let file = {
        if !fs::symlink_metadata(path)?.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid cache entry",
            ));
        }
        File::open(path)?
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the artwork cache entry is not a regular file",
        ));
    }
    Ok(file)
}

fn protect_open_cache_file(file: &File) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

fn open_cache_directory(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    let directory = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    #[cfg(windows)]
    let directory = {
        use std::os::windows::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
                    | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            )
            .open(path)?
    };
    #[cfg(not(any(unix, windows)))]
    let directory = File::open(path)?;
    if !directory.metadata()?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid cache directory",
        ));
    }
    Ok(directory)
}

fn sync_cache_directory(path: &Path) -> std::io::Result<()> {
    open_cache_directory(path)?.sync_all()
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
    let access = match provider {
        TMDB_PROVIDER
            if parsed_host == TMDB_IMAGE_HOST
                && url.path().starts_with("/t/p/w500/")
                && url.query().is_none() =>
        {
            TMDB_ARTWORK_ACCESS
        }
        GOOGLE_BOOKS_PROVIDER if parsed_host == GOOGLE_IMAGE_HOSTS[0] => GOOGLE_ARTWORK_ACCESS,
        GOOGLE_BOOKS_PROVIDER if parsed_host == GOOGLE_IMAGE_HOSTS[1] => GOOGLE_ARTWORK_ACCESS,
        _ => return Err(unsafe_artwork_url()),
    };
    Ok(ArtworkTarget { url, access })
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

#[cfg(unix)]
fn set_file_create_permissions(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_file_create_permissions(_options: &mut OpenOptions) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn reusable_image_policy() -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            chrono::Utc::now(),
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            None,
        )
    }

    #[test]
    #[cfg(unix)]
    fn cache_read_rejects_links_and_fifo_and_keeps_the_opened_inode() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image");
        fs::write(&path, b"original bytes").unwrap();
        symlink(&path, root.path().join("link")).unwrap();
        assert!(open_cache_image(&root.path().join("link")).is_err());
        assert!(open_cache_image(root.path()).is_err());
        let directory = File::open(root.path()).unwrap();
        rustix::fs::mkfifoat(&directory, "fifo", rustix::fs::Mode::from_raw_mode(0o600)).unwrap();
        assert!(open_cache_image(&root.path().join("fifo")).is_err());

        let mut opened = open_cache_image(&path).unwrap();
        fs::rename(&path, root.path().join("old-image")).unwrap();
        fs::write(&path, b"replacement bytes").unwrap();
        let mut bytes = Vec::new();
        opened.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original bytes");
        assert_eq!(fs::read(&path).unwrap(), b"replacement bytes");
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn artwork_cache_follows_physical_node_not_configured_path() {
        let root = tempfile::tempdir().unwrap();
        let configured = root.path().join("data");
        let moved = root.path().join("moved-data");
        let shared_cache = root.path().join("provider-artwork");
        let kernel = fasti_store::SqliteKernel::open(&configured).unwrap();
        let original = ArtworkCache::new(&shared_cache, kernel.data_root_identity());
        let url = "https://image.tmdb.org/t/p/w500/poster.jpg";
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&500_u32.to_be_bytes());
        png.extend_from_slice(&750_u32.to_be_bytes());
        fs::create_dir_all(&shared_cache).unwrap();
        let legacy_path =
            shared_cache.join(original.path_for(TMDB_PROVIDER, url).file_name().unwrap());
        fs::write(&legacy_path, &png).unwrap();
        assert!(original.local_path(TMDB_PROVIDER, url).is_none());
        original
            .store(TMDB_PROVIDER, url, &png, &reusable_image_policy())
            .unwrap();
        assert_eq!(fs::read(&legacy_path).unwrap(), png);
        prune_cache(original.root(), &original.path_for(TMDB_PROVIDER, url)).unwrap();
        assert_eq!(fs::read(&legacy_path).unwrap(), png);
        let original_path = original.local_path(TMDB_PROVIDER, url).unwrap();

        fs::rename(&configured, &moved).unwrap();
        let renamed = ArtworkCache::new(&shared_cache, kernel.data_root_identity());
        assert_eq!(original.root(), renamed.root());
        assert_eq!(
            renamed.local_path(TMDB_PROVIDER, url),
            Some(original_path.clone())
        );
        drop(kernel);
        let reopened = fasti_store::SqliteKernel::open(&moved).unwrap();
        let reopened_cache = ArtworkCache::new(&shared_cache, reopened.data_root_identity());
        assert_eq!(
            reopened_cache.local_path(TMDB_PROVIDER, url),
            Some(original_path.clone())
        );

        let replacement = fasti_store::SqliteKernel::open(&configured).unwrap();
        let replacement_cache = ArtworkCache::new(&shared_cache, replacement.data_root_identity());
        assert_ne!(original.root(), replacement_cache.root());
        assert!(replacement_cache.local_path(TMDB_PROVIDER, url).is_none());
        replacement_cache
            .store(TMDB_PROVIDER, url, &png, &reusable_image_policy())
            .unwrap();
        assert_ne!(
            replacement_cache.local_path(TMDB_PROVIDER, url),
            Some(original_path.clone())
        );
        assert_eq!(
            reopened_cache.local_path(TMDB_PROVIDER, url),
            Some(original_path)
        );
    }

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
    fn record_posters_use_provider_provenance_not_identifier_namespace() {
        use crate::records::{list_records, require_access};
        use crate::setup::{
            complete_setup,
            test_support::{new_kernel, MemoryStore},
        };
        use fasti_application::{
            provider_identity_mapping, CreateProviderRecordCommand, IdentityPort,
            ProviderMetadataField, ProviderMetadataPort, ProviderResponseCachePolicy,
            ProviderResponseReuse, RegisterNamespaceDefinitionCommand,
        };
        use fasti_domain::{
            FieldClaim, FieldClaimProvenance, FieldClaimStatus, FieldKey, MetadataClaimId,
            MetadataProviderId, NamespaceKey, ReceivedAt, RequestCorrelationId, Sha256Digest,
            POSTER_FIELD_KEY,
        };

        for (provider, kind, url) in [
            (
                TMDB_PROVIDER,
                "movie",
                "https://image.tmdb.org/t/p/w500/poster.jpg",
            ),
            (
                GOOGLE_BOOKS_PROVIDER,
                "book",
                "https://books.google.com/books/content?id=42",
            ),
        ] {
            let (root, kernel) = new_kernel();
            let store = MemoryStore::default();
            complete_setup(&kernel, &store).unwrap();
            let access = require_access(&kernel, &store).unwrap();
            let mapping = provider_identity_mapping(provider, kind).unwrap();
            kernel
                .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    mapping.namespace_definition().unwrap(),
                ))
                .unwrap();
            let observed = chrono::Utc::now();
            let claim = FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                url,
                FieldClaimProvenance::try_new(
                    MetadataProviderId::try_new(provider).unwrap(),
                    NamespaceKey::try_new(mapping.namespace()).unwrap(),
                    "42",
                    None,
                    None,
                    None,
                    Sha256Digest::from_bytes(&[7; 32]),
                )
                .unwrap(),
                ReceivedAt::from_application_clock(observed),
                Some(observed + chrono::Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap();
            kernel
                .create_provider_record(CreateProviderRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    mapping.grain(),
                    mapping.identifier("42").unwrap(),
                    vec![ProviderMetadataField::new(
                        FieldKey::try_new(POSTER_FIELD_KEY).unwrap(),
                        claim,
                    )],
                    ProviderResponseCachePolicy::new(
                        ProviderResponseReuse::Reusable,
                        observed,
                        std::time::Duration::ZERO,
                        Some(std::time::Duration::from_secs(120)),
                        None,
                    ),
                ))
                .unwrap();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
            png.extend_from_slice(&500_u32.to_be_bytes());
            png.extend_from_slice(&750_u32.to_be_bytes());
            cache
                .store(provider, url, &png, &reusable_image_policy())
                .unwrap();
            assert!(cache.local_path(mapping.namespace(), url).is_none());
            let page =
                serde_json::to_value(list_records(&kernel, &store, &cache, None).unwrap()).unwrap();
            assert_eq!(page["records"][0]["poster"]["source"], mapping.namespace());
            assert_eq!(
                page["records"][0]["poster_asset_path"],
                cache
                    .locator(
                        provider,
                        url,
                        access,
                        page["records"][0]["record_id"]
                            .as_str()
                            .unwrap()
                            .parse()
                            .unwrap()
                    )
                    .unwrap()
            );
        }
    }

    #[test]
    fn local_path_requires_a_bounded_supported_regular_image() {
        let root = tempfile::tempdir().expect("cache root");
        let (_node, kernel) = crate::setup::test_support::new_kernel();
        let cache = ArtworkCache::new(root.path(), kernel.data_root_identity());
        cache.prepare().expect("prepare cache");
        let url = "https://image.tmdb.org/t/p/w500/poster.jpg";
        assert!(cache.local_path(TMDB_PROVIDER, url).is_none());
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&500_u32.to_be_bytes());
        png.extend_from_slice(&750_u32.to_be_bytes());
        cache
            .store(TMDB_PROVIDER, url, &png, &reusable_image_policy())
            .expect("store image");
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

    #[tokio::test]
    #[ignore = "requires the public TMDB image endpoint; no credential is used"]
    async fn live_tmdb_artwork_preserves_observed_policy_and_reopens_from_cache() {
        // Public sample from https://developer.themoviedb.org/docs/image-basics.
        // This checks governed HTTP and cache reuse, not native WebView decoding.
        const URL: &str = "https://image.tmdb.org/t/p/w500/wwemzKWzjKYJFfCeiB57q3r4Bcm.png";
        let (root, kernel) = crate::setup::test_support::new_kernel();
        let cache_root = root.path().join("live-artwork");
        let cache = ArtworkCache::new(&cache_root, kernel.data_root_identity());
        let body = cache
            .load(
                TMDB_PROVIDER,
                URL,
                &OutboundAccessPolicy::default(),
                &GovernedTransport::default(),
            )
            .await
            .expect("public documentation image through governed transport");
        let mime = image_content_type(&body).expect("validated negotiated image format");
        let entry = cache
            .cached_entry(TMDB_PROVIDER, URL)
            .expect("documentation image must currently permit cache storage");
        assert_eq!(entry.bytes, body);
        let policy = entry.header.policy;
        println!(
            "Live TMDB image: {} bytes; MIME {mime}; policy {}; digest {:?}",
            body.len(),
            serde_json::to_string(&policy).unwrap(),
            entry.header.digest
        );
        assert!(entry.reusable_at(chrono::Utc::now(), false));
        drop(cache);
        let cache = ArtworkCache::new(&cache_root, kernel.data_root_identity());
        let reopened = cache
            .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                panic!("eligible reopened cache must not poll a network request")
            })
            .await
            .unwrap();
        assert_eq!(reopened, body);
        assert_eq!(
            cache
                .cached_entry(TMDB_PROVIDER, URL)
                .unwrap()
                .header
                .policy,
            policy
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
