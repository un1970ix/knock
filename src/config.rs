use serde::Deserialize;
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub origins: Vec<String>,
    pub email: String,
}

pub type Sites = BTreeMap<String, Site>;

pub fn parse_sites(json: &str) -> Result<Sites, &'static str> {
    let sites: Sites = serde_json::from_str(json).map_err(|_| "Invalid site configuration.")?;
    if sites.is_empty() || sites.len() > 100 {
        return Err("Configure between 1 and 100 sites.");
    }
    for (id, site) in &sites {
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || site.origins.is_empty()
            || site.origins.len() > 10
            || !site.origins.iter().all(|origin| valid_origin(origin))
            || !valid_email(&site.email)
        {
            return Err("Invalid site configuration.");
        }
    }
    Ok(sites)
}

pub fn valid_origin(origin: &str) -> bool {
    let Ok(url) = Url::parse(origin) else {
        return false;
    };
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    origin.len() <= 255
        && url.origin().ascii_serialization() == origin
        && (url.scheme() == "https" || (local && url.scheme() == "http"))
}

pub fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !domain.is_empty()
        && email.len() <= 254
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}
