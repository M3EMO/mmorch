# Always-on host + GitHub sync — receta

Arquitectura: una PC always-on (ej **pc-mateo**) HOSTEA el server y hace el trabajo pesado
(`claude -p`); las otras hacen **auto-pull**. GitHub = bus de sync. Reglas: un escritor
(agente → branch `mmorch/auto`, vos mergeás), auto-pull solo si el árbol está limpio.

## A. En el HOST (pc-mateo) — una vez

1. **Deps**: Git, Python 3.12, Claude Code (`npm i -g @anthropic-ai/claude-code`), Tailscale (ya).
2. **Clonar + instalar** (packaging: una línea trae todas las deps del host):
   ```
   git clone https://github.com/M3EMO/mmorch.git %USERPROFILE%\.claude\orchestration
   cd %USERPROFILE%\.claude\orchestration
   python -m venv .venv
   .venv\Scripts\pip install -e .[host]
   ```
   (`[host]` = mcp+memory+checkers+server+factory. Da los scripts `mmorch-server` y `mmorch-sync`.)
3. **.env** (copialo a mano, NO va por git):
   ```
   DEEPSEEK_API_KEY=...
   GEMINI_API_KEY=...
   MMORCH_SERVER_TOKEN=bfP0brI-if387ExSyUD6-uZm
   MMORCH_SERVER_HOST=100.88.0.57      # IP tailnet de pc-mateo
   MMORCH_SERVER_PORT=8787
   ```
4. **Login del plan** (pa que `claude -p` use cupo): `claude login`
5. **Credenciales git** pa push (PAT o `gh auth login`).
6. **Clonar los proyectos** que vas a controlar + registrarlos:
   ```
   git clone https://github.com/<vos>/portfolio.git C:\work\portfolio
   .venv\Scripts\python.exe -c "import sys; sys.path.insert(0,'.'); from mmorch import register_project; register_project('portfolio', r'C:\work\portfolio')"
   ```
7. **Server always-on** (Scheduled Task al iniciar sesión):
   ```
   schtasks /create /tn "mmorch-server" /sc onlogon /rl highest /f ^
     /tr "cmd /c cd /d %USERPROFILE%\.claude\orchestration && .venv\Scripts\mmorch-server.exe"
   schtasks /run /tn "mmorch-server"
   ```
   (server.main lee MMORCH_SERVER_* del .env vía dotenv.)

## B. En las OTRAS PCs (esta) — auto-pull

Scheduled Task cada 5 min que trae los cambios del agente:
```
schtasks /create /tn "mmorch-pull" /sc minute /mo 5 /f ^
  /tr "cmd /c cd /d %USERPROFILE%\.claude\orchestration && .venv\Scripts\mmorch-sync.exe pull-all"
```
Seguro: solo pullea repos con árbol limpio, ff-only (no pisa tu WIP).

## C. Flujo de los proyectos de ESTA PC -> pc-mateo

Editar archivos es local al host. Para que pc-mateo trabaje tus proyectos de acá:
1. Cada proyecto = repo con remote GitHub (los que no, `git init` + crear repo + push).
2. pc-mateo los clona (paso A6).
3. El agente edita en pc-mateo (mode=edit, push=true) → pushea a `mmorch/auto`.
4. Revisás/mergeás `mmorch/auto` → main en GitHub.
5. Esta PC: `mmorch-pull` trae main al día.

## Seguridad
- Solo alcanzable dentro del tailnet + token. Nunca `0.0.0.0` público.
- `.env`/keys NUNCA por git (gitignored). Copialas a mano a cada host.
- Un escritor (pc-mateo). Las demás pull. Evita conflictos.
- `claude -p` mode=edit escribe; corre sobre repos con git (rollback). plan=read-only.

## Atajo
Abrí Claude Code EN pc-mateo y pedile que ejecute esta receta — esa sesión sí puede hacerlo
allá (yo desde acá no alcanzo esa máquina).
