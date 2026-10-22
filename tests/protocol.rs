use knock::{
    config::{parse_sites, valid_origin},
    protocol::*,
};

const KEY: [u8; 32] = [42; 32];
const NOW: u64 = 1_800_000_000;
const IP: &str = "192.0.2.1";

fn challenge() -> Challenge {
    Challenge {
        version: 1,
        id: "0123456789abcdef0123456789abcdef".into(),
        site: "example".into(),
        origin: "https://example.com".into(),
        client: fingerprint(&KEY, "example", IP, NOW / 60),
        difficulty: MIN_DIFFICULTY,
        issued_at: NOW,
        expires_at: NOW + TTL,
    }
}

fn solution(token: &str) -> String {
    (0..u32::MAX)
        .map(|n| n.to_string())
        .find(|n| valid_work(token, n, MIN_DIFFICULTY))
        .unwrap()
}

fn check(token: &str, nonce: &str, time: u64) -> Result<Challenge, InvalidProof> {
    verify(
        &KEY,
        token,
        nonce,
        "example",
        "https://example.com",
        IP,
        time,
    )
}

#[test]
fn valid_proof_and_binding_boundaries() {
    let token = issue(&KEY, &challenge());
    let nonce = solution(&token);
    assert!(check(&token, &nonce, NOW).is_ok());
    let contexts = [
        ([41; 32], "example", "https://example.com", IP, NOW),
        (KEY, "other", "https://example.com", IP, NOW),
        (KEY, "example", "https://example.org", IP, NOW),
        (KEY, "example", "https://example.com", "192.0.2.2", NOW),
        (KEY, "example", "https://example.com", IP, NOW - 1),
    ];
    for (key, site, origin, ip, time) in contexts {
        let result = verify(&key, &token, &nonce, site, origin, ip, time);
        assert_eq!(result.unwrap_err(), InvalidProof::Invalid);
    }
    let expired = check(&token, &nonce, NOW + TTL);
    assert_eq!(expired.unwrap_err(), InvalidProof::Expired);
}

#[test]
fn rejects_tampering_and_noncanonical_proofs() {
    let token = issue(&KEY, &challenge());
    let nonce = solution(&token);
    let invalid = [
        String::new(),
        format!("0{nonce}"),
        format!("+{nonce}"),
        format!(" {nonce}"),
        "4294967296".into(),
        "1e5".into(),
    ];
    for bad in invalid {
        assert_eq!(check(&token, &bad, NOW).unwrap_err(), InvalidProof::Invalid);
    }
    let mut tampered = token.clone().into_bytes();
    tampered[0] = if tampered[0] == b'A' { b'B' } else { b'A' };
    let tampered = std::str::from_utf8(&tampered).unwrap();
    assert!(check(tampered, &nonce, NOW).is_err());
    let wrong = (0..u32::MAX)
        .map(|n| n.to_string())
        .find(|n| !valid_work(&token, n, MIN_DIFFICULTY))
        .unwrap();
    assert!(check(&token, &wrong, NOW).is_err());
    let mut unsupported = challenge();
    unsupported.difficulty = 0;
    assert!(check(&issue(&KEY, &unsupported), "0", NOW).is_err());
    unsupported = challenge();
    unsupported.expires_at += 1;
    assert!(check(&issue(&KEY, &unsupported), "0", NOW).is_err());
}

#[test]
fn fingerprints_are_scoped_and_rotate() {
    let first = fingerprint(&KEY, "example", IP, NOW / 60);
    assert_ne!(first, fingerprint(&KEY, "other", IP, NOW / 60));
    assert_ne!(first, fingerprint(&KEY, "example", IP, NOW / 60 + 1));
    assert!(!first.contains(IP));
}

#[test]
fn configuration_rejects_ambiguous_origins_and_email_injection() {
    let valid = [
        "https://example.com",
        "http://localhost:8787",
        "http://127.0.0.1:8000",
        "http://[::1]:8000",
    ];
    for origin in valid {
        assert!(valid_origin(origin));
    }
    let invalid = [
        "*",
        "null",
        "http://example.com",
        "https://example.com/",
        "https://example.com/path",
        "https://user@example.com",
        "https://EXAMPLE.com",
        "https://example.com?x=1",
        "https://example.com#x",
    ];
    for origin in invalid {
        assert!(!valid_origin(origin));
    }
    let config = r#"{"example":{"origins":["https://example.com"],"email":"hello@example.com"}}"#;
    assert!(parse_sites(config).is_ok());
    let invalid = [
        "hello@example.com?subject=spam",
        "hello@example.com\r\nBcc: x@example.org",
        "hello@other@example.com",
        "<hello@example.com>",
        "hello@example.com ",
    ];
    for email in invalid {
        let config =
            serde_json::json!({"example":{"origins":["https://example.com"],"email":email}});
        assert!(parse_sites(&config.to_string()).is_err());
    }
    assert!(signing_key("weak").is_err());
}
