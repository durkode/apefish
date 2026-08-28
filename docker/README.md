# apefish on Lichess

Package apefish with [lichess-bot](https://github.com/lichess-bot-devs/lichess-bot)
in one OCI container and challenge it from your main account.

## Layout

| File | Purpose |
| --- | --- |
| `Containerfile` | Single-stage image: `python:3.12-slim` + pinned lichess-bot + apefish binary |
| `build.sh` | Builds the static musl binary on the host, then builds the image |
| `redeploy.sh` | `build.sh` + restart the `apefish-bot` user service onto the new image |
| `apefish-uci` | Wrapper baked into the image; execs `apefish.bin --uci` |
| `config.yml` | lichess-bot config, bind-mounted at runtime (edit + restart, no rebuild) |
| `apefish-bot.container` | Podman Quadlet unit for an overnight-safe user service |
| `apefish.bin` | Build output, git-ignored, produced by `build.sh` |

## One-time setup

1. **Token** — you already have it in `~/.config/apefish-bot/lichess.env`:

   ```
   LICHESS_BOT_TOKEN=lip_xxxxxxxxxxxx
   ```

   Scope required: `bot:play`. `chmod 600` that file.

2. **Bot username** — put your *main* Lichess username in `config.yml` under
   `challenge.allow_list` so only you can challenge the engine. Delete that
   block to accept challenges from anyone.

3. **Build** (needs `rustup`, and `podman` or `docker`):

   ```sh
   docker/build.sh
   ```

   If the musl build fails on a linker error: `sudo apt install musl-tools`.

4. **Upgrade the bot account to BOT** (irreversible, account must have zero
   games played):

   ```sh
   podman run --rm \
     --env-file ~/.config/apefish-bot/lichess.env \
     -v ~/projects/apefish/docker/config.yml:/lichess-bot/config.yml:ro \
     apefish-bot:latest -u
   ```

## Run ad hoc

```sh
podman run -d --name apefish-bot \
  --env-file ~/.config/apefish-bot/lichess.env \
  -v ~/projects/apefish/docker/config.yml:/lichess-bot/config.yml:ro \
  apefish-bot:latest

podman logs -f apefish-bot          # watch it connect and wait for challenges
podman stop apefish-bot && podman rm apefish-bot
```

Then from your main account: open the bot's profile and click Challenge, or go
to `lichess.org/?user=<botname>#friend`. lichess-bot auto-accepts per
`config.yml`.

## Run overnight (Quadlet)

```sh
mkdir -p ~/.config/containers/systemd
ln -s ~/projects/apefish/docker/apefish-bot.container ~/.config/containers/systemd/
loginctl enable-linger "$USER"
systemctl --user daemon-reload
systemctl --user start apefish-bot
journalctl --user -u apefish-bot -f          # Ctrl-C to stop tailing
```

Expected within ~10s of start: `Welcome apefish!` and `You're now connected
to https://lichess.org/ and awaiting challenges.`

`enable-linger` keeps it running with no active login session; the
`[Install]` section in the unit makes it auto-start on the next login/boot
after `daemon-reload`.

The unit uses `Restart=always`, not `on-failure`, on purpose: lichess-bot
exits with status `0` even on a fatal startup error (bad token, network not
ready at boot), so `on-failure` would never bring it back. `StartLimitBurst=6`
/ `StartLimitIntervalSec=300` still make systemd give up if it fails 6 times
in 5 minutes, so a genuine misconfig doesn't loop forever. Within a running
session lichess-bot also reconnects through transient Lichess API and network
errors on its own.

### Manage the service

All commands are `--user` (rootless, no `sudo`). Unit name: `apefish-bot`.

```sh
# status / logs
systemctl --user status apefish-bot            # state + last few log lines
systemctl --user is-active apefish-bot         # just: active / inactive / failed
journalctl --user -u apefish-bot -f            # live logs, Ctrl-C to stop tailing
journalctl --user -u apefish-bot -n 100        # last 100 lines

# stop / start / restart
systemctl --user stop apefish-bot
systemctl --user start apefish-bot
systemctl --user restart apefish-bot           # after editing config.yml

# after editing apefish-bot.container itself
systemctl --user daemon-reload
systemctl --user restart apefish-bot

# after hitting the start limit (status shows start-limit-hit)
systemctl --user reset-failed apefish-bot
systemctl --user start apefish-bot
```

`stop` only lasts until the next reboot/login — the unit's
`[Install] WantedBy=default.target` starts it again then. To keep it off
across reboots, stop it, remove (or move aside) the
`~/.config/containers/systemd/apefish-bot.container` symlink, and
`systemctl --user daemon-reload`.

### Redeploy after a code change

```sh
docker/redeploy.sh
```

Rebuilds the engine and image, stops the service, `daemon-reload`s, and starts
it back on the new image, then prints the last log lines. Honours the same
`IMAGE` / `CONTAINER_ENGINE` env vars as `build.sh`. Requires the Quadlet unit
to be installed already.

### It's not coming online

```sh
systemctl --user status apefish-bot --no-pager
journalctl --user -u apefish-bot --no-pager -n 50
```

- `Active: inactive (dead)` with `status=0/SUCCESS` and a `RuntimeError` about
  the token in the log: lichess-bot hit a startup error and exited 0. With
  `Restart=always` it should recover; if it exhausted the start limit,
  `systemctl --user reset-failed apefish-bot && systemctl --user start apefish-bot`.
- `error in retrieving information about the bot's token`: check the token
  itself, not the container -
  `curl -H "Authorization: Bearer $TOKEN" https://lichess.org/api/token/test -d "$TOKEN"`
  should show scope `bot:play`. Confirm `~/.config/apefish-bot/lichess.env`
  has Unix (LF) line endings and no quotes around the value.
- After editing `apefish-bot.container`: `systemctl --user daemon-reload`
  then `systemctl --user restart apefish-bot`.

## Tuning

- **Flagging games**: raise `move_overhead` in `config.yml` (2000 -> 3000+),
  restart. Start with blitz/rapid before enabling bullet.
- **Time controls**: edit `challenge.time_controls`, restart.
- **Hash**: `engine.uci_options.Hash` (MiB), restart.
- **Pondering**: `engine.ponder: true` once you trust it, restart.

## Updating lichess-bot

lichess-bot's semver tags (`1.1.x`) are abandoned and years out of date; the
project ships from `master`. The image pins a `master` commit SHA in
`LICHESS_BOT_REF`. To upgrade: set it to the current
`github.com/lichess-bot-devs/lichess-bot` `master` HEAD, rerun `docker/build.sh`.
Current pin: `df7e730` (2026-08-09).
