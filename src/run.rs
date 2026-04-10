use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;

use crate::config::{Config, MonitorConfig, RunConfig};
use crate::formatting::{discord_message, format_duration, render_command, strip_ansi, truncate};
use crate::notifier::Notifier;
use crate::procfs::{
    AttachSource, discover_attach_sources, ensure_pid_owned_by_current_user, process_exists,
    process_label, sources_were_unavailable,
};
use crate::rules::{SharedState, StreamKind, compile_rules, process_line, read_stream};

pub async fn run_command_mode(
    config: Config,
    no_pty: bool,
    quiet: bool,
    cli_command: Option<String>,
    cli_cwd: Option<PathBuf>,
) -> Result<()> {
    let run = config
        .run
        .as_ref()
        .ok_or_else(|| anyhow!("missing [run] section in config"))?;
    let mut run = run.clone();

    if let Some(command) = cli_command {
        let parsed = shell_words::split(&command)
            .with_context(|| "failed to parse --command; check quoting")?;
        let (program, args) = parsed
            .split_first()
            .ok_or_else(|| anyhow!("--command must not be empty"))?;
        run.command = program.clone();
        run.args = args.to_vec();
    }
    if let Some(cwd) = cli_cwd {
        run.cwd = Some(cwd);
    }

    let use_pty = if no_pty {
        false
    } else {
        run.pty.unwrap_or(true)
    };
    let state = Arc::new(Mutex::new(SharedState::default()));
    let notifier = Arc::new(Notifier::new(
        config.discord.webhook_url.clone(),
        config.discord.bot_name.clone(),
    ));

    if config.monitor.notify_on_start.unwrap_or(true) {
        notifier
            .send(&discord_message(
                "Started",
                &config.monitor.name,
                &[format!(
                    "command: `{}`",
                    render_command(&run.command, &run.args)
                )],
            ))
            .await?;
    }

    if use_pty {
        return run_command_mode_pty(config, &run, notifier, state, quiet).await;
    }

    let mut command = Command::new(&run.command);
    command.args(&run.args);
    if let Some(cwd) = &run.cwd {
        command.current_dir(cwd);
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let started_at = std::time::Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{}`", run.command))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing stderr pipe"))?;

    let compiled_rules = compile_rules(&config.rules)?;
    let stdout_task = tokio::spawn(read_stream(
        stdout,
        StreamKind::Stdout,
        config.monitor.name.clone(),
        compiled_rules.clone(),
        notifier.clone(),
        state.clone(),
        quiet,
    ));
    let stderr_task = tokio::spawn(read_stream(
        stderr,
        StreamKind::Stderr,
        config.monitor.name.clone(),
        compiled_rules,
        notifier.clone(),
        state.clone(),
        quiet,
    ));

    let exit_status = child
        .wait()
        .await
        .context("failed while waiting for child")?;

    stdout_task.await??;
    stderr_task.await??;

    let elapsed = started_at.elapsed();
    let exit_summary = ExitSummary::from_std(exit_status);
    let command_label = render_command(&run.command, &run.args);
    finish_run_notification(&config.monitor, notifier, state, elapsed, exit_summary)
        .await
        .with_context(|| {
            format!(
                "Nae^2 says: [{}] command `{}` failed",
                config.monitor.name, command_label
            )
        })
}

async fn run_command_mode_pty(
    config: Config,
    run: &RunConfig,
    notifier: Arc<Notifier>,
    state: Arc<Mutex<SharedState>>,
    quiet: bool,
) -> Result<()> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open PTY")?;

    let mut cmd = CommandBuilder::new(&run.command);
    for arg in &run.args {
        cmd.arg(arg);
    }
    if let Some(cwd) = &run.cwd {
        cmd.cwd(cwd);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .with_context(|| format!("failed to spawn `{}` in PTY", run.command))?;
    drop(pair.slave);

    let child_pid = child
        .process_id()
        .map(|pid| pid as i32)
        .ok_or_else(|| anyhow!("PTY child pid unavailable"))?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("failed to clone PTY reader")?;

    let compiled_rules = compile_rules(&config.rules)?;
    let (tx, mut rx) = mpsc::channel::<String>(256);
    let read_task =
        tokio::task::spawn_blocking(move || read_pty_output_blocking(reader, tx, quiet));
    let ctrlc_task = tokio::spawn(forward_sigint(child_pid));
    let started_at = std::time::Instant::now();

    while let Some(line) = rx.recv().await {
        process_line(
            &line,
            StreamKind::Combined,
            &config.monitor.name,
            &compiled_rules,
            &notifier,
            &state,
        )
        .await?;
    }

    let wait_task = tokio::task::spawn_blocking(move || child.wait());
    let exit_status = wait_task
        .await
        .context("failed to join PTY wait task")?
        .context("failed while waiting for PTY child")?;
    let exit_summary = ExitSummary::from_portable(exit_status);

    ctrlc_task.abort();
    read_task
        .await
        .context("failed to join PTY read task")?
        .context("failed while reading PTY output")?;

    finish_run_notification(
        &config.monitor,
        notifier,
        state,
        started_at.elapsed(),
        exit_summary,
    )
    .await
    .with_context(|| {
        format!(
            "Nae^2 says: [{}] command `{}` failed",
            config.monitor.name,
            render_command(&run.command, &run.args)
        )
    })
}

pub async fn attach_mode(
    config: Config,
    pid: i32,
    log_file_override: Option<PathBuf>,
) -> Result<()> {
    ensure_pid_owned_by_current_user(pid).await?;

    let compiled_rules = compile_rules(&config.rules)?;
    let state = Arc::new(Mutex::new(SharedState::default()));
    let notifier = Arc::new(Notifier::new(
        config.discord.webhook_url.clone(),
        config.discord.bot_name.clone(),
    ));
    let process_label = process_label(pid).await.unwrap_or_else(|_| pid.to_string());
    let start_at_end = config
        .attach
        .as_ref()
        .and_then(|attach| attach.start_at_end)
        .unwrap_or(true);

    let log_file = log_file_override.or_else(|| {
        config
            .attach
            .as_ref()
            .and_then(|attach| attach.log_file.clone())
    });
    let sources = discover_attach_sources(pid, log_file).await?;

    if config.monitor.notify_on_start.unwrap_or(true) {
        let details = if sources.is_empty() {
            "regex tracking unavailable: no readable stdout/stderr file source detected".to_string()
        } else {
            let rendered = sources
                .iter()
                .map(|source| format!("{}={}", source.stream.as_str(), source.path.display()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("tracking output from {rendered}")
        };
        notifier
            .send(&discord_message(
                "Attached",
                &config.monitor.name,
                &[
                    format!("pid: `{pid}`"),
                    format!("process: `{}`", truncate(&process_label, 120)),
                    details,
                ],
            ))
            .await?;
    }

    let mut tasks = Vec::new();
    for source in sources {
        tasks.push(tokio::spawn(tail_file_source(
            source,
            config.monitor.name.clone(),
            compiled_rules.clone(),
            notifier.clone(),
            state.clone(),
            start_at_end,
        )));
    }

    let started_at = std::time::Instant::now();
    while process_exists(pid).await {
        sleep(Duration::from_secs(1)).await;
    }

    for task in tasks {
        task.await??;
    }

    let elapsed = started_at.elapsed();
    let state_guard = state.lock().await;
    let last_stdout = state_guard.last_stdout.clone();
    let last_stderr = state_guard.last_stderr.clone();
    drop(state_guard);

    if config.monitor.notify_on_finish.unwrap_or(true) {
        let mut message = discord_message(
            "Finished",
            &config.monitor.name,
            &[
                format!("pid: `{pid}`"),
                format!("duration: `{}`", format_duration(elapsed)),
            ],
        );
        if sources_were_unavailable(pid).await.is_err()
            && config
                .monitor
                .include_last_output_in_fail_message
                .unwrap_or(true)
        {
            if let Some(line) = last_stderr.or(last_stdout) {
                message.push_str(&format!("\nlast output: `{}`", truncate(&line, 300)));
            }
        }
        notifier.send(&message).await?;
    }

    Ok(())
}

async fn tail_file_source(
    source: AttachSource,
    job_name: String,
    rules: Vec<crate::rules::CompiledRule>,
    notifier: Arc<Notifier>,
    state: Arc<Mutex<SharedState>>,
    start_at_end: bool,
) -> Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&source.path)
        .await
        .with_context(|| format!("failed to open {}", source.path.display()))?;

    let mut offset = if start_at_end {
        file.metadata().await?.len()
    } else {
        0
    };
    let mut carry = String::new();

    loop {
        let current_len = match file.metadata().await {
            Ok(metadata) => metadata.len(),
            Err(_) => break,
        };

        if current_len < offset {
            offset = 0;
            carry.clear();
        }

        if current_len > offset {
            file.seek(std::io::SeekFrom::Start(offset)).await?;
            let bytes_to_read = (current_len - offset) as usize;
            let mut buf = vec![0_u8; bytes_to_read];
            file.read_exact(&mut buf).await?;
            offset = current_len;

            let chunk = String::from_utf8_lossy(&buf);
            carry.push_str(&chunk);
            while let Some(newline_idx) = carry.find('\n') {
                let line = carry[..newline_idx].trim_end_matches('\r').to_string();
                process_line(&line, source.stream, &job_name, &rules, &notifier, &state).await?;
                carry.drain(..=newline_idx);
            }
        }

        if !process_exists(source.pid).await {
            if !carry.is_empty() {
                let line = carry.trim_end_matches('\r').to_string();
                if !line.is_empty() {
                    process_line(&line, source.stream, &job_name, &rules, &notifier, &state)
                        .await?;
                }
            }
            break;
        }

        sleep(Duration::from_millis(750)).await;
    }

    Ok(())
}

async fn finish_run_notification(
    monitor: &MonitorConfig,
    notifier: Arc<Notifier>,
    state: Arc<Mutex<SharedState>>,
    elapsed: Duration,
    exit_status: ExitSummary,
) -> Result<()> {
    let state = state.lock().await;
    let last_stdout = state.last_stdout.clone();
    let last_stderr = state.last_stderr.clone();
    drop(state);

    if exit_status.success {
        if monitor.notify_on_finish.unwrap_or(true) {
            notifier
                .send(&discord_message(
                    "Finished",
                    &monitor.name,
                    &[format!("duration: `{}`", format_duration(elapsed))],
                ))
                .await?;
        }
        return Ok(());
    }

    if monitor.notify_on_fail.unwrap_or(true) {
        let code = exit_status
            .code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        let mut message = discord_message(
            "Failed",
            &monitor.name,
            &[
                format!("duration: `{}`", format_duration(elapsed)),
                format!("exit: `{}`", code),
            ],
        );
        if monitor.include_last_output_in_fail_message.unwrap_or(true) {
            if let Some(line) = last_stderr.or(last_stdout) {
                message.push_str(&format!("\nlast output: `{}`", truncate(&line, 300)));
            }
        }
        notifier.send(&message).await?;
    }

    Err(anyhow!(
        "Nae^2 says: [{}] job exited unsuccessfully",
        monitor.name
    ))
}

#[derive(Clone, Copy)]
struct ExitSummary {
    success: bool,
    code: Option<i32>,
}

impl ExitSummary {
    fn from_std(status: std::process::ExitStatus) -> Self {
        Self {
            success: status.success(),
            code: status.code(),
        }
    }

    fn from_portable(status: portable_pty::ExitStatus) -> Self {
        Self {
            success: status.success(),
            code: Some(status.exit_code() as i32),
        }
    }
}

async fn forward_sigint(pid: i32) -> Result<()> {
    loop {
        tokio::signal::ctrl_c()
            .await
            .context("failed to listen for ctrl-c")?;
        // Forward SIGINT directly so multi-press terminal semantics remain intact.
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }
}

fn read_pty_output_blocking(
    mut reader: Box<dyn Read + Send>,
    tx: mpsc::Sender<String>,
    quiet: bool,
) -> Result<()> {
    let mut buf = [0_u8; 4096];
    let mut carry = String::new();
    let mut stdout = std::io::stdout();

    loop {
        let read = reader.read(&mut buf).context("failed to read PTY output")?;
        if read == 0 {
            break;
        }

        if !quiet {
            stdout
                .write_all(&buf[..read])
                .context("failed to write PTY output to terminal")?;
            stdout.flush().context("failed to flush PTY output")?;
        }

        let chunk = strip_ansi(&String::from_utf8_lossy(&buf[..read]));
        carry.push_str(&chunk.replace('\r', "\n"));

        while let Some(newline_idx) = carry.find('\n') {
            let line = carry[..newline_idx].trim().to_string();
            if !line.is_empty() {
                tx.blocking_send(line)
                    .context("failed to forward PTY line")?;
            }
            carry.drain(..=newline_idx);
        }
    }

    let line = carry.trim();
    if !line.is_empty() {
        tx.blocking_send(line.to_string())
            .context("failed to forward PTY tail line")?;
    }

    Ok(())
}
