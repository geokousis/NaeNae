# Examples

These examples are intentionally small and local. They show the config shape and
common rule patterns, but `naenae` is usually most useful for wrapping external
commands from other repos or tools you already run.

## Slow Counter

Print `1`, `2`, `3`, `4` with a 5 second pause each time:

```bash
cp examples/slow_counter.toml /tmp/slow_counter.toml
# edit /tmp/slow_counter.toml and add your Discord webhook
naenae run --config /tmp/slow_counter.toml
```

The example config sets `[run].cwd = "."`, so `naenae` resolves the script path
relative to the config file location.

Files:

- `examples/slow_counter.sh`
- `examples/slow_counter.toml`

## Fake Best Progress

Simulate progress output like `Best=1`, `Best=2`, `Best=3`, `Best=4`:

```bash
cp examples/best_counter.toml /tmp/best_counter.toml
# edit /tmp/best_counter.toml and add your Discord webhook
naenae run --config /tmp/best_counter.toml
```

Files:

- `examples/best_counter.sh`
- `examples/best_counter.toml`

## Fake Errors

Simulate warnings and errors:

```bash
cp examples/fake_errors.toml /tmp/fake_errors.toml
# edit /tmp/fake_errors.toml and add your Discord webhook
naenae run --config /tmp/fake_errors.toml
```

Files:

- `examples/fake_errors.sh`
- `examples/fake_errors.toml`

## Mixed Progress Demo

Simulate `Best=` and `gain` on the same line:

```bash
cp examples/mixed_progress.toml /tmp/mixed_progress.toml
# edit /tmp/mixed_progress.toml and add your Discord webhook
naenae run --config /tmp/mixed_progress.toml
```

Files:

- `examples/mixed_progress.sh`
- `examples/mixed_progress.toml`

If multiple rules match the same line, `naenae` sends one combined notification
instead of one notification per rule.
