use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tokio::fs;

use crate::formatting::truncate;
use crate::rules::StreamKind;

#[derive(Clone)]
pub struct AttachSource {
    pub pid: i32,
    pub stream: StreamKind,
    pub path: PathBuf,
}

pub async fn list_processes() -> Result<()> {
    let uid = current_uid();
    let mut rows = Vec::new();

    let mut entries = fs::read_dir("/proc")
        .await
        .context("failed to read /proc for process listing")?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let pid: i32 = match name.parse() {
            Ok(pid) => pid,
            Err(_) => continue,
        };

        let status_path = entry.path().join("status");
        let status_contents = match fs::read_to_string(&status_path).await {
            Ok(contents) => contents,
            Err(_) => continue,
        };

        if extract_uid_from_status(&status_contents) != Some(uid) {
            continue;
        }

        let process_name =
            extract_name_from_status(&status_contents).unwrap_or_else(|| "<unknown>".to_string());
        let command = read_cmdline(pid)
            .await
            .unwrap_or_else(|_| "<no cmdline>".to_string());
        rows.push((pid, process_name, command));
    }

    rows.sort_by_key(|row| row.0);
    println!("{:<8} {:<24} COMMAND", "PID", "NAME");
    for (pid, name, command) in rows {
        println!(
            "{:<8} {:<24} {}",
            pid,
            truncate(&name, 24),
            truncate(&command, 120)
        );
    }

    Ok(())
}

pub async fn discover_attach_sources(
    pid: i32,
    log_file_override: Option<PathBuf>,
) -> Result<Vec<AttachSource>> {
    if let Some(path) = log_file_override {
        return Ok(vec![AttachSource {
            pid,
            stream: StreamKind::Combined,
            path,
        }]);
    }

    let mut seen = HashSet::new();
    let mut sources = Vec::new();

    for (fd, stream) in [(1, StreamKind::Stdout), (2, StreamKind::Stderr)] {
        let link_path = PathBuf::from(format!("/proc/{pid}/fd/{fd}"));
        let resolved = match fs::read_link(&link_path).await {
            Ok(path) => path,
            Err(_) => continue,
        };

        let real = normalize_proc_path(&resolved);
        let metadata = match fs::metadata(&real).await {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if !metadata.is_file() {
            continue;
        }

        let key = real.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }

        sources.push(AttachSource {
            pid,
            stream,
            path: real,
        });
    }

    Ok(sources)
}

fn normalize_proc_path(path: &Path) -> PathBuf {
    let rendered = path.to_string_lossy();
    if let Some(stripped) = rendered.strip_prefix("/proc/self/fd/") {
        return PathBuf::from(format!("/proc/{stripped}"));
    }
    path.to_path_buf()
}

pub async fn ensure_pid_owned_by_current_user(pid: i32) -> Result<()> {
    if !process_exists(pid).await {
        return Err(anyhow!("pid {} does not exist", pid));
    }

    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .await
        .with_context(|| format!("failed to read /proc/{pid}/status"))?;
    let owner = extract_uid_from_status(&status)
        .ok_or_else(|| anyhow!("failed to determine owner of pid {}", pid))?;

    if owner != current_uid() {
        return Err(anyhow!(
            "pid {} is not owned by the current user; refusing to attach",
            pid
        ));
    }

    Ok(())
}

pub async fn process_exists(pid: i32) -> bool {
    fs::try_exists(format!("/proc/{pid}"))
        .await
        .unwrap_or(false)
}

pub async fn process_label(pid: i32) -> Result<String> {
    let cmdline = read_cmdline(pid).await?;
    if cmdline != "<no cmdline>" {
        return Ok(cmdline);
    }
    let status = fs::read_to_string(format!("/proc/{pid}/status")).await?;
    Ok(extract_name_from_status(&status).unwrap_or_else(|| pid.to_string()))
}

pub async fn read_cmdline(pid: i32) -> Result<String> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).await?;
    if bytes.is_empty() {
        return Ok("<no cmdline>".to_string());
    }

    let parts = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Ok("<no cmdline>".to_string());
    }
    Ok(parts.join(" "))
}

fn extract_uid_from_status(status: &str) -> Option<u32> {
    status.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? != "Uid:" {
            return None;
        }
        parts.next()?.parse().ok()
    })
}

fn extract_name_from_status(status: &str) -> Option<String> {
    status.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key != "Name" {
            return None;
        }
        Some(value.trim().to_string())
    })
}

fn current_uid() -> u32 {
    // Safe because `geteuid` has no preconditions and returns the effective uid.
    unsafe { libc::geteuid() }
}

pub async fn sources_were_unavailable(pid: i32) -> Result<()> {
    let mut found = false;
    for fd in [1, 2] {
        let link_path = PathBuf::from(format!("/proc/{pid}/fd/{fd}"));
        if fs::read_link(link_path).await.is_ok() {
            found = true;
        }
    }
    if found {
        Ok(())
    } else {
        Err(anyhow!("no output sources"))
    }
}
