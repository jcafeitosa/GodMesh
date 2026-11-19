# Discagem QUIC entre pares autorizados

A coordenação aprova a [SPEC v1.1](../../08-specs/federacao/quic-discagem-pares-pendentes.md) de SHA-256 `7a17956e287d61b33c06707f897c6ab45804fdd7b0a458dee7d01d6e56ebc3ee`, após revisão independente, conforme ADR-075. A base inclui o [aceite M4-B2-B](2026-11-18-m4-b2-b-transporte-snapshot.md#aceite-operacional). Esta correção antecede a B2-C.

Em ensaio no binário de `eaab4a6`, com ingresso oficial e dados descartáveis em loopback, o socket silencioso de B recebeu o datagrama QUIC inicial e permaneceu pendente. C estava autorizado e tinha endpoint válido. O Pull de C deixou de progredir dentro da guarda de quatro segundos. O resultado reproduz a precedência da discagem de B no mantenedor sequencial.

Se a chave mTLS divergir do pin, o discador encerra a conexão antes de enviar o primeiro manifesto. Nesse caso, nenhuma ponta conforme chega a `ready`. Se o manifesto recebido divergir da identidade mTLS, a ponta receptora recusa antes de publicar seu próprio `ready`. O envio do manifesto pelo aceitador, no wire vigente, deixa a validação pelo discador sem confirmação bilateral. A correção preserva o ALPN e a autorização M1C/M3.

1. **Transporte pinado.** Vincular ID, chave e endereço à discagem. Testar o endpoint que apresenta outro par autorizado e a recusa local de manifesto divergente em cada direção.
2. **Ciclo da tentativa.** Serializar publicação e cancelamento. Testar as duas ordens entre TLS e promoção, recolhimento e invalidação restrita à conexão original.
3. **Mantenedor.** Distribuir duas vagas FIFO, preservar fallback sequencial por par e atualizar a allowlist em cadência independente. Testar duplicação, justiça, revogação, troca de endpoint, falha de refresh e progresso com B pendente.
4. **Parada e aceite.** Recolher discagens e refresh SQL no shutdown. Confirmar estado saudável no pool descartável. Repetir o ensaio A/B/C em loopback, executar a regressão B2-B, suíte e Clippy. Registrar revisão independente antes de integrar.

## Aceite

Código integrado em `3ca12a1dfbff44ddba03aaaf227d6aa7004ba81b`. A revisão independente aprovou o transporte pinado, o mantenedor e o ajuste que cancela as tentativas invalidadas antes de publicar a atualização do registro.

- **Discagem no fio.** Em IPv4 e IPv6, B recebeu o datagrama de A antes que C subisse. B continuou silencioso. Em `eaab4a6`, C ficou sem progresso na guarda de quatro segundos. No candidato corrigido, C confirmou o Pull dentro da guarda e A encerrou por SIGTERM enquanto B continuava pendente. Cada rodada usou três processos no mesmo host, bancos descartáveis e ingresso oficial.
- **Concorrência e identidade.** Os testes cobriram duas vagas, FIFO, fallback, falha de refresh e PID descartado após cancelamento SQL sob lock. O transporte recusou pin divergente antes do manifesto e preservou a evidência de recusa sem sobrescrever sessão paralela. Com QUIC real, a retirada do endpoint após TLS encerrou a tentativa sem callback tardio se o cancelamento venceu. Se a promoção venceu, a substituta continuou `ready` e abriu uma stream depois da invalidação da conexão original.
- **Regressão.** Passaram 636 testes padrão e 645 com `--all-features`, sem falhas. O teste de retenção passou isoladamente. O entrypoint de laboratório foi executado nos smokes de snapshot em IPv4 e IPv6, ambos aprovados. O ensaio A2 IPv6, 76 invariantes SQL, upgrade idempotente `0016 → 0017` e Clippy estrito com e sem a feature também passaram. As migrações e o lockfile conservaram os bytes.

O aceite encerra este recorte e abre a etapa de aprovação da B2-C. O ensaio em três hosts Linux independentes permanece pendente.
