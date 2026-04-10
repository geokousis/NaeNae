use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::config::RuleConfig;
use crate::formatting::{inline_fields, truncate};
use crate::notifier::Notifier;

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamSelector {
    Stdout,
    Stderr,
    Both,
}

#[derive(Clone, Copy, Debug)]
pub enum StreamKind {
    Stdout,
    Stderr,
    Combined,
}

impl StreamKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::Combined => "combined",
        }
    }
}

#[derive(Clone)]
pub struct CompiledRule {
    pub name: String,
    pub regexes: Vec<Regex>,
    pub stream: StreamSelector,
    pub notify_template: Option<String>,
    pub cooldown: Duration,
    pub max_notifications: Option<u32>,
}

#[derive(Default)]
pub struct RuleState {
    pub sent_count: u32,
    pub last_sent_at: Option<Instant>,
}

#[derive(Default)]
pub struct SharedState {
    pub last_stdout: Option<String>,
    pub last_stderr: Option<String>,
    pub rules: HashMap<String, RuleState>,
}

pub fn compile_rules(rules: &[RuleConfig]) -> Result<Vec<CompiledRule>> {
    let mut compiled = Vec::with_capacity(rules.len());
    for rule in rules {
        let mut patterns = Vec::new();
        if let Some(pattern) = &rule.pattern {
            patterns.push(pattern.clone());
        }
        patterns.extend(rule.patterns.clone());
        if patterns.is_empty() {
            return Err(anyhow!(
                "rule `{}` must define `pattern` or `patterns`",
                rule.name
            ));
        }

        let regexes = patterns
            .iter()
            .map(|pattern| {
                Regex::new(pattern)
                    .with_context(|| format!("invalid regex for rule `{}`", rule.name))
            })
            .collect::<Result<Vec<_>>>()?;

        compiled.push(CompiledRule {
            name: rule.name.clone(),
            regexes,
            stream: rule.stream,
            notify_template: rule.notify_template.clone(),
            cooldown: Duration::from_secs(rule.cooldown_secs.unwrap_or(0)),
            max_notifications: rule.max_notifications,
        });
    }
    Ok(compiled)
}

pub async fn read_stream<R>(
    reader: R,
    stream: StreamKind,
    job_name: String,
    rules: Vec<CompiledRule>,
    notifier: Arc<Notifier>,
    state: Arc<Mutex<SharedState>>,
    quiet: bool,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .context("failed to read process output")?
    {
        if !quiet {
            match stream {
                StreamKind::Stdout | StreamKind::Combined => println!("{line}"),
                StreamKind::Stderr => eprintln!("{line}"),
            }
        }
        process_line(&line, stream, &job_name, &rules, &notifier, &state).await?;
    }

    Ok(())
}

pub async fn process_line(
    line: &str,
    stream: StreamKind,
    job_name: &str,
    rules: &[CompiledRule],
    notifier: &Arc<Notifier>,
    state: &Arc<Mutex<SharedState>>,
) -> Result<()> {
    {
        let mut state = state.lock().await;
        match stream {
            StreamKind::Stdout | StreamKind::Combined => state.last_stdout = Some(line.to_string()),
            StreamKind::Stderr => state.last_stderr = Some(line.to_string()),
        }
    }

    // Coalesce multiple rule hits on the same output line into one Discord message.
    let mut triggered_rules: Vec<&CompiledRule> = Vec::new();
    {
        let mut state = state.lock().await;
        for rule in rules {
            let matched = rule.regexes.iter().any(|regex| regex.is_match(line));
            if !rule_matches_stream(rule.stream, stream) || !matched {
                continue;
            }

            let rule_state = state.rules.entry(rule.name.clone()).or_default();
            if let Some(max_notifications) = rule.max_notifications {
                if rule_state.sent_count >= max_notifications {
                    continue;
                }
            }
            if let Some(last_sent_at) = rule_state.last_sent_at {
                if last_sent_at.elapsed() < rule.cooldown {
                    continue;
                }
            }

            rule_state.sent_count += 1;
            rule_state.last_sent_at = Some(Instant::now());
            triggered_rules.push(rule);
        }
    }

    if !triggered_rules.is_empty() {
        let message = format_triggered_message(job_name, stream, line, &triggered_rules);
        notifier.send(&message).await?;
    }

    Ok(())
}

fn rule_matches_stream(selector: StreamSelector, stream: StreamKind) -> bool {
    matches!(selector, StreamSelector::Both)
        || matches!((stream, selector), (StreamKind::Combined, _))
        || matches!(
            (selector, stream),
            (StreamSelector::Stdout, StreamKind::Stdout)
        )
        || matches!(
            (selector, stream),
            (StreamSelector::Stderr, StreamKind::Stderr)
        )
}

fn format_rule_message(
    job_name: &str,
    stream: StreamKind,
    line: &str,
    rule: &CompiledRule,
) -> String {
    let default_message = discord_message(
        &format!("Rule Match: {}", rule.name),
        job_name,
        stream,
        line,
    );

    if let Some(template) = &rule.notify_template {
        return template
            .replace("{job}", job_name)
            .replace("{rule}", &rule.name)
            .replace("{stream}", stream.as_str())
            .replace("{line}", &truncate(line, 400));
    }

    default_message
}

fn format_triggered_message(
    job_name: &str,
    stream: StreamKind,
    line: &str,
    rules: &[&CompiledRule],
) -> String {
    if rules.len() == 1 {
        return format_rule_message(job_name, stream, line, rules[0]);
    }

    let rule_names = rules
        .iter()
        .map(|rule| rule.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    discord_message(
        &format!("Rule Match: {}", rule_names),
        job_name,
        stream,
        line,
    )
}

fn discord_message(title: &str, job_name: &str, stream: StreamKind, line: &str) -> String {
    format!(
        "Nae^2 says | **{}**\n{}\n`{}`",
        title,
        inline_fields(&[
            ("job", job_name.to_string()),
            ("stream", stream.as_str().to_string())
        ]),
        truncate(line, 400)
    )
}
