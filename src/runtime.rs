use crate::{
    config::{Sites, parse_sites},
    protocol::{self, Challenge, InvalidProof},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use worker::*;

const MAX_BODY: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    site: String,
    token: Option<String>,
    nonce: Option<String>,
}

fn now() -> u64 {
    Date::now().as_millis() / 1000
}

fn reply(status: u16, body: Value) -> Result<Response> {
    let mut response = Response::from_json(&body)?.with_status(status);
    let headers = response.headers_mut();
    headers.set("Cache-Control", "no-store")?;
    headers.set(
        "Content-Security-Policy",
        "default-src 'none'; frame-ancestors 'none'",
    )?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    headers.set("Referrer-Policy", "no-referrer")?;
    headers.set("Vary", "Origin")?;
    if status == 429 {
        headers.set("Retry-After", "60")?;
    }
    if status == 405 {
        headers.set("Allow", "POST, OPTIONS")?;
    }
    Ok(response)
}

fn failure(status: u16, code: &str) -> Result<Response> {
    reply(status, json!({"error": code}))
}

fn sites(env: &Env) -> Result<Sites> {
    parse_sites(&env.secret("SITES")?.to_string()).map_err(Error::from)
}

#[event(fetch)]
pub async fn fetch(mut request: Request, env: Env, _ctx: Context) -> Result<Response> {
    match route(&mut request, &env).await {
        Ok(response) => Ok(response),
        Err(_) => failure(503, "unavailable"),
    }
}

async fn route(request: &mut Request, env: &Env) -> Result<Response> {
    if !matches!(request.path().as_str(), "/v1/challenge" | "/v1/reveal")
        || request.url()?.query().is_some()
    {
        return failure(404, "not_found");
    }
    let origin = request.headers().get("Origin")?.unwrap_or_default();
    let configured = sites(env)?;
    if !configured
        .values()
        .any(|site| site.origins.contains(&origin))
    {
        return failure(403, "origin_denied");
    }
    let response = route_allowed(request, env, &configured, &origin)
        .await
        .unwrap_or_else(|_| failure(503, "unavailable").expect("Static response is valid."));
    let headers = response.headers().clone();
    headers.set("Access-Control-Allow-Origin", &origin)?;
    Ok(response.with_headers(headers))
}

async fn route_allowed(
    request: &mut Request,
    env: &Env,
    configured: &Sites,
    origin: &str,
) -> Result<Response> {
    if request.method() == Method::Options {
        let requested = request
            .headers()
            .get("Access-Control-Request-Headers")?
            .unwrap_or_default();
        if request
            .headers()
            .get("Access-Control-Request-Method")?
            .as_deref()
            != Some("POST")
            || requested.split(',').any(|header| {
                !header.trim().is_empty() && !header.trim().eq_ignore_ascii_case("content-type")
            })
        {
            return failure(403, "preflight_denied");
        }
        let mut response = Response::empty()?.with_status(204);
        let headers = response.headers_mut();
        headers.set("Access-Control-Allow-Methods", "POST")?;
        headers.set("Access-Control-Allow-Headers", "Content-Type")?;
        headers.set("Cache-Control", "no-store")?;
        headers.set(
            "Vary",
            "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
        )?;
        return Ok(response);
    }
    if request.method() != Method::Post {
        return failure(405, "method_not_allowed");
    }
    let content_type = request.headers().get("Content-Type")?.unwrap_or_default();
    if !content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .eq_ignore_ascii_case("application/json")
    {
        return failure(415, "json_required");
    }
    if request.headers().get("Content-Encoding")?.is_some() {
        return failure(415, "encoding_not_supported");
    }
    if request
        .headers()
        .get("Content-Length")?
        .and_then(|n| n.parse::<usize>().ok())
        .is_some_and(|n| n > MAX_BODY)
    {
        return failure(413, "body_too_large");
    }
    let mut body = Vec::new();
    let mut stream = request.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len() + chunk.len() > MAX_BODY {
            return failure(413, "body_too_large");
        }
        body.extend_from_slice(&chunk);
    }
    let Ok(input) = serde_json::from_slice::<Input>(&body) else {
        return failure(400, "invalid_request");
    };
    let Some(site) = configured.get(&input.site) else {
        return failure(403, "site_denied");
    };
    if !site.origins.iter().any(|allowed| allowed == origin) {
        return failure(403, "site_denied");
    }
    let ip = request
        .headers()
        .get("CF-Connecting-IP")?
        .unwrap_or_default();
    if ip.parse::<std::net::IpAddr>().is_err() {
        return failure(403, "client_unavailable");
    }
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Origin", origin)?;
    headers.set("CF-Connecting-IP", &ip)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(
            String::from_utf8(body)
                .map_err(|_| Error::from("Invalid UTF-8."))?
                .into(),
        ));
    env.durable_object("GATES")?
        .get_by_name(&input.site)?
        .fetch_with_request(Request::new_with_init(
            &format!("https://internal{}", request.path()),
            &init,
        )?)
        .await
}

#[durable_object]
pub struct Gate {
    state: State,
    env: Env,
}

impl Gate {
    fn setup(&self) -> Result<()> {
        self.state.storage().sql().exec("CREATE TABLE IF NOT EXISTS entries (key TEXT PRIMARY KEY, count INTEGER NOT NULL, expires INTEGER NOT NULL)", None)?;
        self.state.storage().sql().exec(
            "CREATE INDEX IF NOT EXISTS entries_expiry ON entries(expires)",
            None,
        )?;
        Ok(())
    }

    fn rate(&self, key: &str, limit: i32, expiry: u64) -> Result<bool> {
        let rows = self.state.storage().sql().exec("INSERT INTO entries (key, count, expires) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1 WHERE count < ? RETURNING count", Some(vec![key.into(), (expiry as i64).into(), limit.into()]))?;
        Ok(!rows.to_array::<Value>()?.is_empty())
    }

    fn consume(&self, challenge: &Challenge) -> Result<bool> {
        let rows = self.state.storage().sql().exec(
            "INSERT OR IGNORE INTO entries (key, count, expires) VALUES (?, 1, ?) RETURNING count",
            Some(vec![
                format!("used:{}", challenge.id).into(),
                (challenge.expires_at as i64).into(),
            ]),
        )?;
        Ok(!rows.to_array::<Value>()?.is_empty())
    }

    fn cleanup(&self) -> Result<()> {
        self.state.storage().sql().exec(
            "DELETE FROM entries WHERE expires <= ?",
            Some(vec![(now() as i64).into()]),
        )?;
        Ok(())
    }

    async fn handle(&self, mut request: Request) -> Result<Response> {
        self.setup()?;
        self.cleanup()?;
        if self.state.storage().get_alarm().await?.is_none() {
            self.state
                .storage()
                .set_alarm(std::time::Duration::from_secs(60))
                .await?;
        }
        let input: Input = request.json().await?;
        let origin = request.headers().get("Origin")?.unwrap_or_default();
        let ip = request
            .headers()
            .get("CF-Connecting-IP")?
            .unwrap_or_default();
        let configured = sites(&self.env)?;
        let site = configured
            .get(&input.site)
            .ok_or_else(|| Error::from("Unknown site."))?;
        if !site.origins.contains(&origin) {
            return failure(403, "site_denied");
        }
        let key = protocol::signing_key(&self.env.secret("SIGNING_KEY")?.to_string())
            .map_err(Error::from)?;
        let time = now();
        let minute = time / 60;
        let expiry = (minute + 1) * 60;
        let client = protocol::fingerprint(&key, &input.site, &ip, minute);
        let issuing = request.path() == "/v1/challenge";
        let action = if issuing { "issue" } else { "reveal" };
        if !self.rate(
            &format!("client:{action}:{client}"),
            if issuing { 10 } else { 20 },
            expiry,
        )? || !self.rate(&format!("total:{action}:{minute}"), 300, expiry)?
        {
            return failure(429, "rate_limited");
        }
        if issuing {
            if input.token.is_some() || input.nonce.is_some() {
                return failure(400, "invalid_request");
            }
            let difficulty = self
                .env
                .var("DIFFICULTY")?
                .to_string()
                .parse::<u8>()
                .map_err(|_| Error::from("Invalid difficulty."))?;
            if !(protocol::MIN_DIFFICULTY..=protocol::MAX_DIFFICULTY).contains(&difficulty) {
                return failure(503, "unavailable");
            }
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).map_err(|_| Error::from("Randomness unavailable."))?;
            let challenge = Challenge {
                version: 1,
                id: random.iter().map(|byte| format!("{byte:02x}")).collect(),
                site: input.site,
                origin,
                client,
                difficulty,
                issued_at: time,
                expires_at: time + protocol::TTL,
            };
            return reply(
                200,
                json!({"token": protocol::issue(&key, &challenge), "difficulty": difficulty, "expires_at": challenge.expires_at, "algorithm": "sha256", "version": 1}),
            );
        }
        let (Some(token), Some(nonce)) = (input.token, input.nonce) else {
            return failure(400, "invalid_request");
        };
        let challenge =
            match protocol::verify(&key, &token, &nonce, &input.site, &origin, &ip, time) {
                Ok(challenge) => challenge,
                Err(InvalidProof::Expired) => return failure(410, "challenge_expired"),
                Err(InvalidProof::Invalid) => return failure(400, "invalid_proof"),
            };
        if !self.consume(&challenge)? {
            return failure(409, "challenge_used");
        }
        reply(200, json!({"email": site.email}))
    }
}

impl DurableObject for Gate {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, request: Request) -> Result<Response> {
        self.handle(request)
            .await
            .or_else(|_| failure(503, "unavailable"))
    }

    async fn alarm(&self) -> Result<Response> {
        self.setup()?;
        self.cleanup()?;
        let remaining = self
            .state
            .storage()
            .sql()
            .exec("SELECT key FROM entries LIMIT 1", None)?
            .to_array::<Value>()?;
        if !remaining.is_empty() {
            self.state
                .storage()
                .set_alarm(std::time::Duration::from_secs(60))
                .await?;
        }
        Response::empty()
    }
}
