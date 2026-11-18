#!/usr/bin/env bash
# Verificação dos invariantes de banco contra um banco DESCARTÁVEL.
#
# Cria um banco próprio e aplica, nesta ordem:
#   migrations/0001_core.sql
#   migrations/0002_incident_tenancy.sql
#   migrations/0003_outbox_publisher.sql
#   migrations/0004_grant_revocation.sql
#   migrations/0005_sync_inbox.sql
#   migrations/0006_incident_listing.sql
#   migrations/0007_grant_issuance.sql
# depois roda todos os `*_invariants.sql` e falha (exit != 0) se algum bloco
# `NOTICE: OK` esperado não aparecer.
#
# Por que não basta rodar psql e olhar a saída: os scripts usam
# `\set ON_ERROR_STOP off` (um bloco que falha não deve derrubar os demais) e,
# nesse modo, o psql devolve 0 mesmo depois de um ERROR. A prova é a lista de
# `OK` abaixo — ela é intencionalmente explícita para que remover ou renomear
# uma asserção quebre este script.
#
# Uso:
#   migrations/tests/run_all.sh                 # cria e remove o banco descartável
#   migrations/tests/run_all.sh --keep          # mantém o banco para inspeção
#   migrations/tests/run_all.sh --sabotage F    # aplica o SQL de F antes dos testes
#                                               # (só para provar que a falha é detectada)
#
# Conexão: variáveis PG* padrão (PGHOST/PGPORT/PGUSER/PGPASSWORD). O banco
# descartável é criado a partir de PGADMIN_DB (default: postgres). O banco de
# desenvolvimento compartilhado nunca é tocado: nada de DROP/TRUNCATE nele.

set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tests="$raiz/migrations/tests"
admin_db="${PGADMIN_DB:-postgres}"
manter=0
sabotagem=""

while [ $# -gt 0 ]; do
    case "$1" in
        --keep) manter=1 ;;
        --sabotage)
            [ $# -ge 2 ] || { echo "erro: --sabotage exige um arquivo" >&2; exit 2; }
            sabotagem="$2"; shift ;;
        -h|--help) sed -n '2,25p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "erro: argumento desconhecido: $1" >&2; exit 2 ;;
    esac
    shift
done

banco="godmesh_inv_$(date +%Y%m%d%H%M%S)_$$"
psql_admin=(psql -X -q -v ON_ERROR_STOP=1 -d "$admin_db")

limpar() {
    if [ "$manter" -eq 1 ]; then
        echo "banco mantido para inspeção: $banco"
        return
    fi
    "${psql_admin[@]}" -c "DROP DATABASE IF EXISTS $banco" >/dev/null 2>&1 || true
}
trap limpar EXIT

echo "== banco descartável: $banco"
"${psql_admin[@]}" -c "CREATE DATABASE $banco"

aplicar() {
    local arquivo="$1"
    echo "== aplicando ${arquivo#"$raiz"/}"
    psql -X -q -v ON_ERROR_STOP=1 -d "$banco" -f "$arquivo" >/dev/null
}

aplicar "$raiz/migrations/0001_core.sql"
aplicar "$raiz/migrations/0002_incident_tenancy.sql"
aplicar "$raiz/migrations/0003_outbox_publisher.sql"
aplicar "$raiz/migrations/0004_grant_revocation.sql"
aplicar "$raiz/migrations/0005_sync_inbox.sql"
aplicar "$raiz/migrations/0006_incident_listing.sql"
aplicar "$raiz/migrations/0007_grant_issuance.sql"

if [ -n "$sabotagem" ]; then
    [ -f "$sabotagem" ] || { echo "erro: sabotagem inexistente: $sabotagem" >&2; exit 2; }
    echo "== SABOTAGEM aplicada de $sabotagem"
    psql -X -q -v ON_ERROR_STOP=1 -d "$banco" -f "$sabotagem" >/dev/null
fi

# Blocos exigidos por arquivo. Cada token é comparado literalmente no NOTICE.
esperado_0001=(
    "OK 1: evento sem política recusado"
    "OK 2: evento com política aceito"
    "OK 3: posição duplicada recusada"
    "OK 4: ACK append-only (UPDATE bloqueado pelo trigger)"
    "OK 5: ACK append-only (DELETE bloqueado pelo trigger)"
    "OK 6: estado inventado recusado"
)
esperado_0002=(
    "OK 1: incidente sem classe de retenção recusado"
    "OK 2a: jurisdição fora de BR/CO recusada"
    "OK 2b: incidente sem política recusado (FK)"
    "OK 3: incidente_sos semeado para CO com cópia primária"
    "OK 4: incidente registra a política aplicada (incidente_sos@BR)"
    "OK 5: colisão de id entre tenants recusada"
    "OK 6: PK de sos_incident é (tenant_id, id)"
    "OK 7: política em uso não pode ser removida (FK)"
)
esperado_0003=(
    "OK 1: confirmed_at sem published_at recusado (CHECK)"
    "OK 2: publicado sem confirmado aceito"
    "OK 3: publicado e então confirmado aceito"
    "OK 4: apagar published_at de linha confirmada recusado"
    "OK 5: attempts inicia em 0 e recusa negativo"
    "OK 6: entrega duplicada da mesma posição recusada"
)
esperado_0004=(
    "OK 1: token_id duplicado recusado (PK)"
    "OK 2: motivo vazio recusado (CHECK)"
    "OK 3: revoked_at preenchido por default"
    "OK 4: consulta distingue ausente de revogado"
)
esperado_0005=(
    "OK 1: namespace de Inbox duplicado recusado (PK)"
    "OK 2: contiguous_prefix negativo recusado"
    "OK 3: partição negativa recusada"
    "OK 4: authority_epoch zero recusado"
    "OK 5: partição, epoch e origem são dimensões do namespace (4)"
    "OK 6: prefixo persiste a avançar no mesmo namespace (3 -> 4)"
)
esperado_0006=(
    "OK 1: list_seq atribuída pelo banco cresce na inserção"
    "OK 2: list_seq não aceita escrita explícita"
    "OK 3: índice sos_incident_listagem (tenant_id, list_seq DESC) presente"
)
esperado_0007=(
    "OK 1: token_hash fora do formato sha256 hex recusado (CHECK)"
    "OK 2: hash de token duplicado recusado (UNIQUE)"
    "OK 3: grant sem escopo recusado (CHECK)"
    "OK 4: janela invertida recusada (CHECK)"
    "OK 5: grant_issued guarda token_hash e nenhuma coluna de token"
)

saida="$(mktemp)"
falhas=0
total_ok=0

verificar() {
    local arquivo="$1"; shift
    local -a esperado=("$@")
    local nome
    nome="$(basename "$arquivo")"

    echo
    echo "===== $nome ====="
    psql -X -v ON_ERROR_STOP=0 -d "$banco" -f "$arquivo" 2>&1 | tee "$saida"
    echo

    for bloco in "${esperado[@]}"; do
        if grep -Fq "NOTICE:  $bloco" "$saida"; then
            total_ok=$((total_ok + 1))
        else
            echo "FALTA: $nome -> \"$bloco\"" >&2
            falhas=$((falhas + 1))
        fi
    done

    if grep -Eq '(^|: )ERROR:' "$saida"; then
        echo "ERRO: $nome produziu ERROR no banco" >&2
        falhas=$((falhas + 1))
    fi
}

verificar "$tests/0001_invariants.sql" "${esperado_0001[@]}"
verificar "$tests/0002_invariants.sql" "${esperado_0002[@]}"
verificar "$tests/0003_invariants.sql" "${esperado_0003[@]}"
verificar "$tests/0004_invariants.sql" "${esperado_0004[@]}"
verificar "$tests/0005_invariants.sql" "${esperado_0005[@]}"
verificar "$tests/0006_invariants.sql" "${esperado_0006[@]}"
verificar "$tests/0007_invariants.sql" "${esperado_0007[@]}"

rm -f "$saida"

echo
if [ "$falhas" -ne 0 ]; then
    echo "RESULTADO: FALHOU — $falhas verificação(ões) ausente(s)/inválida(s)." >&2
    exit 1
fi

echo "RESULTADO: OK — $total_ok invariantes verificados em banco criado do zero."