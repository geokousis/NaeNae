![LOGO_v2](logo/Nae.png)

*it watches... naenae*

`naenae` wraps a command, watches its live output, matches regex rules and sends Discord webhook notifications.

It is useful when you want:

- start / finish / fail notifications
- progress notifications from CLI tools
- live local output and Discord updates at the same time
- PTY behavior by default for terminal-sensitive tools

`naenae` is intentionally minimal. It does not try to understand every tool or auto-infer the right regex rules for every project.
That part can be assisted by an agent: give the agent the downloaded repository or the tool you want to monitor, and let it help you write a good `naenae` config for that specific tool.

## Install

### Requirements

- Rust toolchain
- a Discord webhook URL

### Install From Source

Clone the repo and install:

```bash
git clone <your-repo-url>
cd naenae
cargo install --path .
```

Build without installing:

```bash
cargo build
```

Run directly from the repo:

```bash
cargo run -- run --config naenae.toml
```

### Update

After local changes:

```bash
cargo install --path .
```

## Quick Start

Create a config:

```toml
[discord]
webhook_url = "https://discord.com/api/webhooks/your/webhook"
bot_name = "NaeNae"

[monitor]
name = "demo"

[run]
command = "bash"
args = ["-lc", "for i in 1 2 3 4; do echo \"$i\"; sleep 5; done"]

[[rules]]
name = "numbers"
pattern = "^[1-4]$"
cooldown_secs = 1
```

Then run:

```bash
naenae run --config naenae.toml
```

## Why Minimal

`naenae` focuses on a small core:

- run a command
- capture output
- match regex rules
- send notifications

That keeps the tool predictable and reusable across projects.

If you need help figuring out:

- which output lines matter
- which regex rules to use
- whether PTY is a good fit
- how to monitor a specific repo or CLI

an agent can help with that part.

A practical workflow is:

1. download or clone the repo for the tool you want to monitor
2. give that repo to an agent
3. ask the agent to inspect the tool's output patterns and write a `naenae` config
4. run `naenae` with that config

## Minimal Config

For a normal run, this is enough:

```toml
[discord]
webhook_url = "https://discord.com/api/webhooks/your/webhook"
bot_name = "NaeNae"

[monitor]
name = "my-job"

[run]
command = "python"
args = ["my_script.py"]

[[rules]]
name = "progress"
patterns = ["Best=\\d+", "gain\\s*\\+?\\d+"]
cooldown_secs = 1
```

Defaults:

- PTY is enabled by default
- start / finish / fail notifications are enabled by default
- `stream = "both"` is the default
- local output is shown by default

## CLI

Run using the config command:

```bash
naenae run --config naenae.toml
```

Override the config command with a single string:

```bash
naenae run --config naenae.toml --command "python my_script.py --epochs 10"
```

Disable PTY:

```bash
naenae run --config naenae.toml --no-pty
```

Hide local output:

```bash
naenae run --config naenae.toml --quiet
```

List your processes:

```bash
naenae ps
```

Attach to a running PID with a known log file:

```bash
naenae attach --config naenae.toml --pid 12345 --log-file /tmp/job.log
```

## Examples

Example scripts and configs live under [examples/](/media/storage/kousis/work_2/naenae/examples).

### 1. Slow Counter

Print `1`, `2`, `3`, `4` with a 5 second pause each time:

```bash
cp examples/slow_counter.toml /tmp/slow_counter.toml
# edit /tmp/slow_counter.toml and add your Discord webhook
naenae run --config /tmp/slow_counter.toml
```

Files:

- [examples/slow_counter.sh](/media/storage/kousis/work_2/naenae/examples/slow_counter.sh)
- [examples/slow_counter.toml](/media/storage/kousis/work_2/naenae/examples/slow_counter.toml)

### 2. Fake Best Progress

Simulate progress output like `Best=1`, `Best=2`, `Best=3`, `Best=4`:

```bash
cp examples/best_counter.toml /tmp/best_counter.toml
# edit /tmp/best_counter.toml and add your Discord webhook
naenae run --config /tmp/best_counter.toml
```

Files:

- [examples/best_counter.sh](/media/storage/kousis/work_2/naenae/examples/best_counter.sh)
- [examples/best_counter.toml](/media/storage/kousis/work_2/naenae/examples/best_counter.toml)

### 3. Fake Errors

Simulate warnings and errors:

```bash
cp examples/fake_errors.toml /tmp/fake_errors.toml
# edit /tmp/fake_errors.toml and add your Discord webhook
naenae run --config /tmp/fake_errors.toml
```

Files:

- [examples/fake_errors.sh](/media/storage/kousis/work_2/naenae/examples/fake_errors.sh)
- [examples/fake_errors.toml](/media/storage/kousis/work_2/naenae/examples/fake_errors.toml)

### 4. Mixed Progress Demo

Simulate `Best=` and `gain` on the same line:

```bash
cp examples/mixed_progress.toml /tmp/mixed_progress.toml
# edit /tmp/mixed_progress.toml and add your Discord webhook
naenae run --config /tmp/mixed_progress.toml
```

Files:

- [examples/mixed_progress.sh](/media/storage/kousis/work_2/naenae/examples/mixed_progress.sh)
- [examples/mixed_progress.toml](/media/storage/kousis/work_2/naenae/examples/mixed_progress.toml)

If multiple rules match the same line, `naenae` sends one combined notification instead of spamming one notification per rule.

## Streams

Rules can target:

- `stdout`
- `stderr`
- `both`

If you omit `stream`, the default is `both`.

In PTY mode, output is merged into a single terminal stream, so notifications will report `stream: combined`.

## Attach Mode

`attach` is for an already-running process.

In that mode, `naenae` usually needs a readable output source, such as a log file:

```toml
[attach]
log_file = "/tmp/job.log"
start_at_end = true
```

You do not need `log_file` for normal `run` mode.

## Notes

- `--command` accepts a full shell-style command string
- if the executable is already on `PATH`, use it directly
- if not, use something like `python test.py test ...`
- PTY mode is the default because many terminal test tools behave differently without it
