use crate::setup::DesktopProblem;
use fasti_application::OutboundAccessPolicy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_VERSION: u8 = 1;
const CONFIG_FILE_LIMIT: u64 = 128 * 1024;
const POLICY_LIST_LIMIT: usize = 64;
const POLICY_VALUE_LIMIT: usize = 253;
const ORIGIN_LIMIT: usize = 2048;
const DEFAULT_SERVICE_URL: &str = "http://127.0.0.1:8420";
static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SettingSource {
    Default,
    Saved,
    Build,
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ManagedSetting<T> {
    value: T,
    source: SettingSource,
    managed: bool,
}

impl<T> ManagedSetting<T> {
    fn managed(value: T, source: SettingSource) -> Self {
        Self {
            value,
            source,
            managed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ConnectionPreferenceView {
    service_url: ManagedSetting<String>,
    public_url: ManagedSetting<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NetworkConfiguration {
    connection: ConnectionPreferenceView,
    outbound_policy: OutboundAccessPolicy,
}

impl NetworkConfiguration {
    pub(crate) fn outbound_policy(&self) -> &OutboundAccessPolicy {
        &self.outbound_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveNetworkConfigurationInput {
    service_url: String,
    public_url: Option<String>,
    outbound_policy: OutboundAccessPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedNetworkConfiguration {
    version: u8,
    service_url: String,
    public_url: Option<String>,
    outbound_policy: OutboundAccessPolicy,
}

impl Default for PersistedNetworkConfiguration {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            service_url: DEFAULT_SERVICE_URL.to_owned(),
            public_url: None,
            outbound_policy: OutboundAccessPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ManagedOverrides {
    service_url: Option<(String, SettingSource)>,
    public_url: Option<(Option<String>, SettingSource)>,
}

impl ManagedOverrides {
    fn from_process() -> Result<Self, DesktopProblem> {
        let service_url = managed_raw("FASTI_API_URL", option_env!("FASTI_API_URL"))?;
        let public_url = managed_raw("FASTI_PUBLIC_URL", option_env!("FASTI_PUBLIC_URL"))?
            .map(|(value, source)| (Some(value), source));

        Ok(Self {
            service_url,
            public_url,
        })
    }
}

pub(crate) struct NetworkConfigStore {
    path: PathBuf,
    gate: Mutex<()>,
}

impl NetworkConfigStore {
    pub(crate) fn new(config_root: &Path) -> Self {
        Self {
            path: config_root.join("network-configuration.json"),
            gate: Mutex::new(()),
        }
    }

    pub(crate) fn load(&self) -> Result<NetworkConfiguration, DesktopProblem> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The network settings lock is unavailable."))?;
        self.load_unlocked()
    }

    pub(crate) fn save(
        &self,
        input: SaveNetworkConfigurationInput,
    ) -> Result<NetworkConfiguration, DesktopProblem> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The network settings lock is unavailable."))?;
        validate_input(&input)?;
        let mut saved = self.read_saved()?;
        let overrides = ManagedOverrides::from_process()?;
        let current = resolve_configuration(&saved, &overrides)?;

        require_editable(
            "service URL",
            &current.connection.service_url,
            &input.service_url,
        )?;
        require_editable(
            "public URL",
            &current.connection.public_url,
            &input.public_url,
        )?;

        if !current.connection.service_url.managed {
            saved.service_url = input.service_url;
        }
        if !current.connection.public_url.managed {
            saved.public_url = input.public_url;
        }
        saved.outbound_policy = input.outbound_policy;
        self.write_saved(&saved)?;

        resolve_configuration(&saved, &overrides)
    }

    fn load_unlocked(&self) -> Result<NetworkConfiguration, DesktopProblem> {
        let saved = self.read_saved()?;
        resolve_configuration(&saved, &ManagedOverrides::from_process()?)
    }

    fn read_saved(&self) -> Result<PersistedNetworkConfiguration, DesktopProblem> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PersistedNetworkConfiguration::default())
            }
            Err(_) => {
                return Err(DesktopProblem::storage(
                    "Fasti could not open the saved network settings.",
                ))
            }
        };
        if file
            .metadata()
            .map_err(|_| DesktopProblem::storage("Fasti could not inspect the network settings."))?
            .len()
            > CONFIG_FILE_LIMIT
        {
            return Err(DesktopProblem::configuration(
                "The saved network settings file is too large.",
            ));
        }
        let mut bytes = Vec::new();
        file.take(CONFIG_FILE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| DesktopProblem::storage("Fasti could not read the network settings."))?;
        if bytes.len() as u64 > CONFIG_FILE_LIMIT {
            return Err(DesktopProblem::configuration(
                "The saved network settings file is too large.",
            ));
        }
        let saved: PersistedNetworkConfiguration =
            serde_json::from_slice(&bytes).map_err(|_| {
                DesktopProblem::configuration("The saved network settings are not valid JSON.")
            })?;
        if saved.version != CONFIG_VERSION {
            return Err(DesktopProblem::configuration(
                "The saved network settings use an unsupported version.",
            ));
        }
        validate_saved(&saved)?;
        Ok(saved)
    }

    fn write_saved(&self, saved: &PersistedNetworkConfiguration) -> Result<(), DesktopProblem> {
        let parent = self.path.parent().ok_or_else(|| {
            DesktopProblem::storage("The network settings directory is unavailable.")
        })?;
        fs::create_dir_all(parent).map_err(|_| {
            DesktopProblem::storage("Fasti could not create the network settings directory.")
        })?;
        set_directory_permissions(parent)?;

        let bytes = serde_json::to_vec_pretty(saved)
            .map_err(|_| DesktopProblem::storage("Fasti could not encode the network settings."))?;
        if bytes.len() as u64 > CONFIG_FILE_LIMIT {
            return Err(DesktopProblem::configuration(
                "The network settings exceed the storage limit.",
            ));
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let temporary = self.path.with_extension(format!(
            "tmp-{}-{nonce}-{}",
            std::process::id(),
            TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_file_create_permissions(&mut options);
        let mut file = options
            .open(&temporary)
            .map_err(|_| DesktopProblem::storage("Fasti could not stage the network settings."))?;
        let write_result = file
            .write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all());
        drop(file);
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(DesktopProblem::storage(
                "Fasti could not save the network settings.",
            ));
        }
        if replace_file(&temporary, &self.path).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(DesktopProblem::storage(
                "Fasti could not save the network settings.",
            ));
        }
        set_file_permissions(&self.path)?;
        Ok(())
    }
}

fn resolve_configuration(
    saved: &PersistedNetworkConfiguration,
    overrides: &ManagedOverrides,
) -> Result<NetworkConfiguration, DesktopProblem> {
    validate_saved(saved)?;
    let service_url = overrides
        .service_url
        .as_ref()
        .map(|(value, source)| ManagedSetting::managed(value.clone(), *source))
        .unwrap_or_else(|| {
            saved_setting(saved.service_url.clone(), DEFAULT_SERVICE_URL.to_owned())
        });
    let public_url = overrides
        .public_url
        .as_ref()
        .map(|(value, source)| ManagedSetting::managed(value.clone(), *source))
        .unwrap_or_else(|| saved_setting(saved.public_url.clone(), None));
    validate_origin(&service_url.value, "service URL")?;
    if let Some(url) = public_url.value.as_deref() {
        validate_origin(url, "public URL")?;
    }
    Ok(NetworkConfiguration {
        connection: ConnectionPreferenceView {
            service_url,
            public_url,
        },
        outbound_policy: saved.outbound_policy.clone(),
    })
}

fn saved_setting<T: PartialEq>(value: T, default: T) -> ManagedSetting<T> {
    let source = if value == default {
        SettingSource::Default
    } else {
        SettingSource::Saved
    };
    ManagedSetting {
        value,
        source,
        managed: false,
    }
}

fn managed_raw(
    name: &str,
    build_value: Option<&'static str>,
) -> Result<Option<(String, SettingSource)>, DesktopProblem> {
    match std::env::var(name) {
        Ok(value) => select_managed_value(name, Some(value), build_value),
        Err(std::env::VarError::NotUnicode(_)) => Err(DesktopProblem::configuration(format!(
            "{name} must contain valid UTF-8."
        ))),
        Err(std::env::VarError::NotPresent) => select_managed_value(name, None, build_value),
    }
}

fn select_managed_value(
    name: &str,
    runtime_value: Option<String>,
    build_value: Option<&str>,
) -> Result<Option<(String, SettingSource)>, DesktopProblem> {
    if let Some(value) = runtime_value {
        if value.is_empty() {
            return Err(DesktopProblem::configuration(format!(
                "{name} must not be empty."
            )));
        }
        return Ok(Some((value, SettingSource::Environment)));
    }
    build_value
        .map(|value| {
            if value.is_empty() {
                Err(DesktopProblem::configuration(format!(
                    "The build value for {name} must not be empty."
                )))
            } else {
                Ok((value.to_owned(), SettingSource::Build))
            }
        })
        .transpose()
}

fn validate_input(input: &SaveNetworkConfigurationInput) -> Result<(), DesktopProblem> {
    validate_origin(&input.service_url, "service URL")?;
    if let Some(url) = input.public_url.as_deref() {
        validate_origin(url, "public URL")?;
    }
    validate_policy(&input.outbound_policy)
}

fn validate_saved(saved: &PersistedNetworkConfiguration) -> Result<(), DesktopProblem> {
    validate_input(&SaveNetworkConfigurationInput {
        service_url: saved.service_url.clone(),
        public_url: saved.public_url.clone(),
        outbound_policy: saved.outbound_policy.clone(),
    })
}

fn validate_origin(value: &str, label: &str) -> Result<(), DesktopProblem> {
    parse_origin(value, label).map(|_| ())
}

pub(crate) fn parse_origin(value: &str, label: &str) -> Result<reqwest::Url, DesktopProblem> {
    if value.is_empty()
        || value.len() > ORIGIN_LIMIT
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(DesktopProblem::configuration(format!(
            "The {label} must contain 1 to {ORIGIN_LIMIT} bytes without surrounding whitespace or control characters."
        )));
    }
    let url = reqwest::Url::parse(value).map_err(|_| {
        DesktopProblem::configuration(format!("The {label} must be a valid HTTP or HTTPS origin."))
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port() == Some(0)
    {
        return Err(DesktopProblem::configuration(format!(
            "The {label} must be an origin without credentials, a path, a query, a fragment, or port 0."
        )));
    }
    let loopback = url.host_str().is_some_and(|host| {
        let address = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .unwrap_or(host);
        host.eq_ignore_ascii_case("localhost")
            || address
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() == "http" && !loopback {
        return Err(DesktopProblem::configuration(format!(
            "The {label} must use HTTPS unless it points to a loopback host."
        )));
    }
    Ok(url)
}

fn validate_policy(policy: &OutboundAccessPolicy) -> Result<(), DesktopProblem> {
    policy.validate_identifiers().map_err(|_| {
        DesktopProblem::configuration(
            "Provider and capability policy values must use canonical lowercase identifiers.",
        )
    })?;
    for (label, values) in [
        ("allowed hosts", &policy.allow_hosts),
        ("denied hosts", &policy.deny_hosts),
    ] {
        if values.len() > POLICY_LIST_LIMIT {
            return Err(DesktopProblem::configuration(format!(
                "The {label} list can contain at most {POLICY_LIST_LIMIT} values."
            )));
        }
        for value in values {
            if value.is_empty()
                || value.len() > POLICY_VALUE_LIMIT
                || value.trim() != value
                || value.chars().any(char::is_control)
                || !valid_policy_host(value)
            {
                return Err(DesktopProblem::configuration(format!(
                    "The {label} list contains an invalid value."
                )));
            }
        }
    }
    for networks in [&policy.allow_networks, &policy.deny_networks] {
        if networks.len() > 8 {
            return Err(DesktopProblem::configuration(
                "A network class list cannot contain more than eight values.",
            ));
        }
    }
    Ok(())
}

fn valid_policy_host(value: &str) -> bool {
    let canonical = value.strip_suffix('.').unwrap_or(value);
    !canonical.is_empty()
        && !canonical.ends_with('.')
        && canonical.len() <= POLICY_VALUE_LIMIT
        && canonical.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn require_editable<T: PartialEq>(
    label: &str,
    current: &ManagedSetting<T>,
    requested: &T,
) -> Result<(), DesktopProblem> {
    if current.managed && current.value != *requested {
        return Err(DesktopProblem::configuration(format!(
            "The {label} is managed by the environment or app build and cannot be changed here."
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), DesktopProblem> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| {
        DesktopProblem::storage("Fasti could not protect the network settings directory.")
    })
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
        .map_err(|_| DesktopProblem::storage("Fasti could not protect the network settings."))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), DesktopProblem> {
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are live, NUL-terminated Windows paths.
    #[rustfmt::skip]
    if unsafe { // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SaveNetworkConfigurationInput {
        SaveNetworkConfigurationInput {
            service_url: "http://localhost:19420".to_owned(),
            public_url: Some("https://fasti.internal".to_owned()),
            outbound_policy: OutboundAccessPolicy {
                allow_providers: vec!["google-books".to_owned()],
                ..OutboundAccessPolicy::default()
            },
        }
    }

    #[test]
    fn saves_only_app_owned_non_secret_configuration() {
        let root = tempfile::tempdir().expect("config root");
        let store = NetworkConfigStore::new(root.path());
        let response = store.save(input()).expect("save configuration");

        assert_eq!(
            response.connection.service_url.value,
            "http://localhost:19420"
        );
        assert_eq!(
            store
                .load()
                .expect("reload")
                .outbound_policy
                .allow_providers,
            ["google-books"]
        );
        let saved =
            fs::read_to_string(root.path().join("network-configuration.json")).expect("saved JSON");
        assert!(!saved.contains("credential"));
        assert!(!saved.contains("api_key"));
    }

    #[test]
    fn managed_values_are_read_only_and_remain_visible() {
        let saved = PersistedNetworkConfiguration {
            service_url: "http://127.0.0.1:9000".to_owned(),
            ..PersistedNetworkConfiguration::default()
        };
        let overrides = ManagedOverrides {
            service_url: Some((
                "https://fasti.internal".to_owned(),
                SettingSource::Environment,
            )),
            ..ManagedOverrides::default()
        };
        let view = resolve_configuration(&saved, &overrides).expect("managed view");

        assert_eq!(view.connection.service_url.value, "https://fasti.internal");
        assert!(view.connection.service_url.managed);
        assert_eq!(
            view.connection.service_url.source,
            SettingSource::Environment
        );
    }

    #[test]
    fn environment_values_take_precedence_over_build_values() {
        assert_eq!(
            select_managed_value(
                "FASTI_API_URL",
                Some("https://runtime.internal".to_owned()),
                Some("https://build.internal"),
            )
            .expect("managed value"),
            Some((
                "https://runtime.internal".to_owned(),
                SettingSource::Environment,
            ))
        );
        assert_eq!(
            select_managed_value("FASTI_API_URL", None, Some("https://build.internal"))
                .expect("build value"),
            Some(("https://build.internal".to_owned(), SettingSource::Build,))
        );
    }

    #[test]
    fn rejects_port_zero_and_non_loopback_plain_http() {
        assert!(validate_origin("http://fasti.internal", "service URL").is_err());
        assert!(validate_origin("http://localhost:0", "service URL").is_err());
        assert!(validate_origin("http://127.0.0.1:8420", "service URL").is_ok());
        assert!(validate_origin("http://[::1]:8420", "service URL").is_ok());
        assert!(validate_origin("https://fasti.internal", "service URL").is_ok());
    }

    #[test]
    fn origins_reject_whitespace_controls_and_oversized_values_before_parsing() {
        assert!(validate_origin(" http://localhost:8420", "service URL").is_err());
        assert!(validate_origin("http://localhost:8420\n", "service URL").is_err());
        assert!(validate_origin(&"h".repeat(ORIGIN_LIMIT + 1), "service URL").is_err());
    }

    #[test]
    fn policy_values_are_bounded_and_hostnames_do_not_accept_wildcards() {
        let mut value = input();
        value.outbound_policy.allow_hosts = vec!["*.googleapis.com".to_owned()];
        assert!(validate_input(&value).is_err());
        value.outbound_policy.allow_hosts = vec!["www.googleapis.com".to_owned()];
        assert!(validate_input(&value).is_ok());
        value.outbound_policy.deny_providers = vec!["GOOGLE-BOOKS".to_owned()];
        assert!(validate_input(&value).is_err());
    }
}
