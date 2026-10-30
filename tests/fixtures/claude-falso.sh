#!/bin/sh
# Builder falso para los tests de `corre`. No llama a `claude`. Emite stream-json en stdout y crea
# `hecho.txt` en su cwd. Sale con 9 si ve una clave de API o ROOT_HOME en su entorno.
# Variable de prueba: CLAUDE_FALSO_LEE_ORACULO=<ruta>. Si está, emite una llamada Bash que nombra esa
# ruta, como si el builder leyera un archivo del oráculo. Nunca la ejecuta.
if [ -n "${DEEPSEEK_API_KEY+x}" ] || [ -n "${ANTHROPIC_API_KEY+x}" ] || [ -n "${ROOT_HOME+x}" ]; then
  echo "entorno con clave de API o ROOT_HOME" >&2
  exit 9
fi

modelo=desconocido
previo=""
for arg in "$@"; do
  if [ "$previo" = "--model" ]; then
    modelo="$arg"
  fi
  previo="$arg"
done

cwd=$(pwd)
printf 'hecho\n' > hecho.txt

cat <<EOF
{"type":"system","subtype":"init","cwd":"$cwd","session_id":"s1","model":"$modelo"}
EOF

if [ -n "${CLAUDE_FALSO_LEE_ORACULO+x}" ]; then
  cat <<EOF
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"r1","name":"Bash","input":{"command":"cat $CLAUDE_FALSO_LEE_ORACULO"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"r1","content":"contenido"}]}}
EOF
fi

cat <<EOF
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"w1","name":"Write","input":{"file_path":"$cwd/hecho.txt","content":"hecho\n"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"w1","content":"ok"}]}}
{"type":"result","subtype":"success","is_error":false,"duration_ms":1000,"num_turns":2,"total_cost_usd":0.01,"usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"modelUsage":{"$modelo":{"inputTokens":10,"outputTokens":5,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"costUSD":0.01}},"session_id":"s1"}
EOF
