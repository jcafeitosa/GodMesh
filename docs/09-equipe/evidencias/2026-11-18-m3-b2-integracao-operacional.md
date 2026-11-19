# M3-B2 — integração operacional do piso de configuração

A resposta do usuário para continuar após a solicitação de autorização libera a [SPEC M3-B2 v1.2](../../08-specs/federacao/m3-b2-integracao-operacional.md), o adaptador `tokio-rustls` e o nível 1 para `POST /v1/events`. O texto recebeu parecer independente favorável no SHA-256 `475b476ee0eb7566372ba7bd843910530a0b34202f2083e0f137a8ae5b0a4711`. A base `f646842` inclui o [aceite M3-B1](2026-11-18-m3-b1-servico-duravel.md). M4 permanece condicionada ao aceite operacional.

1. **Transporte.** Reutilizar a verificação de chaves Ed25519 do QUIC em TLS 1.3 dedicado, com ALPN HTTP/1.1, pin no cliente e verificação mTLS no servidor. Testar ausência de certificado, pin divergente, saturação e prazos de handshake/cabeçalhos.
2. **API e pull.** Ligar publicação, seleção condicional e resolução à transação B1. Revalidar a identidade autenticada na aceitação do pull. Testar status, ETag, limites, perda de resposta, revogação concorrente e orçamento de tentativas.
3. **Runtime e aplicação.** Validar a política antes dos listeners. Ativar no relógio da B1 e aplicar o piso transacional no QUIC e ingest sob o guard M1C. Testar mudança em sessão aberta, vencimento e recuperação por HTTPS.
4. **Aceite.** Executar testes de regressão e revisão independente. Subir três processos com chaves e bancos próprios e verificar no fio a ativação de piso 2, o bloqueio do QUIC e do ingest e a recuperação por um bundle superior.

A documentação oficial registra `tokio-rustls` 0.26.6 estável, MIT ou Apache-2.0 e Rust mínimo 1.71. Os requisitos Tokio 1 e Rustls 0.23.27 ou superior na série 0.23 são compatíveis com 1.54.1 e 0.23.46 já resolvidos. O adaptador liga o I/O assíncrono ao Rustls existente. Desabilitar features padrão e manter Ring. A implementação deve conferir a resolução e a árvore de features. Consulta de 18/11/2026. [Manifesto publicado](https://docs.rs/crate/tokio-rustls/0.26.6/source/Cargo.toml) · [Avisos RustSec](https://rustsec.org/packages/tokio-rustls.html)

## Implementação e verificação

O commit de código `da1474f6af272e05b6478747bfa94aba3203e7d8` liga o transporte mTLS, a API, o pull e a aplicação do piso. A suíte usou PostgreSQL 16.15 descartável. As migrações mantiveram os mesmos bytes. A comparação dos pacotes resolvidos no lockfile identificou apenas a adição de `tokio-rustls 0.26.6`.

| Verificação | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` | 551 aprovados, zero falhas e um teste ignorado por desenho. |
| `cargo test --locked --offline -p godmesh-server --test retention_policy_missing -- --ignored` | Um aprovado em banco próprio. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 57 invariantes em banco criado do zero. |
| `git diff --check 39682cc da1474f` | Exit 0. |

A revisão independente localizou quatro problemas, com regressões reproduzidas antes das correções. O transporte passou a usar o mesmo schema de erro do app para recusas anteriores ao handler. A fase PostgreSQL do pull passou a compartilhar o prazo da tentativa, com prazo SQL local e tratamento separado de commit incerto. O agendador passou a limitar a espera no próprio PostgreSQL e a ativar e consultar o piso sob o mesmo guard e segundo. Após cancelar e aguardar as tarefas auxiliares, o finalizador do binário espera até dois segundos pelo fechamento gracioso do pool. Se esse prazo esgota, registra diagnóstico e a saída do runtime encerra os sockets restantes. [Prazos SQL locais](https://www.postgresql.org/docs/16/runtime-config-client.html#GUC-STATEMENT-TIMEOUT) · [Fechamento do pool SQLx](https://docs.rs/sqlx/latest/sqlx/struct.Pool.html#method.close)

## Aceite em três instâncias

O binário do commit foi executado em três processos A/B/C no mesmo host, com bancos, chaves e listeners próprios, em rodadas IPv4 e IPv6. A malha foi formada pelas APIs M1C oficiais com A como raiz e publicador, B como delegado e B/C como receptores. SQL da fixture ficou restrito a retenção e escopo de ingest sintéticos, barreiras de concorrência e leitura do estado durável.

1. Confirmamos GET mTLS `200` com ETag forte, `304` sem corpo, recusa de cliente sem certificado e ausência da rota no HTTP comum. Após publicar o piso 1, as seis direções chegaram a `ready` e o ingest assinado gravou posição.
2. Com o piso 2 pendente, as seis direções permaneceram `ready`. Um ingest autorizado antes da ativação concluiu sob o lock e a ativação só confirmou depois. Após o piso 2 efetivo, as seis direções perderam `ready` e novo ingest retornou `503 config_fence_blocked` sem gravar posição.
3. A publicação HTTPS do piso 1 superior recuperou a malha. O cliente fechou o canal após observar o commit, antes de ler a resposta. Reaplicar o mesmo frame retornou `200` e preservou a única entrada de auditoria.
4. Um lock externo permaneceu retido durante 16 sondagens de saúde no publicador e todas retornaram `200`. Com GET mTLS aguardando o PostgreSQL, `SIGTERM` encerrou A com exit 0 antes da liberação do lock. A partição permitiu observar B↔C em `ready` até o vencimento do snapshot, quando o enlace foi retirado. Retirar a política na reinicialização de A bloqueou o boot. Restaurá-la e publicar um frame superior recuperou os enlaces.
5. Dois frames autenticados e divergentes na mesma versão abriram conflito em A. GET em A retornou `503 config_conflict`; B e C preservaram sua configuração vigente. A resolução explícita e a versão superior restauraram as seis direções.

Uma compilação descartável a partir do mesmo commit injetou relógio nos três processos. Confirmamos a versão 2 ainda pendente em `applies_at - 1` e o registro de ativação no segundo exato em todos os bancos. O ensaio também confirmou o fechamento dos enlaces de A diante de timeout do PostgreSQL e sua recuperação após liberar o lock. Suprimindo a notificação de conflito na fixture, o agendador periódico ainda retirou `ready` nos enlaces de A.

A regressão M1C em IPv6 também passou com M3 desabilitada, cobrindo ingresso, revogação, recuperação de prova após perda de resposta e ingest ordenado contra revogação. A validação foi local com dados sintéticos.
