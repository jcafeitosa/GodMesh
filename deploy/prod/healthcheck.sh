#!/bin/bash
# Healthcheck do contêiner de produção do godmesh-server.
#
# Verifica duas coisas, nesta ordem:
#   1. GET /healthz no endereço de bind EFEITIVO (BIND_ADDR), esperando 200;
#   2. alcance TCP do Postgres cujo host/porta vêm de DATABASE_URL.
#
# LIMITES CONHECIDOS (não esconda isto):
#   - O `/healthz` atual NÃO consulta o pool (crates/godmesh-server/src/http.rs
#     devolve JSON estático e ignora o estado). O correto é ele responder 503
#     quando o pool falhar; enquanto isso não existir, a checagem de banco aqui é
#     de ALCANCE TCP — prova porta aberta, não prova consulta bem-sucedida.
#   - DATABASE_URL com host unix-socket (ex.: ?host=/var/run/postgresql) não é
#     verificável por TCP; o script avisa e segue só com o /healthz.
#   - IPv6 literal em DATABASE_URL não é suportado por este parser.
#
# Sem curl/wget/nc na imagem: `/dev/tcp` do bash faz o trabalho.

set -uo pipefail

falha() {
    echo "healthcheck: $*" >&2
    exit 1
}

# ---------- 1. /healthz no bind efetivo ----------
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

# ---------- 2. alcance do Postgres ----------
if [ -z "${DATABASE_URL:-}" ]; then
    falha "DATABASE_URL ausente; o servidor não sobe sem banco"
fi

autoridade="${DATABASE_URL#*://}"
autoridade="${autoridade%%/*}"
autoridade="${autoridade##*@}"

if [ -z "$autoridade" ] || [ "${autoridade#/}" != "$autoridade" ]; then
    echo "healthcheck: DATABASE_URL sem host TCP (unix socket) — só /healthz verificado" >&2
    exit 0
fi

db_host="${autoridade%%:*}"
db_port="5432"
if [ "$db_host" != "$autoridade" ]; then
    db_port="${autoridade##*:}"
fi

if ! exec 4<>"/dev/tcp/$db_host/$db_port"; then
    falha "Postgres inalcançável em $db_host:$db_port (DATABASE_URL)"
fi
exec 4<&- 4>&-

exit 0