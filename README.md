# minit-rs
PID1 init system for embedded systems in rust.

## Run

```bash
cargo run -- examples/minit.json
```

Config format (JSON):

```json
{
  "services": [
    {
      "name": "demo",
      "command": ["/bin/sh", "-c", "sleep 5"],
      "restart": "on-failure"
    }
  ]
}
```

`restart` supports: `always`, `on-failure`, `never`.

Optional per-service respawn controls:

- `respawn_delay_secs`: base restart delay in seconds, default `1`
- `respawn_max`: maximum quick restarts allowed inside the respawn window, default `0` for unlimited
- `respawn_window_secs`: rolling window used to reset restart counters, default `60`

Example:

```json
{
  "services": [
    {
      "name": "demo",
      "command": ["/bin/sh", "-c", "sleep 5"],
      "restart": "on-failure",
      "respawn_delay_secs": 2,
      "respawn_max": 5,
      "respawn_window_secs": 30
    }
  ]
}
```

Send `SIGTERM` or `SIGINT` to `minit-rs` for graceful shutdown; it stops scheduling restarts, sends `SIGTERM` in reverse startup order, waits up to 30 seconds, then sends `SIGKILL` to any remaining children.
