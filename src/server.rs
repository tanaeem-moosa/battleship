//! Lightweight HTTP static file and JSON API server using `std::net::TcpListener`.
//!
//! Serves:
//! - Static web viewer assets from `viewer/` (`/index.html`, `/styles.css`, `/app.js`, `/sample_data.js`)
//! - REST API endpoints:
//!   - `GET /api/match?p1=random_hunt&p2=checkerboard&seed=42`
//!   - `GET /api/benchmark?games=500&seed=42`

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::rng::Rng;
use crate::sim::{run_benchmark, simulate_match};
use crate::strategy::{
    AttackStrategy, CheckerboardAttack, HuntAndTargetAttack, RandomDefense,
};

/// Instantiate an attack strategy from its CLI / URL string identifier.
pub fn create_strategy(id: &str) -> (Box<dyn AttackStrategy>, &'static str) {
    match id.to_ascii_lowercase().as_str() {
        "random_hunt" | "hunt" | "random" => (
            Box::new(HuntAndTargetAttack::new()),
            "Random + Hunt/Target",
        ),
        "checkerboard" | "checkers" | "parity" => (
            Box::new(CheckerboardAttack::new()),
            "Checkerboard + Hunt/Target",
        ),
        _ => (
            Box::new(CheckerboardAttack::new()),
            "Checkerboard + Hunt/Target",
        ),
    }
}

/// Start the HTTP server on `127.0.0.1:{port}`.
pub fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;

    println!("================================================================================");
    println!("             BATTLESHIP SIMULATOR ENGINE HTTP SERVER RUNNING                    ");
    println!("================================================================================");
    println!("Web Viewer: http://{}", addr);
    println!();
    println!("Available API endpoints:");
    println!("  - http://{}/api/match?p1=random_hunt&p2=checkerboard&seed=42", addr);
    println!("  - http://{}/api/benchmark?games=500&seed=42", addr);
    println!();
    println!("Press Ctrl+C to stop the server.");
    println!("================================================================================");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_client(stream) {
                    // Log connection errors without terminating server loop
                    eprintln!("HTTP client handler error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Incoming connection error: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();

    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    // Read remaining request headers until empty line
    let mut header_line = String::new();
    while reader.read_line(&mut header_line)? > 0 {
        if header_line == "\r\n" || header_line == "\n" {
            break;
        }
        header_line.clear();
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(
            &mut stream,
            400,
            "text/plain; charset=utf-8",
            b"400 Bad Request\n",
        );
    }

    let method = parts[0];
    let uri = parts[1];

    if method != "GET" && method != "HEAD" {
        return send_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"405 Method Not Allowed\n",
        );
    }

    // Parse path and query parameters
    let (path, query_str) = match uri.find('?') {
        Some(idx) => (&uri[..idx], &uri[idx + 1..]),
        None => (uri, ""),
    };
    let query = parse_query(query_str);

    // Route requests
    if path == "/api/match" {
        handle_api_match(&mut stream, &query)
    } else if path == "/api/benchmark" {
        handle_api_benchmark(&mut stream, &query)
    } else {
        handle_static_file(&mut stream, path)
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut kv = pair.splitn(2, '=');
        if let Some(key) = kv.next() {
            let val = kv.next().unwrap_or("");
            // Basic URL percent-decoding for '+' and '%20'
            let decoded = val.replace('+', " ").replace("%20", " ");
            map.insert(key.to_string(), decoded);
        }
    }
    map
}

fn handle_api_match(
    stream: &mut TcpStream,
    query: &HashMap<String, String>,
) -> std::io::Result<()> {
    let p1_id = query.get("p1").map(|s| s.as_str()).unwrap_or("random_hunt");
    let p2_id = query.get("p2").map(|s| s.as_str()).unwrap_or("checkerboard");

    let seed = query
        .get("seed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| Rng::from_time().next_u64());

    let (mut p1_att, p1_name) = create_strategy(p1_id);
    let (mut p2_att, p2_name) = create_strategy(p2_id);
    let mut p1_def = RandomDefense::new();
    let mut p2_def = RandomDefense::new();

    let replay = simulate_match(
        p1_name,
        p1_att.as_mut(),
        &mut p1_def,
        p2_name,
        p2_att.as_mut(),
        &mut p2_def,
        seed,
    );

    let json = replay.to_json();
    send_response(
        stream,
        200,
        "application/json; charset=utf-8",
        json.as_bytes(),
    )
}

fn handle_api_benchmark(
    stream: &mut TcpStream,
    query: &HashMap<String, String>,
) -> std::io::Result<()> {
    let games = query
        .get("games")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(200)
        .min(5000);

    let seed = query
        .get("seed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| Rng::from_time().next_u64());

    let report = run_benchmark(games, seed);
    let json = report.to_json();

    send_response(
        stream,
        200,
        "application/json; charset=utf-8",
        json.as_bytes(),
    )
}

fn handle_static_file(stream: &mut TcpStream, path: &str) -> std::io::Result<()> {
    // Sanitize path: strip leading slash, forbid '..'
    let rel = path.trim_start_matches('/');
    let target = if rel.is_empty() || rel == "index.html" {
        "index.html"
    } else {
        rel
    };

    if target.contains("..") {
        return send_response(
            stream,
            403,
            "text/plain; charset=utf-8",
            b"403 Forbidden\n",
        );
    }

    let file_path = PathBuf::from("viewer").join(target);

    if file_path.is_file() {
        match fs::read(&file_path) {
            Ok(bytes) => {
                let content_type = mime_type(&file_path);
                send_response(stream, 200, content_type, &bytes)
            }
            Err(_) => send_response(
                stream,
                500,
                "text/plain; charset=utf-8",
                b"500 Internal Server Error\n",
            ),
        }
    } else {
        send_response(
            stream,
            404,
            "text/plain; charset=utf-8",
            format!("404 Not Found: {}\n", target).as_bytes(),
        )
    }
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn send_response(
    stream: &mut TcpStream,
    status_code: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status_line = match status_code {
        200 => "HTTP/1.1 200 OK",
        400 => "HTTP/1.1 400 Bad Request",
        403 => "HTTP/1.1 403 Forbidden",
        404 => "HTTP/1.1 404 Not Found",
        405 => "HTTP/1.1 405 Method Not Allowed",
        500 => "HTTP/1.1 500 Internal Server Error",
        _ => "HTTP/1.1 200 OK",
    };

    let header = format!(
        "{}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, OPTIONS\r\n\
         Connection: close\r\n\
         \r\n",
        status_line,
        content_type,
        body.len()
    );

    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query() {
        let q = "p1=random_hunt&p2=checkerboard&seed=123";
        let map = parse_query(q);
        assert_eq!(map.get("p1").unwrap(), "random_hunt");
        assert_eq!(map.get("p2").unwrap(), "checkerboard");
        assert_eq!(map.get("seed").unwrap(), "123");
    }

    #[test]
    fn test_create_strategy() {
        let (s1, name1) = create_strategy("random_hunt");
        assert_eq!(name1, "Random + Hunt/Target");
        assert_eq!(s1.current_mode(), "hunt");

        let (s2, name2) = create_strategy("checkerboard");
        assert_eq!(name2, "Checkerboard + Hunt/Target");
        assert_eq!(s2.current_mode(), "checkerboard");
    }
}
