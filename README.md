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
