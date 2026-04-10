use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

pub fn render_command(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        return command.to_string();
    }
    format!("{command} {}", args.join(" "))
}

pub fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>() + "..."
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        return format!("{hours}h {minutes}m {seconds}s");
    }
    if minutes > 0 {
        return format!("{minutes}m {seconds}s");
    }
    format!("{seconds}s")
}

pub fn strip_ansi(value: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    ANSI_RE
        .get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("valid ANSI regex"))
        .replace_all(value, "")
        .into_owned()
}

pub fn discord_message(title: &str, job_name: &str, lines: &[String]) -> String {
    let mut rendered = format!("Nae^2 says | **{}** | job: `{}`", title, job_name);
    if let Some((first, rest)) = lines.split_first() {
        rendered.push('\n');
        rendered.push_str(first);
        for line in rest {
            rendered.push('\n');
            rendered.push_str(line);
        }
    }
    rendered
}

pub fn inline_fields(fields: &[(&str, String)]) -> String {
    fields
        .iter()
        .map(|(label, value)| format!("{label}: `{value}`"))
        .collect::<Vec<_>>()
        .join(" | ")
}
