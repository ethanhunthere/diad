use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use clap::Parser;
use diad_agent::config::settings::DiadConfig;
use diad_agent::core::daemon::DaemonState;
use diad_agent::core::{daemon_alive, open_gui, wait_for_daemon, DAEMON_PORT};
use diad_agent::providers::ProviderFactory;
use diad_agent::tui::app::{App, AppMode};
use diad_agent::tui::event::{Event, EventLoop};
use diad_agent::tui::ui;
use std::sync::Arc;

/// DIAD AGENT — Intent-Driven Operating System Interface
///
/// One brain, two faces: the daemon owns all logic and serves the GUI
/// (default) and the TUI (`--tui`) over a loopback WebSocket.
#[derive(Parser, Debug)]
#[command(name = "diad-agent", version, about, long_about = None)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = None)]
    config: Option<String>,

    /// Run as a headless daemon only (no GUI, no TUI)
    #[arg(long)]
    daemon: bool,

    /// Run the ratatui TUI client (a thin WebSocket client of the daemon)
    #[arg(long)]
    tui: bool,

    /// Configure API keys interactively
    #[arg(long)]
    configure: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Provider to use by default
    #[arg(short, long)]
    provider: Option<String>,

    /// Daemon HTTP/WS port (loopback only)
    #[arg(long, default_value_t = DAEMON_PORT)]
    port: u16,

    /// Do not auto-open the GUI after the daemon is up
    #[arg(long)]
    no_gui: bool,

    /// Detect hardware, classify the perf tier, write /etc/diad/perf-tier
    /// and /etc/diad/hw-profile.json, print a summary, and exit.
    #[arg(long)]
    perf_tier: bool,

    /// Print `x y w h` snap rectangles (one per line) for N windows and exit.
    #[arg(long)]
    snap: Option<usize>,

    /// Output size `WxH` for --snap (defaults to the focused sway output).
    #[arg(long)]
    snap_size: Option<String>,
}

/// Shared daemon singletons (config, factory, state).
struct Brain {
    config: Arc<DiadConfig>,
    factory: Arc<ProviderFactory>,
    state: Arc<DaemonState>,
}

fn build_brain(cli: &Cli) -> anyhow::Result<Brain> {
    let config = Arc::new(DiadConfig::load(cli.config.as_deref()).unwrap_or_else(|e| {
        tracing::warn!("Could not load config: {e}. Using defaults.");
        DiadConfig::default()
    }));
    let vault = Arc::new(diad_agent::auth::vault::Vault::new()?);
    let factory = Arc::new(ProviderFactory::new(
        Arc::clone(&vault),
        Arc::clone(&config),
    ));
    let registry = Arc::new(diad_agent::tools::create_default_registry());
    let state = Arc::new(DaemonState::new(
        Arc::clone(&config),
        Arc::clone(&vault),
        Arc::clone(&factory),
        Arc::clone(&registry),
    ));

    if let Some(p) = &cli.provider {
        *state.provider.write().unwrap() = p.clone();
        *state.model.write().unwrap() = factory.default_model(p);
        state
            .connected
            .store(factory.has_key(p), std::sync::atomic::Ordering::SeqCst);
    }

    Ok(Brain {
        config,
        factory,
        state,
    })
}

/// Spawn the daemon in-process (if none is running) and wait for it.
async fn ensure_daemon(brain: &Brain, port: u16) -> anyhow::Result<()> {
    if daemon_alive().await {
        return Ok(());
    }
    let state = Arc::clone(&brain.state);
    tokio::spawn(async move {
        if let Err(e) = diad_agent::core::server::serve(state, port).await {
            tracing::error!("daemon serve error: {e}");
        }
    });
    if !wait_for_daemon(std::time::Duration::from_secs(5)).await {
        anyhow::bail!("DIAD AGENT daemon failed to start on port {port}");
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // ---- Initialize tracing ----
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(env_filter)
        .init();

    // ---- --perf-tier: detect + classify + persist (no runtime needed) ----
    if cli.perf_tier {
        return run_perf_tier();
    }

    // ---- --snap N: print integer-pixel snap geometry and exit ----
    if let Some(n) = cli.snap {
        return run_snap(n, cli.snap_size.as_deref());
    }

    // ---- Tier-aware tokio runtime (spec 1.6): pay only for the cores the
    // tier allows. `active_tier()` never fails — missing profile falls back
    // to on-the-spot detection (spec 1.9).
    let tier = diad_agent::core::perf::active_tier();
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let workers = tier.tokio_workers(cores);
    tracing::info!(
        "perf tier {:?} — tokio worker threads: {workers} (cores: {cores})",
        tier
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()?;
    runtime.block_on(run(cli))
}

/// `--perf-tier`: detect, classify, persist, summarize.
fn run_perf_tier() -> anyhow::Result<()> {
    use diad_agent::core::perf;

    let profile = perf::detect_profile();
    let written = perf::write_profiles(&profile).is_ok();

    println!("=== DIAD — Hardware Profile ===");
    println!("RAM:        {} MB", profile.ram_kb / 1024);
    println!(
        "CPU:        {} cores — {}",
        profile.cpu_cores, profile.cpu_model
    );
    println!("AVX2:       {}", profile.has_avx2);
    println!(
        "Max freq:   {}",
        profile
            .cpu_max_freq_khz
            .map(|k| format!("{} MHz", k / 1000))
            .unwrap_or_else(|| "n/a".into())
    );
    println!("GPU:        {:?}", profile.gpu_kind);
    println!("GPU desc:   {}", profile.gpu_desc);
    println!("Storage:    {:?}", profile.storage);
    println!("Thermal:    {}", profile.has_thermal);
    println!("Battery:    {}", profile.has_battery);
    println!("Tier:       {:?}", profile.tier);
    println!(
        "Workers:    {}",
        profile.tier.tokio_workers(profile.cpu_cores)
    );
    println!(
        "Profiles:   {}",
        if written {
            format!(
                "written to {} + {}",
                perf::PERF_TIER_PATH,
                perf::HW_PROFILE_PATH
            )
        } else {
            "NOT written (permission denied — run as root)".into()
        }
    );
    Ok(())
}

/// `--snap N [--snap-size WxH]`: print snap rectangles for the focused output.
fn run_snap(n: usize, size: Option<&str>) -> anyhow::Result<()> {
    use diad_agent::core::perf;

    let (w, h, portrait) = match size {
        Some(s) => {
            let (w, h) = s
                .split_once('x')
                .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
                .ok_or_else(|| anyhow::anyhow!("--snap-size must be WxH, e.g. 1920x1080"))?;
            (w, h, h > w)
        }
        None => focused_output_size()?,
    };

    // Portrait outputs snap into vertical stacks (spec 2.3): compute the
    // tiling with swapped dimensions, then swap the rects back.
    let rects = if portrait {
        perf::snap_rects(h, w, n)
            .into_iter()
            .map(|(x, y, rw, rh)| (y, x, rh, rw))
            .collect()
    } else {
        perf::snap_rects(w, h, n)
    };
    for (x, y, rw, rh) in rects {
        println!("{x} {y} {rw} {rh}");
    }
    Ok(())
}

/// Ask sway for the focused output's size (integer px) + orientation.
fn focused_output_size() -> anyhow::Result<(u32, u32, bool)> {
    let out = std::process::Command::new("swaymsg")
        .args(["-t", "get_outputs", "--raw"])
        .output()
        .map_err(|e| anyhow::anyhow!("swaymsg unavailable: {e}"))?;
    let outputs: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout)?;
    let focused = outputs
        .iter()
        .find(|o| o.get("focused").and_then(|f| f.as_bool()).unwrap_or(false))
        .ok_or_else(|| anyhow::anyhow!("no focused sway output"))?;
    let rect = &focused["rect"];
    let w = rect["width"].as_u64().unwrap_or(1920) as u32;
    let h = rect["height"].as_u64().unwrap_or(1080) as u32;
    Ok((w, h, h > w))
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    tracing::info!("DIAD AGENT v{} starting...", env!("CARGO_PKG_VERSION"));

    let brain = build_brain(&cli)?;

    // ---- --configure: interactive key setup ----
    if cli.configure {
        return run_configure_flow(&brain.factory).await;
    }

    // ---- --tui: ratatui thin client ----
    if cli.tui {
        ensure_daemon(&brain, cli.port).await?;
        return run_tui_client(&brain, cli.port).await;
    }

    // ---- --daemon: headless service ----
    if cli.daemon {
        tracing::info!("DIAD AGENT daemon mode (headless)");
        return diad_agent::core::server::serve(Arc::clone(&brain.state), cli.port).await;
    }

    // ---- Default: daemon + GUI ----
    ensure_daemon(&brain, cli.port).await?;
    if !cli.no_gui {
        open_gui();
    }
    tracing::info!(
        "DIAD AGENT resident — serving GUI at http://127.0.0.1:{}",
        cli.port
    );

    // Stay resident until killed (SIGTERM/SIGINT).
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

// ===========================================================================
// TUI client (thin WebSocket face of the daemon)
// ===========================================================================

async fn run_tui_client(brain: &Brain, _port: u16) -> anyhow::Result<()> {
    use diad_agent::tui::client::DaemonClient;

    let provider = brain.state.provider.read().unwrap().clone();
    let mut app = App::new(provider, (*brain.config).clone());
    app.model = brain.state.model.read().unwrap().clone();
    app.connected = brain
        .state
        .connected
        .load(std::sync::atomic::Ordering::SeqCst);

    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let event_loop = EventLoop::new();
    diad_agent::tools::external::set_event_sender(event_loop.sender());

    let client = match DaemonClient::connect(event_loop.sender()).await {
        Ok(c) => c,
        Err(e) => {
            crossterm::terminal::disable_raw_mode()?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture
            )?;
            anyhow::bail!("Cannot connect to DIAD AGENT daemon: {e}");
        }
    };
    let (client, _reader) = client.spawn_reader();
    let client = Arc::new(client);
    client.providers_list();

    let result = run_tui_event_loop(&mut terminal, &mut app, event_loop, client).await;

    // Cleanup
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;

    match result {
        Ok(()) => {
            tracing::info!("DIAD AGENT TUI exited normally.");
            Ok(())
        }
        Err(e) => {
            tracing::error!("DIAD AGENT TUI exited with error: {e}");
            Err(e)
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn run_tui_event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    mut event_loop: EventLoop,
    client: Arc<diad_agent::tui::client::ClientHandle>,
) -> anyhow::Result<()> {
    let tx = event_loop.sender();

    loop {
        // ---- Render ----
        terminal.draw(|frame| ui::render(frame, app))?;

        // ---- Handle events ----
        match event_loop.next().await {
            Ok(Event::Key(key_event)) => {
                use crossterm::event::KeyCode;

                let prev_streaming = app.streaming;

                // F9 → GUI (Ctrl+Shift+X in the GUI opens this TUI).
                if key_event.code == KeyCode::F(9) && app.mode == AppMode::Chat {
                    client.open_gui();
                    app.messages.push("[system] DIAD AGENT GUI opened.".into());
                    continue;
                }

                match key_event.code {
                    KeyCode::Char('c')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        break;
                    }
                    KeyCode::Char('d')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        break;
                    }
                    KeyCode::Esc => {
                        if app.mode == AppMode::SecurePrompt {
                            app.handle_key_event(key_event);
                        } else if !app.input_buffer.is_empty() {
                            app.input_buffer.clear();
                        } else if app.offline_flow.is_some() {
                            app.offline_flow = None;
                            app.messages.push("[local] wifi flow cancelled".into());
                        } else {
                            app.mode = AppMode::Chat;
                        }
                    }
                    _ => {
                        app.handle_key_event(key_event);
                    }
                }

                // ---- Provider picker: refresh entries (async via daemon) ----
                if app.picker_requested {
                    app.picker_requested = false;
                    client.providers_list();
                }

                // ---- Provider picker: Enter on a provider ----
                if let Some(name) = app.provider_selected.take() {
                    let needs_key = app
                        .provider_entries
                        .iter()
                        .find(|e| e.name == name)
                        .map(|e| e.needs_key && !e.has_key)
                        .unwrap_or(false);
                    if needs_key {
                        app.pending_connect_provider = Some(name);
                    } else {
                        client.provider_switch(name);
                    }
                }

                // ---- Provider needs a key → masked SecurePrompt → connect ----
                if let Some(name) = app.pending_connect_provider.take() {
                    let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                    let _ = tx.send(Event::SecurePromptRequest {
                        label: format!("Paste API key for {name} (hidden):"),
                        reply: reply_tx,
                    });
                    let cname = name.clone();
                    let client = Arc::clone(&client);
                    tokio::spawn(async move {
                        let key = reply_rx.recv().await.unwrap_or_default();
                        if key.is_empty() {
                            client.event(Event::AppendMessage(format!(
                                "[system] Key entry cancelled for {cname}."
                            )));
                            return;
                        }
                        client.provider_connect(cname, key);
                    });
                }

                // ---- Offline wifi flow: SSID typed in the input bar ----
                if let Some(ssid) = app.pending_wifi_ssid.take() {
                    // The daemon captures the SSID from the next chat.send.
                    client.chat_send(ssid);
                }

                // ---- /set command (THE LAW F-15 TUI parity) ----
                if let Some((key, value)) = app.pending_config_set.take() {
                    client.config_set(key, value);
                }

                // ---- A7 parity slash commands (trash/clipboard/night light) ----
                if let Some(action) = app.pending_action.take() {
                    use diad_agent::tui::app::DesktopAction;
                    match action {
                        DesktopAction::TrashList => client.trash_list(),
                        DesktopAction::TrashMove(p) => client.trash_move(p),
                        DesktopAction::TrashRestore(n) => client.trash_restore(n),
                        DesktopAction::TrashEmpty => client.trash_empty(),
                        DesktopAction::ClipboardList => client.clipboard_list(),
                        DesktopAction::ClipboardPick(i) => client.clipboard_pick(i),
                        DesktopAction::ClipboardClear => client.clipboard_clear(),
                        DesktopAction::NightLight(t) => {
                            client.config_set(
                                "night_light".into(),
                                serde_json::json!({ "enabled": true, "temperature": t }),
                            );
                        }
                        DesktopAction::NightLightOff => {
                            client.config_set(
                                "night_light".into(),
                                serde_json::json!({ "enabled": false }),
                            );
                        }
                        DesktopAction::Dnd(on) => {
                            client.config_set("dnd_enabled".into(), serde_json::json!(on));
                        }
                    }
                }

                // ---- User submitted a message while streaming just started ----
                if !prev_streaming && app.streaming {
                    let input = app.messages.last().cloned().unwrap_or_default();
                    let input = input.strip_prefix("[You] ").unwrap_or(&input).to_string();
                    client.chat_send(input);
                }

                // ---- Connect mode: user entered an API key ----
                if app.api_key.is_some() && !app.connected {
                    let key = app.api_key.clone().unwrap_or_default();
                    let target = app.connect_target.clone();
                    client.provider_connect(target, key);
                    app.api_key = None;
                }
            }
            Ok(Event::Tick) => {}
            Ok(Event::StreamChunk(text)) => {
                if let Some(last) = app.messages.last_mut() {
                    if last.starts_with("[Assistant]") {
                        last.push_str(&text);
                    } else {
                        app.messages.push(format!("[Assistant] {text}"));
                    }
                } else {
                    app.messages.push(format!("[Assistant] {text}"));
                }
            }
            Ok(Event::StreamDone) => {
                app.streaming = false;
            }
            Ok(Event::ProviderResponse(response)) => {
                app.messages.push(format!("[Assistant] {response}"));
                app.streaming = false;
            }
            Ok(Event::ToolResult(result)) => {
                app.messages.push(format!("[Tool Result] {result}"));
            }
            Ok(Event::AppendMessage(line)) => {
                app.messages.push(line);
            }
            Ok(Event::SecurePromptRequest { label, reply }) => {
                app.request_secret(label, reply);
            }
            Ok(Event::ConfirmationRequest { label, reply }) => {
                app.request_confirmation(label, reply);
            }
            Ok(Event::OfflineWifiListed(output)) => {
                for line in output.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        app.messages.push(format!("[local] {line}"));
                    }
                }
                app.offline_flow = Some(diad_agent::tui::app::OfflineFlow::WifiSelect);
                app.messages.push(
                    "[system] Type the SSID you want to connect to, then press Enter.".into(),
                );
                app.streaming = false;
            }
            Ok(Event::HistoryUpdated(_)) => {
                // The daemon owns the structured history.
            }
            Ok(Event::ProviderConnected(name)) => {
                app.provider = name.clone();
                app.connected = true;
                app.messages.push(format!(
                    "[system] Provider switched to {name}. Pick a model with ↑/↓ and press Enter."
                ));
                client.models_list(name);
            }
            Ok(Event::StatusUpdate {
                provider,
                model,
                connected,
            }) => {
                let provider_changed = app.provider != provider;
                app.provider = provider;
                app.model = model;
                app.connected = connected;
                if provider_changed {
                    app.messages.push(format!(
                        "[system] Provider: {} · connected: {}",
                        app.provider, app.connected
                    ));
                }
            }
            Ok(Event::ModelsList {
                provider,
                models,
                current,
            }) => {
                app.model_list = models;
                app.model_index = app
                    .model_list
                    .iter()
                    .position(|m| *m == current)
                    .unwrap_or(0);
                app.messages.push(format!(
                    "[system] {provider} models — pick one with ↑/↓ and press Enter."
                ));
                app.mode = AppMode::ModelPicker;
            }
            Ok(Event::ProvidersList(entries)) => {
                app.provider_entries = entries;
                app.picker_index = app
                    .provider_entries
                    .iter()
                    .position(|e| e.name == app.provider)
                    .unwrap_or(0);
            }
            Ok(Event::ToolCallStart { .. }) => {
                // TUI parity: tool status lines arrive as AppendMessage.
            }
            Ok(Event::ToolCallResult { .. }) => {}
            Ok(Event::EditDiffRequest { .. }) => {
                // Mapped by the client reader onto ConfirmationRequest.
            }
            Ok(Event::Error(error)) => {
                app.messages.push(format!("[Error] {error}"));
                app.streaming = false;
            }
            Err(e) => {
                tracing::error!("Event loop error: {e}");
                break;
            }
        }
    }

    Ok(())
}

// ===========================================================================
// --configure: interactive API key setup
// ===========================================================================

async fn run_configure_flow(factory: &ProviderFactory) -> anyhow::Result<()> {
    use diad_agent::auth::vault::Vault;

    let vault = Vault::new()?;

    println!("=== DIAD AGENT — Provider Configuration ===\n");

    for provider_name in factory.all_provider_names() {
        if !factory.needs_key(&provider_name) {
            println!("• {} needs no API key (local endpoint).", provider_name);
            continue;
        }

        println!(
            "Do you want to configure the '{}' provider? [y/N] ",
            provider_name
        );

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input == "y" || input == "yes" {
            println!("Enter API key for {}: ", provider_name);
            let mut api_key = String::new();
            std::io::stdin().read_line(&mut api_key)?;
            let api_key = api_key.trim().to_string();

            if !api_key.is_empty() {
                vault.store_token(&provider_name, &api_key)?;
                println!("✓ {} configured successfully.\n", provider_name);
            } else {
                println!("✗ Skipped — empty key.\n");
            }
        } else {
            println!("✗ Skipped.\n");
        }
    }

    println!("Configuration complete! Tokens stored securely via libsecret/keyring.");
    println!("Run 'diad-agent' to start.");
    Ok(())
}
