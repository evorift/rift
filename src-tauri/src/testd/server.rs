//! The listener, the guard chain, and the six endpoints.
//!
//! Guard order per connection, and it matters:
//!   1. **source IP allowlist** — cheapest check, and the rejection worth auditing most;
//!   2. **bearer token**, constant-time, on everything except `/health`;
//!   3. **sandbox path resolution**, for any request that names a file;
//!   4. handler.
//!
//! The allowlist is treated as necessary but never sufficient: a host on the same LAN can put
//! the controller's address in a packet, so passing step 1 proves nothing on its own. Step 2 is
//! what actually authenticates, and step 3 is what bounds the damage if both are somehow beaten.

use std::io::BufReader;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::audit::{Audit, Entry};
use super::config::Config;
use super::http::{self, Head};
use super::jobs::{self, JobSpec};
use super::recovery::{self, Deadman};
use super::sandbox;
use super::token;
use super::util::json_escape;

/// Concurrent connections. The controller is a single script; anything beyond a handful is
/// either a bug or an attempt to exhaust the agent's threads.
const MAX_CONNECTIONS: usize = 8;
/// Per-read/write socket timeout. Long enough to push a large build over a slow LAN link,
/// short enough that a stalled peer cannot hold a connection slot indefinitely.
const SOCKET_TIMEOUT: Duration = Duration::from_secs(120);
/// Cap for JSON request bodies (`/run`). Uploads use `config.max_upload_bytes` instead.
const MAX_JSON_BODY: u64 = 64 * 1024;
/// Cap for the `?tail=` convenience on `GET /job/<id>`.
const MAX_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug)]
pub enum ServeError {
    Bind { addr: SocketAddr, source: std::io::Error },
    /// Refusing to serve on the unspecified address. Config validation catches this first; the
    /// check is repeated here so the invariant holds even if `Config` is built another way.
    RefusedUnspecifiedBind,
    Sandbox(sandbox::PathError),
    Audit(std::io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind { addr, source } => {
                write!(f, "cannot bind {addr}: {source}")?;
                // AddrNotAvailable means the address is not assigned to any local interface.
                // The non-obvious way to hit this is a Tailscale address: `tailscale ip -4`
                // reports it and traffic to it works, but if tailscaled is in
                // userspace-networking mode the address is never put on an OS interface, so no
                // other process can bind it. The raw OS message ("The requested address is not
                // valid in its context") gives no hint of that, and it costs an evening.
                if source.kind() == std::io::ErrorKind::AddrNotAvailable {
                    write!(
                        f,
                        " -- that address is not assigned to any interface on this machine. \
                         If it is a Tailscale address (100.64.0.0/10), check that tailscaled is \
                         NOT in userspace-networking mode: `Get-NetIPAddress -AddressFamily IPv4` \
                         must list it, not just `tailscale ip -4`. Otherwise pick an address this \
                         machine actually holds."
                    )?;
                }
                Ok(())
            }
            Self::RefusedUnspecifiedBind => {
                write!(f, "refusing to bind the unspecified address (0.0.0.0 / ::)")
            }
            Self::Sandbox(e) => write!(f, "sandbox root unusable: {e}"),
            Self::Audit(e) => write!(f, "audit log unusable: {e}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// Everything a connection handler needs. Immutable after startup, so it is shared by `Arc` with
/// no locking on the request path.
struct Agent {
    cfg: Config,
    token: String,
    canonical_root: PathBuf,
    audit: Arc<Audit>,
    deadman: Arc<Deadman>,
    started: Instant,
}

/// Bind, arm the deadman, and serve until the process is stopped.
pub fn run(cfg: Config, shared_token: String) -> Result<(), ServeError> {
    // Belt and braces: `Config::load` already rejects this, but a bind to 0.0.0.0 is the single
    // failure this agent must never have, so the check is repeated at the point of no return.
    if cfg.bind_ip.is_unspecified() {
        return Err(ServeError::RefusedUnspecifiedBind);
    }

    let canonical_root = sandbox::canonical_root(&cfg.sandbox_root).map_err(ServeError::Sandbox)?;
    let audit = Arc::new(Audit::open(&cfg.state_dir).map_err(ServeError::Audit)?);

    let deadman = Arc::new(Deadman::new(
        Duration::from_secs(cfg.deadman_secs),
        cfg.state_dir.clone(),
        canonical_root.clone(),
    ));
    deadman.spawn_watchdog(Arc::clone(&audit));

    let addr = SocketAddr::new(cfg.bind_ip, cfg.port);
    let listener =
        TcpListener::bind(addr).map_err(|source| ServeError::Bind { addr, source })?;

    audit.note(&format!(
        "agent started: bound {addr}, sandbox {}, allowlist [{}], deadman {}s",
        canonical_root.display(),
        cfg.allowlist.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" "),
        cfg.deadman_secs
    ));
    eprintln!("[testd] listening on {addr}, sandbox {}", canonical_root.display());

    let agent = Arc::new(Agent {
        cfg,
        token: shared_token,
        canonical_root,
        audit,
        deadman,
        started: Instant::now(),
    });
    let live = Arc::new(AtomicUsize::new(0));

    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[testd] accept failed: {e}");
                continue;
            }
        };
        let agent = Arc::clone(&agent);
        let live = Arc::clone(&live);

        if live.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            live.fetch_sub(1, Ordering::SeqCst);
            let mut stream = stream;
            // Best-effort: the peer is being turned away anyway, and a write error here has no
            // recovery beyond dropping the socket, which happens next regardless.
            let _ = http::write_error(&mut stream, 503, "too many concurrent connections");
            agent.audit.note("rejected a connection: concurrency limit reached");
            continue;
        }

        let spawned = std::thread::Builder::new()
            .name("testd-conn".to_string())
            .spawn(move || {
                handle_connection(&agent, stream);
                live.fetch_sub(1, Ordering::SeqCst);
            });
        if let Err(e) = spawned {
            eprintln!("[testd] connection thread not started: {e}");
        }
    }
    Ok(())
}

fn peer_of(stream: &TcpStream) -> Option<IpAddr> {
    stream.peer_addr().ok().map(|a| a.ip())
}

fn handle_connection(agent: &Agent, mut stream: TcpStream) {
    // Both directions bounded, so neither a slow-loris header nor a peer that stops reading a
    // large /pull can pin a thread forever.
    if let Err(e) = stream.set_read_timeout(Some(SOCKET_TIMEOUT)) {
        eprintln!("[testd] set_read_timeout failed: {e}");
        return;
    }
    if let Err(e) = stream.set_write_timeout(Some(SOCKET_TIMEOUT)) {
        eprintln!("[testd] set_write_timeout failed: {e}");
        return;
    }

    let peer = match peer_of(&stream) {
        Some(p) => p,
        None => return, // socket already dead; nothing to audit and nobody to answer
    };
    let peer_text = peer.to_string();

    // --- Guard 1: source IP allowlist -------------------------------------------------------
    if !agent.cfg.is_allowed(peer) {
        agent.audit.write(&Entry {
            peer: &peer_text,
            method: "-",
            endpoint: "-",
            status: 403,
            command: None,
            exit_code: None,
            note: Some("source IP not in allowlist"),
        });
        // Best-effort: an unauthorised peer that also cannot receive the 403 changes nothing.
        let _ = http::write_error(&mut stream, 403, "source address not allowed");
        return;
    }

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[testd] could not clone socket for {peer_text}: {e}");
            return;
        }
    });

    let head = match http::read_head(&mut reader) {
        Ok(h) => h,
        Err(e) => {
            let status = e.status();
            agent.audit.write(&Entry {
                peer: &peer_text,
                method: "-",
                endpoint: "-",
                status,
                command: None,
                exit_code: None,
                note: Some(&e.to_string()),
            });
            let _ = http::write_error(&mut stream, status, &e.to_string());
            return;
        }
    };

    // --- Guard 2: bearer token (every endpoint except /health) -------------------------------
    if head.path != "/health" {
        let presented = token::bearer_from_headers(&head.headers);
        let ok = presented.map(|p| token::matches(&agent.token, p)).unwrap_or(false);
        if !ok {
            http::discard_body(&mut reader, head.content_length);
            agent.audit.write(&Entry {
                peer: &peer_text,
                method: &head.method,
                endpoint: &head.path,
                status: 401,
                command: None,
                exit_code: None,
                // Records that authentication failed — never what was presented.
                note: Some(if presented.is_some() {
                    "bearer token did not match"
                } else {
                    "no bearer token presented"
                }),
            });
            let _ = http::write_error(&mut stream, 401, "authentication required");
            return;
        }
    }

    let ctx = Ctx { agent, head: &head, peer: &peer_text };
    route(&ctx, &mut stream, &mut reader);
}

/// The three things every handler and every audit record needs. Bundling them keeps handler
/// signatures short and, more usefully, makes it impossible to log one request's peer against
/// another request's endpoint.
struct Ctx<'a> {
    agent: &'a Agent,
    head: &'a Head,
    peer: &'a str,
}

fn route(ctx: &Ctx<'_>, stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let method = ctx.head.method.as_str();
    let path = ctx.head.path.as_str();

    match (method, path) {
        ("GET", "/health") => handle_health(ctx, stream),
        ("POST", "/push") => handle_push(ctx, stream, reader),
        ("POST", "/run") => handle_run(ctx, stream, reader),
        ("GET", "/pull") => handle_pull(ctx, stream),
        ("POST", "/recover") => handle_recover(ctx, stream),
        ("GET", p) if p.starts_with("/job/") => handle_job(ctx, stream),
        // A known path with the wrong verb gets 405, an unknown path gets 404 — the difference
        // is what tells the operator "you used GET where POST was meant" at 2am.
        (_, "/health" | "/push" | "/run" | "/pull" | "/recover") => {
            respond(ctx, stream, 405, "method not allowed", None)
        }
        (_, p) if p.starts_with("/job/") => respond(ctx, stream, 405, "method not allowed", None),
        _ => respond(ctx, stream, 404, "no such endpoint", None),
    }
}

/// Audit and answer with a JSON error in one place, so no rejection path can forget to log.
fn respond(
    ctx: &Ctx<'_>,
    stream: &mut TcpStream,
    status: u16,
    message: &str,
    command: Option<&str>,
) {
    ctx.agent.audit.write(&Entry {
        peer: ctx.peer,
        method: &ctx.head.method,
        endpoint: &ctx.head.path,
        status,
        command,
        exit_code: None,
        note: Some(message),
    });
    if let Err(e) = http::write_error(stream, status, message) {
        eprintln!("[testd] response write failed for {}: {e}", ctx.peer);
    }
}

fn respond_json(
    ctx: &Ctx<'_>,
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    command: Option<&str>,
    note: Option<&str>,
) {
    ctx.agent.audit.write(&Entry {
        peer: ctx.peer,
        method: &ctx.head.method,
        endpoint: &ctx.head.path,
        status,
        command,
        exit_code: None,
        note,
    });
    if let Err(e) = http::write_json(stream, status, body) {
        eprintln!("[testd] response write failed for {}: {e}", ctx.peer);
    }
}

/// `GET /health` — the heartbeat. Resets the deadman and reports enough state for the controller
/// to verify, after a run, that the switch did not fire behind its back.
fn handle_health(ctx: &Ctx<'_>, stream: &mut TcpStream) {
    let agent = ctx.agent;
    // Read the age BEFORE touching, so the response reports the gap the controller actually
    // left rather than the zero it just created.
    let age = agent.deadman.since_last_health().as_secs();
    let armed = agent.deadman.is_armed();
    agent.deadman.touch();
    let body = format!(
        "{{\"ok\":true,\"version\":\"{}\",\"uptime_secs\":{},\"deadman\":{{\"armed\":{armed},\
          \"timeout_secs\":{},\"fires\":{},\"since_last_health_secs\":{age}}},\"sandbox\":\"{}\"}}",
        env!("CARGO_PKG_VERSION"),
        agent.started.elapsed().as_secs(),
        agent.deadman.timeout().as_secs(),
        agent.deadman.fire_count(),
        json_escape(&agent.canonical_root.display().to_string()),
    );
    // Heartbeats are the highest-volume request by far; auditing each one would bury the
    // records that matter. The deadman's own fire records are what prove liveness history.
    if let Err(e) = http::write_json(stream, 200, &body) {
        eprintln!("[testd] health response failed for {}: {e}", ctx.peer);
    }
}

/// `POST /push?path=<sandbox-relative>` — upload a file, streamed straight to disk.
fn handle_push(ctx: &Ctx<'_>, stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let head = ctx.head;
    let rel = match head.query_get("path") {
        Some(p) => p.to_string(),
        None => {
            http::discard_body(reader, head.content_length);
            return respond(ctx, stream, 400, "missing ?path=", None);
        }
    };
    if head.content_length == 0 {
        return respond(ctx, stream, 411, "Content-Length required", Some(&rel));
    }

    let target = match sandbox::resolve_for_create(&ctx.agent.canonical_root, &rel) {
        Ok(t) => t,
        Err(e) => {
            http::discard_body(reader, head.content_length);
            let msg = e.to_string();
            return respond(ctx, stream, 400, &msg, Some(&rel));
        }
    };

    match http::stream_body_to_file(
        reader,
        head.content_length,
        ctx.agent.cfg.max_upload_bytes,
        &target,
    ) {
        Ok((bytes, digest)) => {
            let body = format!(
                "{{\"ok\":true,\"path\":\"{}\",\"bytes\":{bytes},\"sha256\":\"{digest}\"}}",
                json_escape(&rel)
            );
            respond_json(ctx, stream, 201, &body, Some(&rel), Some("upload stored"));
        }
        Err(e) => {
            let status = e.status();
            let msg = e.to_string();
            // A partial file is worse than none: the controller would checksum-mismatch later,
            // far from the cause. Remove it, and say so if even that fails.
            if let Err(rm) = std::fs::remove_file(&target) {
                eprintln!("[testd] could not remove partial upload {}: {rm}", target.display());
            }
            respond(ctx, stream, status, &msg, Some(&rel));
        }
    }
}

/// `POST /run` — start a command, answer with its job id immediately.
fn handle_run(ctx: &Ctx<'_>, stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    let body = match http::read_body_to_vec(reader, ctx.head.content_length, MAX_JSON_BODY) {
        Ok(b) => b,
        Err(e) => {
            let status = e.status();
            let msg = e.to_string();
            return respond(ctx, stream, status, &msg, None);
        }
    };
    let spec: JobSpec = match serde_json::from_slice(&body) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("invalid job spec: {e}");
            return respond(ctx, stream, 400, &msg, None);
        }
    };

    match jobs::start(&spec, &ctx.agent.canonical_root, ctx.agent.cfg.default_job_timeout_secs) {
        Ok(started) => {
            let body = format!(
                "{{\"ok\":true,\"job_id\":\"{}\",\"status_url\":\"/job/{}\"}}",
                started.id, started.id
            );
            // exit_code is deliberately absent here: the job has only just been spawned, and
            // recording a placeholder would make the audit log claim knowledge it lacks.
            respond_json(
                ctx,
                stream,
                202,
                &body,
                Some(&started.command),
                Some(&format!("job {} started", started.id)),
            );
        }
        Err(e) => {
            let status = e.status();
            let msg = e.to_string();
            respond(ctx, stream, status, &msg, None);
        }
    }
}

/// `GET /job/<id>[?tail=N]` — status straight off disk, optionally with the tail of the output.
fn handle_job(ctx: &Ctx<'_>, stream: &mut TcpStream) {
    let agent = ctx.agent;
    let id = ctx.head.path.trim_start_matches("/job/");
    if !jobs::is_valid_job_id(id) {
        return respond(ctx, stream, 400, "malformed job id", None);
    }
    let status = match jobs::read_status(&agent.canonical_root, id) {
        Some(s) => s,
        None => return respond(ctx, stream, 404, "no such job", None),
    };

    let tail_bytes: u64 = ctx
        .head
        .query_get("tail")
        .and_then(|t| t.parse::<u64>().ok())
        .map(|t| t.min(MAX_TAIL_BYTES))
        .unwrap_or(0);

    let body = if tail_bytes == 0 {
        status
    } else {
        let stdout = jobs::tail(&agent.canonical_root, id, "stdout", tail_bytes).unwrap_or_default();
        let stderr = jobs::tail(&agent.canonical_root, id, "stderr", tail_bytes).unwrap_or_default();
        // Splice the two tails into the status document rather than nesting it, so the shape a
        // client parses is the same with and without ?tail=.
        let trimmed = status.trim_end();
        match trimmed.strip_suffix('}') {
            Some(without_brace) => format!(
                "{without_brace},\"stdout_tail\":\"{}\",\"stderr_tail\":\"{}\"}}",
                json_escape(&stdout),
                json_escape(&stderr)
            ),
            // status.json was not the object it is always written as — hand it back untouched
            // rather than emitting something that only looks like valid JSON.
            None => trimmed.to_string(),
        }
    };

    agent.audit.write(&Entry {
        peer: ctx.peer,
        method: &ctx.head.method,
        endpoint: &ctx.head.path,
        status: 200,
        command: None,
        exit_code: exit_code_from_status(&body),
        note: Some("job status read"),
    });
    if let Err(e) = http::write_json(stream, 200, &body) {
        eprintln!("[testd] job response failed for {}: {e}", ctx.peer);
    }
}

/// Pull the exit code out of a status document so the audit log's `exit=` column is populated
/// from the measured value rather than from anything the caller said.
fn exit_code_from_status(status_json: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(status_json).ok()?;
    value.get("exit_code")?.as_i64().map(|c| c as i32)
}

/// `GET /pull?path=<sandbox-relative>` — download a file from the sandbox.
///
/// There is no code path here that can reach `state_dir`: resolution is rooted at the canonical
/// sandbox root, and the config validator refuses a `state_dir` inside it. The token and the
/// audit log therefore cannot be fetched over the network at all.
fn handle_pull(ctx: &Ctx<'_>, stream: &mut TcpStream) {
    let agent = ctx.agent;
    let rel = match ctx.head.query_get("path") {
        Some(p) => p.to_string(),
        None => return respond(ctx, stream, 400, "missing ?path=", None),
    };
    let resolved = match sandbox::resolve_existing(&agent.canonical_root, &rel) {
        Ok(p) => p,
        Err(e) => {
            // An i/o error here is "no such file"; anything else is the caller's path being
            // rejected on its merits, which is a 400 and worth distinguishing in the log.
            let status = if matches!(e, sandbox::PathError::Io(_)) { 404 } else { 400 };
            let msg = e.to_string();
            return respond(ctx, stream, status, &msg, Some(&rel));
        }
    };
    if !resolved.is_file() {
        return respond(ctx, stream, 404, "not a file", Some(&rel));
    }

    let note = match http::write_file(stream, &resolved) {
        Ok(sent) => format!("{sent} bytes sent"),
        Err(e) => {
            // The status line is already on the wire, so there is no clean way to change it —
            // record the truncation instead of pretending the transfer completed.
            eprintln!("[testd] pull of {} failed mid-body: {e}", resolved.display());
            format!("TRANSFER FAILED MID-BODY: {e}")
        }
    };
    agent.audit.write(&Entry {
        peer: ctx.peer,
        method: &ctx.head.method,
        endpoint: &ctx.head.path,
        status: 200,
        command: Some(&rel),
        exit_code: None,
        note: Some(&note),
    });
}

/// `POST /recover` — run the recovery routine on demand, same routine the deadman fires.
fn handle_recover(ctx: &Ctx<'_>, stream: &mut TcpStream) {
    let agent = ctx.agent;
    agent.audit.note(&format!("manual /recover requested by {}", ctx.peer));
    let report = recovery::run("manual");
    let saved = recovery::persist(&report, &agent.cfg.state_dir, &agent.canonical_root);
    agent.audit.note(&report.summary());

    let body = format!(
        "{{\"ok\":{},\"report\":{},\"report_file\":{}}}",
        report.all_ok(),
        report.to_json(),
        saved
            .and_then(|p| p.strip_prefix(&agent.canonical_root).ok().map(|r| r.display().to_string()))
            .map(|r| format!("\"{}\"", json_escape(&r)))
            .unwrap_or_else(|| "null".to_string()),
    );
    // 200 even when steps failed: the request itself succeeded, and the body says exactly what
    // did not. A 500 here would be indistinguishable from the agent being broken.
    respond_json(ctx, stream, 200, &body, Some("recovery routine"), Some(&report.summary()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exit_code_is_read_out_of_a_status_document() {
        assert_eq!(exit_code_from_status(r#"{"exit_code":7}"#), Some(7));
        assert_eq!(exit_code_from_status(r#"{"exit_code":null}"#), None);
        assert_eq!(exit_code_from_status(r#"{"state":"running"}"#), None);
        assert_eq!(exit_code_from_status("not json"), None);
        assert_eq!(exit_code_from_status(r#"{"exit_code":-1}"#), Some(-1));
    }

    #[test]
    fn the_connection_limit_is_a_real_bound() {
        const { assert!(MAX_CONNECTIONS > 0 && MAX_CONNECTIONS <= 64) };
        // A job spec is a few hundred bytes; the cap should not invite anything larger.
        const { assert!(MAX_JSON_BODY < 1024 * 1024) };
    }

    #[test]
    fn serve_errors_render_without_leaking_the_token() {
        let e = ServeError::RefusedUnspecifiedBind;
        assert!(e.to_string().contains("0.0.0.0"));
        let e = ServeError::Bind {
            addr: "192.168.1.50:8765".parse().expect("literal"),
            source: std::io::Error::new(std::io::ErrorKind::AddrInUse, "in use"),
        };
        assert!(e.to_string().contains("192.168.1.50:8765"));
    }
}
