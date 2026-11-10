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

use crate::game::Ruleset;
use crate::rng::Rng;
use crate::sim::{run_benchmark_with_ruleset, simulate_match_with_ruleset};
use crate::strategy::{
    AdaptiveStrideUpAttack, AntiMidpointDefense, AttackStrategy, BaitAndHideDefense,
    BayesianPdfAttack, CheckerboardAttack, ClusteredDefense, CornerEdgeDefense,
    CrosshairDescendingAttack, CrosshairStrideAttack, DefenseStrategy, DescendingStrideAttack,
    HuntAndTargetAttack, LargestRectMidpointAttack, MonteCarloAttack, ParallelLanesDefense,
    RandomDefense, RectCheckerboardAttack, RectDescendingStrideAttack, RectStrideFourAttack,
    SpacedDefense, StaggeredFourToTwoAttack,
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
        "largest_rect" | "rect" | "largest_rectangle" | "midpoint" => (
            Box::new(LargestRectMidpointAttack::new()),
            "Largest Empty Rect Midpoint",
        ),
        "adaptive_stride" | "adaptive" | "stride_up" => (
            Box::new(AdaptiveStrideUpAttack::new()),
            "Adaptive Stride (2->3->4->5)",
        ),
        "stride4_to_2" | "staggered" | "staggered_stride" | "stride4" => (
            Box::new(StaggeredFourToTwoAttack::new()),
            "Staggered Stride (4->2)",
        ),
        "rect_stride4" | "rect_stride" | "rect4" | "largest_rect_stride" => (
            Box::new(RectStrideFourAttack::new()),
            "Largest Rect + Stride (4->2)",
        ),
        "stride_5432" | "descending_stride" | "stride5432" | "descending" => (
            Box::new(DescendingStrideAttack::new()),
            "Descending Stride (5->4->3->2)",
        ),
        "rect_stride_5432" | "rect_descending" | "rect_stride5432" | "largest_rect_descending" => (
            Box::new(RectDescendingStrideAttack::new()),
            "Largest Rect + Stride (5->4->3->2)",
        ),
        "rect_checkerboard" | "rect_parity" | "largest_rect_checkerboard" => (
            Box::new(RectCheckerboardAttack::new()),
            "Largest Rect + Checkerboard",
        ),
        "monte_carlo" | "mc" | "sampling" | "monte_carlo_sampling" => (
            Box::new(MonteCarloAttack::new()),
            "Monte Carlo Sampling",
        ),
        "bayesian_pdf" | "bayesian" | "pdf" | "probability_density" | "bayes" => (
            Box::new(BayesianPdfAttack::new()),
            "Bayesian Probability Density",
        ),
        "crosshair_stride" | "crosshair" | "cross" | "open_target" => (
            Box::new(CrosshairStrideAttack::new()),
            "Crosshair Stride + Open-Target",
        ),
        "crosshair_5432" | "crosshair_descending" | "crosshair5432" => (
            Box::new(CrosshairDescendingAttack::new()),
            "Crosshair Stride (5->4->3->2)",
        ),
        _ => (
            Box::new(CrosshairDescendingAttack::new()),
            "Crosshair Stride (5->4->3->2)",
        ),
    }
}

/// Instantiate a defense strategy from its CLI / URL string identifier.
pub fn create_defense(id: &str) -> (Box<dyn DefenseStrategy>, &'static str) {
    match id.to_ascii_lowercase().as_str() {
        "random" | "uniform" | "uniform_random" => (
            Box::new(RandomDefense::new()),
            "Uniform Random",
        ),
        "spaced" | "no_touch" | "spaced_defense" => (
            Box::new(SpacedDefense::new()),
            "Spaced (No-Touching)",
        ),
        "corner_edge" | "corner" | "edge" | "corners" => (
            Box::new(CornerEdgeDefense::new()),
            "Corner & Edge Hugging",
        ),
        "clustered" | "phalanx" | "cluster" => (
            Box::new(ClusteredDefense::new()),
            "Tightly Clustered Phalanx",
        ),
        "anti_midpoint" | "midpoint_dodger" | "bisection_dodger" => (
            Box::new(AntiMidpointDefense::new()),
            "Anti-Midpoint (Bisection Dodger)",
        ),
        "bait_and_hide" | "bait" | "corner_destroyer" => (
            Box::new(BaitAndHideDefense::new()),
            "Bait Capital + Corner Destroyer",
        ),
        "parallel_lanes" | "lanes" | "parallel" => (
            Box::new(ParallelLanesDefense::new()),
            "Parallel Lanes (Single Axis)",
        ),
        _ => (
            Box::new(RandomDefense::new()),
            "Uniform Random",
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
    let d1_id = query.get("d1").map(|s| s.as_str()).unwrap_or("random");
    let d2_id = query.get("d2").map(|s| s.as_str()).unwrap_or("random");

    let ruleset = query
        .get("ruleset")
        .and_then(|s| Ruleset::from_id(s))
        .unwrap_or(Ruleset::Standard);

    let seed = query
        .get("seed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| Rng::from_time().next_u64());

    let (mut p1_att, p1_name) = create_strategy(p1_id);
    let (mut p2_att, p2_name) = create_strategy(p2_id);
    let (mut p1_def, _) = create_defense(d1_id);
    let (mut p2_def, _) = create_defense(d2_id);

    let replay = simulate_match_with_ruleset(
        p1_name,
        p1_att.as_mut(),
        p1_def.as_mut(),
        p2_name,
        p2_att.as_mut(),
        p2_def.as_mut(),
        ruleset,
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

    let ruleset = query
        .get("ruleset")
        .and_then(|s| Ruleset::from_id(s))
        .unwrap_or(Ruleset::Standard);

    let seed = query
        .get("seed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| Rng::from_time().next_u64());

    let report = run_benchmark_with_ruleset(games, seed, ruleset);
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

        let (s3, name3) = create_strategy("largest_rect");
        assert_eq!(name3, "Largest Empty Rect Midpoint");
        assert_eq!(s3.current_mode(), "largest_rect");

        let (s4, name4) = create_strategy("adaptive_stride");
        assert_eq!(name4, "Adaptive Stride (2->3->4->5)");
        assert_eq!(s4.current_mode(), "adaptive_stride");

        let (s5, name5) = create_strategy("stride4_to_2");
        assert_eq!(name5, "Staggered Stride (4->2)");
        assert_eq!(s5.current_mode(), "stride4_to_2");

        let (s6, name6) = create_strategy("rect_stride4");
        assert_eq!(name6, "Largest Rect + Stride (4->2)");
        assert_eq!(s6.current_mode(), "rect_stride4");

        let (s7, name7) = create_strategy("stride_5432");
        assert_eq!(name7, "Descending Stride (5->4->3->2)");
        assert_eq!(s7.current_mode(), "stride_5432");

        let (s8, name8) = create_strategy("rect_stride_5432");
        assert_eq!(name8, "Largest Rect + Stride (5->4->3->2)");
        assert_eq!(s8.current_mode(), "rect_stride_5432");

        let (s9, name9) = create_strategy("rect_checkerboard");
        assert_eq!(name9, "Largest Rect + Checkerboard");
        assert_eq!(s9.current_mode(), "rect_checkerboard");

        let (s10, name10) = create_strategy("monte_carlo");
        assert_eq!(name10, "Monte Carlo Sampling");
        assert_eq!(s10.current_mode(), "monte_carlo");

        let (s11, name11) = create_strategy("bayesian_pdf");
        assert_eq!(name11, "Bayesian Probability Density");
        assert_eq!(s11.current_mode(), "bayesian_pdf");

        let (s12, name12) = create_strategy("crosshair_stride");
        assert_eq!(name12, "Crosshair Stride + Open-Target");
        assert_eq!(s12.current_mode(), "crosshair_stride");

        let (s13, name13) = create_strategy("crosshair_5432");
        assert_eq!(name13, "Crosshair Stride (5->4->3->2)");
        assert_eq!(s13.current_mode(), "crosshair_5432");
    }

    #[test]
    fn test_create_defense() {
        let (d1, name1) = create_defense("random");
        assert_eq!(name1, "Uniform Random");
        assert_eq!(d1.id(), "random");

        let (d2, name2) = create_defense("spaced");
        assert_eq!(name2, "Spaced (No-Touching)");
        assert_eq!(d2.id(), "spaced");

        let (d3, name3) = create_defense("corner_edge");
        assert_eq!(name3, "Corner & Edge Hugging");
        assert_eq!(d3.id(), "corner_edge");

        let (d4, name4) = create_defense("clustered");
        assert_eq!(name4, "Tightly Clustered Phalanx");
        assert_eq!(d4.id(), "clustered");

        let (d5, name5) = create_defense("anti_midpoint");
        assert_eq!(name5, "Anti-Midpoint (Bisection Dodger)");
        assert_eq!(d5.id(), "anti_midpoint");

        let (d6, name6) = create_defense("bait_and_hide");
        assert_eq!(name6, "Bait Capital + Corner Destroyer");
        assert_eq!(d6.id(), "bait_and_hide");

        let (d7, name7) = create_defense("parallel_lanes");
        assert_eq!(name7, "Parallel Lanes (Single Axis)");
        assert_eq!(d7.id(), "parallel_lanes");
    }
}
