//! op-sled-top — top-like live view of the Identity Sled (The Sled)
//! 1:1 shared memory layout at /dev/shm/plugin_schema.dat
//! Per AGENTS.md: The Sled is the Absolute Base — no valid schema = no entity.

use anyhow::{Context, Result};
use blake3;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use op_identity::{read_sled, IdentitySled, SHM_SLED_PATH};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Application state
struct App {
    sled: Option<IdentitySled>,
    layout: String,             // "canonical" | "compact" | "missing"
    last_refresh: Instant,
    refresh_interval: Duration,
    schema_catalog_hash: String,
    trace_id: String,
}

impl App {
    fn new(refresh_ms: u64) -> Self {
        Self {
            sled: None,
            layout: "missing".into(),
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_millis(refresh_ms),
            schema_catalog_hash: String::new(),
            trace_id: String::new(),
        }
    }

    /// Read the sled from /dev/shm
    fn refresh(&mut self) -> Result<()> {
        // Try to read canonical sled first
        if let Ok((ptr, _mmap)) = read_sled() {
            // SAFETY: read_sled returns a valid pointer to IdentitySled in shared memory
            // The mmap guard keeps the mapping alive for this borrow
            let sled = unsafe { &*ptr };

            self.sled = Some(sled.clone());
            self.layout = "canonical".into();
            self.trace_id = sled.trace_id_hex();
            self.schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
                .map(|b| hex::encode(blake3::hash(&b).as_bytes()))
                .unwrap_or_else(|_| "(missing)".into());
        } else {
            self.sled = None;
            self.layout = "missing".into();
        }

        self.last_refresh = Instant::now();
        Ok(())
    }
}

/// Terminal UI rendering
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Header
            Constraint::Min(10),     // Sled content
            Constraint::Length(3),   // Footer
        ])
        .split(size);

    // ─── Header ──────────────────────────────────────────────
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("op-sled-top", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  │  "),
            Span::styled("The Sled (Identity State)", Style::default().fg(Color::White)),
            Span::raw("  │  "),
            Span::styled(format!("layout: {}", app.layout), Style::default().fg(Color::Yellow)),
            Span::raw("  │  "),
            Span::styled(
                format!("refresh: {:.1}s", app.refresh_interval.as_secs_f32()),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(SHM_SLED_PATH, Style::default().fg(Color::White)),
            Span::raw("  │  "),
            Span::styled("schema catalog: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.schema_catalog_hash, Style::default().fg(Color::White)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Identity Sled "))
    .alignment(Alignment::Left);
    f.render_widget(header, chunks[0]);

    // ─── Sled Content ────────────────────────────────────────
    let content = if let Some(sled) = &app.sled {
        let valid_style = if sled.is_sled_valid() {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };

        let items = vec![
            ListItem::new(Line::from(vec![
                Span::styled("WireGuard Pubkey: ", Style::default().fg(Color::DarkGray)),
                Span::styled(encode_b64(&sled.wireguard_pubkey), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Trace ID:        ", Style::default().fg(Color::DarkGray)),
                Span::styled(&app.trace_id, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Mutation Index:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(sled.mutation_index.to_string(), Style::default().fg(Color::Yellow)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Schema Version:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(sled.schema_version.to_string(), Style::default().fg(Color::Blue)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Vector ID (Qdrant): ", Style::default().fg(Color::DarkGray)),
                Span::styled(sled.vector_id_uuid(), Style::default().fg(Color::Cyan)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Hashed Footprint:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(hex(&sled.hashed_footprint), Style::default().fg(Color::Cyan)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Valid (Absolute Base): ", Style::default().fg(Color::DarkGray)),
                Span::styled(if sled.is_sled_valid() { "YES" } else { "NO" }, valid_style),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("Layout:            ", Style::default().fg(Color::DarkGray)),
                Span::styled(&app.layout, Style::default().fg(Color::Yellow)),
            ])),
        ];

        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Sled Data (updated {:.1}s ago) ",
                    app.last_refresh.elapsed().as_secs_f32()
                )),
        )
    } else {
        List::new(vec![ListItem::new(Line::from(
            Span::styled(
                "NO SLED FOUND — /dev/shm/plugin_schema.dat missing or unreadable",
                Style::default().fg(Color::Red),
            )
        ))]).block(Block::default().borders(Borders::ALL).title(" Sled Data "))
    };

    f.render_widget(content, chunks[1]);

    // ─── Footer ──────────────────────────────────────────────
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q/ESC ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::raw(" quit  │  "),
        Span::styled(" r ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::raw(" refresh now  │  "),
        Span::styled(" s ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::raw(" dump JSON"),
    ]))
    .block(Block::default().borders(Borders::ALL))
    .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}

/// Run the TUI loop
async fn run_tui(
    app: Arc<tokio::sync::Mutex<App>>,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));

    // Initial read
    app.lock().await.refresh()?;

    // Refresh loop
    let refresh_handle = {
        let app = app.clone();
        let running = running.clone();
        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(500)).await;
                let mut app_guard = app.lock().await;
                if app_guard.last_refresh.elapsed() >= app_guard.refresh_interval {
                    let _ = app_guard.refresh();
                }
            }
        })
    };

    // Event loop
    while running.load(Ordering::Relaxed) {
        let app_guard = app.lock().await;
        terminal.draw(|f| ui(f, &app_guard))?;
        drop(app_guard);

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                    KeyCode::Char('r') => {
                        app.lock().await.refresh()?;
                    }
                    KeyCode::Char('s') => {
                        let app_guard = app.lock().await;
                        if let Some(sled) = &app_guard.sled {
                            println!("{}", serde_json::to_string_pretty(sled)?);
                        }
                        drop(app_guard);
                    }
                    _ => {}
                }
            }
        }
    }

    running.store(false, Ordering::Relaxed);
    refresh_handle.abort();
    Ok(())
}

/// Encode WG pubkey as base64
fn encode_b64(bytes: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Format bytes as hex
fn hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Print sled once (non-TUI mode)
fn print_sled(app: &App, json: bool) -> Result<()> {
    if let Some(sled) = &app.sled {
        if json {
            println!("{}", serde_json::to_string_pretty(sled)?);
        } else {
            println!("WireGuard Pubkey: {}", encode_b64(&sled.wireguard_pubkey));
            println!("Trace ID:        {}", app.trace_id);
            println!("Mutation Index:  {}", sled.mutation_index);
            println!("Schema Version:  {}", sled.schema_version);
            println!("Vector ID:       {}", sled.vector_id_uuid());
            println!("Hashed Footprint:{}", hex(&sled.hashed_footprint));
            println!("Valid:           {}", if sled.is_sled_valid() { "YES" } else { "NO" });
            println!("Layout:          {}", app.layout);
            println!("Schema Catalog:  {}", app.schema_catalog_hash);
        }
    } else {
        eprintln!("No sled found at {}", SHM_SLED_PATH);
        std::process::exit(1);
    }
    Ok(())
}

fn print_help() {
    println!(
        "op-sled-top — live view of the Identity Sled (The Sled)\n\
\n\
Usage: op-sled-top [options]\n\n\
Options:\n\
  -r, --refresh MS    Refresh interval in milliseconds (default: 2000)\n\
  -o, --once          Print once and exit (no TUI)\n\
  -j, --json          Output JSON (implies --once)\n\
  -p, --path PATH     Sled file path (default: /dev/shm/plugin_schema.dat)\n\
  -h, --help          Show this help\n\n\
Keys (TUI mode):\n\
  q / ESC   Quit\n\
  r         Refresh now\n\
  s         Dump JSON to stdout\n\n\
Environment:\n\
  OP_IDENTITY_SLED_PATH  Override sled path\n"
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut refresh_ms = 2000;
    let mut once = false;
    let mut json = false;
    let mut path = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-r" | "--refresh" => {
                i += 1;
                if i < args.len() {
                    refresh_ms = args[i].parse().context("--refresh requires milliseconds")?;
                }
            }
            "-o" | "--once" => once = true,
            "-j" | "--json" => {
                json = true;
                once = true;
            }
            "-p" | "--path" => {
                i += 1;
                if i < args.len() {
                    path = Some(args[i].clone());
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if let Some(p) = path {
        std::env::set_var("OP_IDENTITY_SLED_PATH", p);
    }

    let mut app = App::new(refresh_ms);
    app.refresh()?;

    if once {
        print_sled(&app, json)?;
        return Ok(());
    }

    // TUI mode - wrap in Arc<Mutex> for shared state
    let app = Arc::new(tokio::sync::Mutex::new(app));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(app, &mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}
