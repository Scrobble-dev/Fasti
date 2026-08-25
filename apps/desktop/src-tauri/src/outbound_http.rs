use futures_util::StreamExt;

pub(crate) async fn bounded_body(
    response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>, &'static str> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err("The response was too large.");
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "The response was incomplete.")?;
        if body.len().saturating_add(chunk.len()) > maximum {
            return Err("The response was too large.");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
