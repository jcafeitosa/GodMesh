#!/bin/bash
# Healthcheck do contêiner de produção do godmesh-server.
#
# Verifica, nesta ordem:
#   1. GET /healthz no endereço de bind EFEITIVO (BIND_ADDR), exigindo 200.
#      Desde 18/11/2026 o `/healthz` **consulta o pool** (`SELECT 1` com timeout
#      de 500 ms em crates/godmesh-server/src/http/mod.rs): 200 prova que o
#      Postgres respondeu; 503 `{"status":"unavailable","dependency":"postgres"}`
#      quando o banco não responde. Esta é a checagem AUTORITATIVA.
#   2. Alcance TCP do Postgres derivado de DATABASE_URL — **diagnóstico
#      secundário**, nunca decide sozinho: serve para dizer no log se a porta
#      está fechada quando o /healthz falha, e não derruba o contêiner quando o
#      /healthz passou (pool já provou consulta bem-sucedida).
#
# LIMITES CONHECIDOS (não esconda isto):
#   - O `/healthz` prova uma consulta ao pool, não a saúde de cada tabela, do
#     outbox, da réplica primária nem do disco. Não é readiness de negócio.
#   - DATABASE_URL com host unix-socket (ex.: ?host=/var/run/postgresql) não é
#     verificável por TCP; o script avisa e segue só com o /healthz (que continua
#     válido, porque o pool usa o mesmo caminho).
#   - IPv6 literal em DATABASE_URL não é suportado por este parser; a checagem
#     TCP é pulada com aviso e o /healthz segue mandando.
#   - A checagem TCP abre e fecha um socket; não autentica nem consulta.
#
# Sem curl/wget/nc na imagem: `/dev/tcp` do bash faz o trabalho.

set -uo pipefail

falha() {
    echo "healthcheck: $*" >&2
    exit 1
}

aviso() {
    echo "healthcheck: $*" >&2
}

# ---------- 1. /healthz no bind efetivo (autoritativo) ----------
# Espelha a regra do binário (crates/godmesh-server/src/main.rs): BIND_ADDR é
# obrigatório fora de dev; em dev o default é loopback 127.0.0.1:8080.
bind="${BIND_ADDR:-}"
if [ -z "$bind" ]; then
    case "$(printf '%s' "${GODMESH_ENV:-}" | tr '[:upper:]' '[:lower:]')" in
        dev|development) bind=127.0.0.1:8080 ;;
        *) falha "BIND_ADDR ausente e GODMESH_ENV=${GODMESH_ENV:-<ausente>} não é dev; sem endereço efetivo para checar" ;;
    esac
fi

port="${bind##*:}"
host="${bind%:*}"
case "$host" in
    0.0.0.0|'*'|'::'|'[::]'|'') host=127.0.0.1 ;;
esac
host="${host#[}"
host="${host%]}"

if ! exec 3<>"/dev/tcp/$host/$port"; then
    falha "não conectou em $host:$port (bind declarado: $bind)"
fi
printf 'GET /healthz HTTP/1.0\r\nHost: %s\r\n\r\n' "$host" >&3
resposta="$(cat <&3)"
exec 3<&- 3>&-

linha_status="$(printf '%s\n' "$resposta" | head -n 1)"
case "$linha_status" in
    *" 200 "*) ;;
    *) falha "/healthz em $host:$port não respondeu 200: ${linha_status:-<vazio>}" ;;
esac
printf '%s' "$resposta" | grep -q '"status":"ok"' || falha "/healthz sem status ok"

# ---------- 2. alcance do Postgres (diagnóstico secundário) ----------
# Nunca decide o resultado depois que o /healthz passou: só explica no log.
if [ -z "${DATABASE_URL:-}" ]; then
    aviso "DATABASE_URL ausente no ambiente do healthcheck; banco não diagnosticado (o /healthz já provou o pool)"
    exit 0
fi

autoridade="${DATABASE_URL#*://}"
autoridade="${autoridade%%/*}"
autoridade="${autoridade##*@}"

if [ -z "$autoridade" ] || [ "${autoridade#/}" != "$autoridade" ]; then
    aviso "DATABASE_URL sem host TCP (unix socket) — só /healthz verificado"
    exit 0
fi

db_host="${autoridade%%:*}"
db_port="5432"
if [ "$db_host" != "$autoridade" ]; then
    db_port="${autoridade##*:}"
fi

case "$db_host" in
    *:*)
        aviso "host IPv6 literal em DATABASE_URL ($db_host) não suportado pela checagem TCP — só /healthz verificado"
        exit 0
        ;;
esac

if ! exec 4<>"/dev/tcp/$db_host/$db_port"; then
    aviso "Postgres inalcançável em $db_host:$db_port (diagnóstico de DATABASE_URL); /healthz respondeu 200, então o pool está de pé — não reprova"
    exit 0
fi
exec 4<&- 4>&-

echo "healthcheck: ok — /healthz 200 (pool consultado) e Postgres alcançável em $db_host:$db_port"
exit 0