use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// One scripted answer: the status line's code and the body.
struct Reply {
    status: u16,
    body: &'static [u8],
}

/// What the server saw of one request: the request line and the
/// `Authorization` header, if any.
#[derive(Debug, Clone)]
struct Seen {
    request_line: String,
    authorization: Option<String>,
}

/// A tiny HTTP server that answers one connection per scripted reply, in
/// order, and records what each request carried.
fn serve(replies: Vec<Reply>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut authorization = None;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("authorization") {
                        authorization = Some(value.trim().to_string());
                    }
                }
            }
            log.lock().unwrap_or_else(|e| e.into_inner()).push(Seen {
                request_line: request_line.trim().to_string(),
                authorization,
            });
            let head = format!(
                "HTTP/1.1 {} X\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.status,
                reply.body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(reply.body).unwrap();
            stream.flush().unwrap();
            let _ = reader.read(&mut [0u8; 1]);
        }
    });
    (format!("http://{}", addr), seen)
}

const PDF_PATH: &str = "/guides/g-1/export/guide.pdf";
const PDF: &[u8] = b"%PDF-1.7\n\x00\xff binary\r\n%%EOF";

async fn no_refresh() -> Result<Option<String>, String> {
    panic!("no refresh expected")
}

#[tokio::test]
async fn a_2xx_saves_the_body_from_the_api_base_with_the_bearer() {
    let (base, seen) = serve(vec![Reply {
        status: 200,
        body: PDF,
    }]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("tok-1", PDF_PATH, Some(&base), dir.path(), no_refresh).await;

    assert_eq!(outcome, Ok(PdfOutcome::Saved));
    assert_eq!(std::fs::read(dir.path().join("guide.pdf")).unwrap(), PDF);
    let seen = seen.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].request_line, format!("GET {PDF_PATH} HTTP/1.1"));
    assert_eq!(seen[0].authorization.as_deref(), Some("Bearer tok-1"));
}

#[tokio::test]
async fn a_401_refreshes_and_retries_once_with_the_new_token() {
    let (base, seen) = serve(vec![
        Reply {
            status: 401,
            body: b"",
        },
        Reply {
            status: 200,
            body: PDF,
        },
    ]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("old", PDF_PATH, Some(&base), dir.path(), || async {
        Ok(Some("new".to_string()))
    })
    .await;

    assert_eq!(outcome, Ok(PdfOutcome::Saved));
    let seen = seen.lock().unwrap_or_else(|e| e.into_inner());
    let auth: Vec<_> = seen.iter().map(|s| s.authorization.clone()).collect();
    assert_eq!(
        auth,
        vec![
            Some("Bearer old".to_string()),
            Some("Bearer new".to_string())
        ]
    );
}

#[tokio::test]
async fn a_second_401_is_an_error_without_a_third_request() {
    let (base, seen) = serve(vec![
        Reply {
            status: 401,
            body: b"",
        },
        Reply {
            status: 401,
            body: b"",
        },
        Reply {
            status: 200,
            body: PDF,
        },
    ]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("old", PDF_PATH, Some(&base), dir.path(), || async {
        Ok(Some("new".to_string()))
    })
    .await;

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(seen.lock().unwrap_or_else(|e| e.into_inner()).len(), 2);
    assert!(!dir.path().join("guide.pdf").exists());
}

#[tokio::test]
async fn a_401_with_no_session_to_refresh_is_an_error() {
    let (base, seen) = serve(vec![Reply {
        status: 401,
        body: b"",
    }]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("old", PDF_PATH, Some(&base), dir.path(), || async {
        Ok(None)
    })
    .await;

    assert!(outcome.is_err(), "{outcome:?}");
    assert_eq!(seen.lock().unwrap_or_else(|e| e.into_inner()).len(), 1);
}

#[tokio::test]
async fn a_404_is_no_pdf_and_writes_nothing() {
    let (base, _seen) = serve(vec![Reply {
        status: 404,
        body: b"{\"detail\":\"Not Found\"}",
    }]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("tok", PDF_PATH, Some(&base), dir.path(), no_refresh).await;

    assert_eq!(outcome, Ok(PdfOutcome::NotFound));
    assert!(!dir.path().join("guide.pdf").exists());
}

#[tokio::test]
async fn another_status_is_an_error_naming_it() {
    let (base, _seen) = serve(vec![Reply {
        status: 500,
        body: b"",
    }]);
    let dir = tempfile::tempdir().unwrap();

    let outcome = download_pdf_with("tok", PDF_PATH, Some(&base), dir.path(), no_refresh).await;

    let error = outcome.expect_err("a 500 is not a PDF");
    assert!(error.contains("500"), "{error}");
    assert!(!dir.path().join("guide.pdf").exists());
}

#[tokio::test]
async fn a_pdf_url_that_is_not_a_path_makes_no_request() {
    let (base, seen) = serve(vec![Reply {
        status: 200,
        body: PDF,
    }]);
    let dir = tempfile::tempdir().unwrap();

    for pdf_url in [
        format!("{base}{PDF_PATH}"),
        format!("//{}{PDF_PATH}", base.trim_start_matches("http://")),
        "guides/g-1/export/guide.pdf".into(),
    ] {
        let outcome = download_pdf_with("tok", &pdf_url, Some(&base), dir.path(), no_refresh).await;
        let error = outcome.expect_err(&pdf_url);
        assert!(error.contains("not an API path"), "{pdf_url}: {error}");
    }

    assert!(seen.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
    assert!(!dir.path().join("guide.pdf").exists());
}
