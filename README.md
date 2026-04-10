<img src="logo/Nae.png" alt="LOGO_v2" width="360" />

*it watches... naenae*

`naenae` wraps a command, watches its live output, matches regex rules and sends Discord webhook notifications.

It is useful when you want:

- start / finish / fail notifications
- progress notifications from CLI tools
- live local output and Discord updates at the same time
- PTY behavior by default for terminal-sensitive tools

`naenae` is intentionally minimal. It does not try to understand every tool or auto-infer the right regex rules for every project.
That part can be assisted by an agent: give the agent the downloaded repository or the tool you want to monitor and let it help you write a `naenae` config for that specific tool.

## Install

### Requirements

- Rust toolchain
- a Discord webhook URL

### Install From crates.io

```bash
cargo install naenae
```

### Install From Source

Clone the repo and install:

```bash
git clone https://github.com/geokousis/NaeNae 
cd NaeNae
cargo install --path .
```

Build without installing:

```bash
cargo build
```


## Quick Start

Create a config (toml file):

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

A practical workflow if you are too bored to understand `naenae`:

0. clone/install NaeNae 
1. download or clone the repo for the tool you want to monitor or a live output
2. give both repos (our output of your tool of choice) to an agent
3. ask the agent to inspect the tool's output patterns and write a `naenae` toml config
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

Hide local tool output:

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

Detailed examples live in [examples/README.md](examples/README.md).

```bash
cp examples/slow_counter.toml /tmp/slow_counter.toml
# edit /tmp/slow_counter.toml and add your Discord webhook
naenae run --config /tmp/slow_counter.toml
```

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

`attach` is best-effort. It is useful for tailing output and regex matches from
an existing process, but it does not have a reliable exit-status source, so its
final notification means the process disappeared, not that it definitely exited
successfully.

You do not need `log_file` for normal `run` mode.

## Notes

- `--command` accepts a full shell-style command string
- if the executable is already on `PATH`, use it directly
- if not, use something like `python test.py test ...`
- PTY mode is the default because many terminal test tools behave differently without it
