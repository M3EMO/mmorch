# Capa host — estado y como prenderla

Estado (2026-10-26): **apagada**. Solo existe la laptop Fedora; la receta vieja de Windows (`schtasks`,
pc-mateo y auto-pull entre PCs) se borro con esa maquina. Nada agenda `mmorch-server`, `mmorch-sync`
ni `mmorch-nightly`. El MCP `mmorch` no depende de esta capa: Claude Code lo lanza solo.

Prenderla es una decision: el nightly gasta dolares de API y abre branches en `mmorch/auto`. Hasta
que una metrica lo pida, queda apagada (GOAL.md: "complejidad que ninguna metrica justifica").

## Prender el server (servicio de usuario systemd)

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/mmorch-server.service <<'EOF'
[Unit]
Description=mmorch server

[Service]
WorkingDirectory=%h/.claude/orchestration
ExecStart=%h/.claude/orchestration/.venv/bin/mmorch-server
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now mmorch-server
```

`server.main` lee `MMORCH_SERVER_*` del `.env`. Escucha solo en el host y puerto del `.env`; nunca `0.0.0.0`.

## Prender el nightly (timer de usuario systemd)

```bash
cat > ~/.config/systemd/user/mmorch-nightly.service <<'EOF'
[Unit]
Description=mmorch nightly

[Service]
Type=oneshot
WorkingDirectory=%h/.claude/orchestration
ExecStart=%h/.claude/orchestration/.venv/bin/mmorch-nightly
EOF
cat > ~/.config/systemd/user/mmorch-nightly.timer <<'EOF'
[Unit]
Description=mmorch nightly 03:00

[Timer]
OnCalendar=*-*-* 03:00
Persistent=true

[Install]
WantedBy=timers.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now mmorch-nightly.timer
```

Apagar: `systemctl --user disable --now mmorch-server mmorch-nightly.timer`.

## Seguridad
- `.env` y las claves nunca van por git (gitignored).
- `claude -p` en modo edit escribe: corre solo sobre repos con git (rollback).
