pub fn auth(builder: reqwest::RequestBuilder, token: Option<&String>) -> reqwest::RequestBuilder {
    match token {
        Some(t) => builder.bearer_auth(t),
        None => builder,
    }
}
