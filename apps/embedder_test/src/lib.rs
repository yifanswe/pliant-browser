pub fn fixture_response(path: &str) -> (&'static str, &'static str) {
    match path {
        "/a" => (
            "200 OK",
            "<!doctype html><meta charset=\"utf-8\"><title>Pliant manual fixture A</title><style>body{font:18px system-ui;background:#e8f2ff;padding:32px}h1{color:#174f89}</style><h1>Fixture A</h1><p id=\"identity\">pliant-manual-a</p>",
        ),
        "/b" => (
            "200 OK",
            "<!doctype html><meta charset=\"utf-8\"><title>Pliant manual fixture B</title><style>body{font:18px system-ui;background:#fff2df;padding:32px}h1{color:#8a4b08}</style><h1>Fixture B</h1><p id=\"identity\">pliant-manual-b</p>",
        ),
        _ => (
            "404 Not Found",
            "<!doctype html><meta charset=\"utf-8\"><title>Not found</title><h1>Not found</h1>",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::fixture_response;

    #[test]
    fn fixture_pages_are_visibly_distinct_and_unknown_paths_fail() {
        let (status_a, body_a) = fixture_response("/a");
        let (status_b, body_b) = fixture_response("/b");
        let (status_missing, _) = fixture_response("/missing");

        assert_eq!(status_a, "200 OK");
        assert_eq!(status_b, "200 OK");
        assert_ne!(body_a, body_b);
        assert!(body_a.contains("pliant-manual-a"));
        assert!(body_b.contains("pliant-manual-b"));
        assert_eq!(status_missing, "404 Not Found");
    }
}
