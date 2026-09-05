//! Private native image delivery. Locators constrain scope; they never authorize access.

use crate::{artwork, records, DesktopState, KeyringSetupSecretStore};
use tauri::{http, AppHandle, Manager, Runtime, UriSchemeContext, UriSchemeResponder};

#[cfg(test)]
include!("artwork_protocol_tests.rs");

// Bound queued work before spawning, not merely the eventual network response.
static ARTWORK_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(64);

fn trusted_origin<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<String> {
    if label != "main" {
        return None;
    }
    let origin = app
        .get_webview_window(label)?
        .url()
        .ok()?
        .origin()
        .ascii_serialization();
    #[cfg(not(target_os = "android"))]
    let trusted = origin == fasti_api::FASTI_ACCESS_ORIGIN;
    #[cfg(target_os = "android")]
    let trusted = origin == "http://tauri.localhost";
    trusted.then_some(origin)
}

fn request_locator<'a>(request: &'a http::Request<Vec<u8>>, origin: &str) -> Option<&'a str> {
    if request.method() != http::Method::GET
        || !request.body().is_empty()
        || request.uri().query().is_some()
        || request.uri().to_string().len() > 320
        || request.headers().len() > 32
        || request
            .headers()
            .iter()
            .map(|(name, value)| name.as_str().len() + value.len())
            .sum::<usize>()
            > 8192
    {
        return None;
    }
    match (
        request.uri().scheme_str(),
        request.uri().authority()?.as_str(),
    ) {
        (Some("asset"), "localhost") | (Some("http"), "asset.localhost") => {}
        _ => return None,
    }
    let mut origins = request.headers().get_all(http::header::ORIGIN).iter();
    if origins
        .next()
        .is_some_and(|value| value.as_bytes() != origin.as_bytes())
        || origins.next().is_some()
    {
        return None;
    }
    let locator = request.uri().path().strip_prefix('/')?;
    (locator.starts_with("fasti-artwork.")
        && locator.len() <= 256
        && locator
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
    .then_some(locator)
}

fn response(bytes: Option<Vec<u8>>, origin: Option<&str>) -> http::Response<Vec<u8>> {
    let mut builder = http::Response::builder()
        .header(http::header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if let Some(origin) = origin {
        builder = builder.header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    match bytes.and_then(|bytes| artwork::image_content_type(&bytes).map(|mime| (bytes, mime))) {
        Some((bytes, mime)) => builder.header(http::header::CONTENT_TYPE, mime).body(bytes),
        None => builder.status(http::StatusCode::NOT_FOUND).body(Vec::new()),
    }
    .expect("static headers and a canonical native origin form a valid response")
}

pub(crate) fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let Some(origin) = trusted_origin(&app, context.webview_label()) else {
        responder.respond(response(None, None));
        return;
    };
    let Some(locator) = request_locator(&request, &origin).map(ToOwned::to_owned) else {
        responder.respond(response(None, Some(&origin)));
        return;
    };
    let Ok(permit) = ARTWORK_REQUESTS.try_acquire() else {
        let mut busy = response(None, Some(&origin));
        *busy.status_mut() = http::StatusCode::SERVICE_UNAVAILABLE;
        busy.headers_mut().insert(
            http::header::RETRY_AFTER,
            http::HeaderValue::from_static("1"),
        );
        responder.respond(busy);
        return;
    };
    tauri::async_runtime::spawn(async move {
        let _permit = permit;
        let bytes = async {
            let state = app.try_state::<DesktopState>()?;
            // ponytail: global provider gate serializes images and mutations; use keyed
            // ordering only if measured throughput requires it without weakening publication.
            let _guard = state.provider_operation_gate.lock().await;
            let kernel = state.kernel().ok()?;
            let store = KeyringSetupSecretStore::new(kernel.data_root_identity());
            let before =
                records::artwork_selection(&kernel, &store, &state.artwork, &locator).ok()?;
            let provider = before
                .1
                .provenance()?
                .claim_provenance()
                .provider_id()?
                .as_str();
            let url = before.1.value()?;
            let configuration = state.network.load().ok()?;
            let runtime = state.provider_runtime(&kernel).ok()?;
            let bytes = state
                .artwork
                .load(
                    provider,
                    url,
                    configuration.outbound_policy(),
                    runtime.transport(),
                )
                .await
                .ok()?;
            let after =
                records::artwork_selection(&kernel, &store, &state.artwork, &locator).ok()?;
            if before != after || trusted_origin(&app, "main").as_deref() != Some(origin.as_str()) {
                return None;
            }
            Some(bytes)
        }
        .await;
        responder.respond(response(bytes, Some(&origin)));
    });
}
