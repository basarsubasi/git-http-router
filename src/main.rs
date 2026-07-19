use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use tokio_util::io::ReaderStream;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Root directory of git repositories
    #[arg(short, long, default_value = ".")]
    root: String,

    /// Basic auth username
    #[arg(short, long)]
    username: Option<String>,

    /// Basic auth password
    #[arg(long)]
    password: Option<String>,
}

struct AppState {
    root: PathBuf,
    expected_auth: Option<String>,
    expected_username: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let root = std::fs::canonicalize(&args.root).unwrap_or_else(|_| PathBuf::from(&args.root));

    let (expected_auth, expected_username) = if let (Some(u), Some(p)) = (args.username, args.password) {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        (Some(format!("Basic {}", STANDARD.encode(format!("{}:{}", &u, p)))), Some(u))
    } else {
        (None, None)
    };

    let state = Arc::new(AppState { root: root.clone(), expected_auth, expected_username });

    let app = Router::new()
        .route("/{*path}", any(handle_git_cgi))
        .route("/", any(handle_git_cgi))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    println!("Listening on http://{} serving git from {:?}", addr, root);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_git_cgi(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Response, StatusCode> {
    if let Some(expected_auth) = &state.expected_auth {
        let auth_header = request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        
        if auth_header != expected_auth {
            let mut response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from("Unauthorized"))
                .unwrap();
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Basic realm=\"git-http-router\""),
            );
            return Ok(response);
        }
    }

    let method = request.method().as_str().to_string();
    let uri = request.uri().clone();
    let mut path_info = uri.path().to_string();
    while path_info.contains("//") {
        path_info = path_info.replace("//", "/");
    }
    let query_string = uri.query().unwrap_or("").to_string();
    
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let mut cmd = Command::new("git");
    cmd.arg("http-backend");
    
    // Set CGI environment variables
    cmd.env("GIT_PROJECT_ROOT", &state.root);
    cmd.env("GIT_HTTP_EXPORT_ALL", "1");
    cmd.env("REQUEST_METHOD", method);
    cmd.env("PATH_INFO", path_info);
    cmd.env("QUERY_STRING", query_string);
    if !content_type.is_empty() {
        cmd.env("CONTENT_TYPE", content_type);
    }
    if let Some(username) = &state.expected_username {
        cmd.env("REMOTE_USER", username);
    }

    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::inherit());

    let mut child = cmd.spawn().map_err(|e| {
        eprintln!("Failed to spawn git http-backend: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Pipe request body to child stdin
    if let Some(mut stdin) = child.stdin.take() {
        let mut body_stream = request.into_body().into_data_stream();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            while let Some(Ok(chunk)) = body_stream.next().await {
                if stdin.write_all(&chunk).await.is_err() {
                    break;
                }
            }
        });
    }

    let mut stdout = child.stdout.take().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Read headers from stdout until \r\n\r\n or \n\n
    let mut header_buffer = Vec::new();
    let mut buf = [0u8; 1];
    
    let mut header_end_found = false;
    while let Ok(n) = stdout.read(&mut buf).await {
        if n == 0 { break; }
        header_buffer.push(buf[0]);
        
        let len = header_buffer.len();
        if len >= 4 && &header_buffer[len-4..] == b"\r\n\r\n" {
            header_end_found = true;
            break;
        } else if len >= 2 && &header_buffer[len-2..] == b"\n\n" {
            header_end_found = true;
            break;
        }
    }

    if !header_end_found {
        eprintln!("Failed to read valid CGI headers");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let header_str = String::from_utf8_lossy(&header_buffer);
    let mut response_builder = Response::builder().status(StatusCode::OK);
    let mut response_headers = HeaderMap::new();

    for line in header_str.lines() {
        if line.trim().is_empty() { continue; }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            if k.eq_ignore_ascii_case("status") {
                if let Some(code) = v.split_whitespace().next() {
                    if let Ok(c) = code.parse::<u16>() {
                        response_builder = response_builder.status(c);
                    }
                }
            } else if let Ok(name) = HeaderName::from_bytes(k.as_bytes()) {
                if let Ok(value) = HeaderValue::from_str(v) {
                    response_headers.insert(name, value);
                }
            }
        }
    }

    // Wrap the rest of stdout in a stream
    let body = Body::from_stream(ReaderStream::new(stdout));
    
    let mut response = response_builder.body(body).unwrap();
    *response.headers_mut() = response_headers;
    
    Ok(response)
}
