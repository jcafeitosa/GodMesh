# Plano de engenharia — sistema integral, sem redução do escopo

> **Objetivo:** construir o GodMesh completo, com gates verificáveis por subsistema. A ordem expressa dependências técnicas, **não** exclusão de funcionalidades nem promessa de prazos.


## Prioridade de implementação futura aprovada pelo usuário

Primeiro: servidor completo Rust distribuído em contêiner Linux; desktop macOS universal Apple Silicon (arm64) e Intel (x86_64) com SwiftUI; mobile iOS com SwiftUI. Windows, Linux desktop e Android permanecem no planejamento integral em etapas posteriores. A prioridade é de plataformas e não exclui capacidades do servidor completo; web e ecossistema permanecem no escopo integral sem antecipação de sua implementação nesta etapa.

Preferir o máximo de bibliotecas nativas Rust, sujeito a manutenção, segurança, licença e compatibilidade. Axum, SQLx, Serde, Garde, Utoipa, Scalar e Swift OpenAPI continuam candidatos/propostas; esta lista não aprova seleção, versão ou instalação. Interfaces Apple continuam SwiftUI/Swift. Redis/cache e tarefas versus JetStream dependem dos contratos e decisões de dados/backend; não são serviços obrigatórios aprovados por esta lista.

Conforme ordem posterior transmitida por Ada, a autoria de todas as SPECs avança em paralelo à conclusão do planejamento integral, correções e decisões bloqueadoras. Conclusão do planejamento e aprovação de todas as SPECs, com as revisões exigidas, precedem a implementação; rascunhos locais não antecipam esse gate. Os gates de execução nos programas abaixo são de implementação/validação futura, não exigência de produto passando para encerrar planejamento. Todos os resultados observados de produto continuam NOT_EXECUTED até execução autorizada e evidência.

Para a federação, a [ADR-075](decisoes.md#adr-075--aprovação-por-fatia-da-federação) estabelece revisão e aprovação por fatia. Os resultados observados ficam vinculados à versão e ao ambiente de cada ensaio.

## Programa A — Linguagem do sistema, identidade e contratos
GMP v1, envelope de eventos, autenticação de usuário/nó/dispositivo, tenants, permissões, observabilidade, Rust workspace, OpenAPI/Protobuf, CI e cadeia de releases assinadas.
**Gate:** contrato de interoperabilidade versionado, autenticação, migrações, teste multi-tenant e threat model revisados.

## Programa B — GodMesh Native Voice
PTT/floor control, Opus, WebRTC para web, media edge nativo, prioridade/ACL, salas regionais, reconexão, sessão e métricas de áudio.
**Gate:** comunicação GodMesh↔GodMesh independente de qualquer Mumble/FRN; carga e falhas ensaiadas.

## Programa C — Federação de nós e operação regional
Identidade persistente, IPv6-first/IPv4, libp2p QUIC/TCP, bootstrap autorizado, manifests, readiness, balanceamento regional, controle distribuído e leases.
**Gate:** diversos servidores na mesma região operando active-active, sem ingresso anônimo nem split brain em recursos exclusivos.

### Sequência de entrega da malha

1. **Ingresso M1C.** Emitir e consumir prova de ingresso v2 verificável a partir de genesis, com histórico durável e revogação assinada. Provar A → B → C pela API, com bancos e identidades próprios. [ADR-076](decisoes.md#adr-076--prova-portátil-e-revogação-de-ingresso)
2. **Transporte M2.** Integrar a cadeia ao registro de pares, ao QUIC/mTLS e ao ingresso de eventos. Verificar que expiração, revogação e perda de conectividade degradam os estados e que recuperação exige evidência nova.
3. **Configuração M3.** Distribuir bundles assinados por escopo, com defesa contra rollback, quórum declarado onde exigido, ativação atômica e recuperação documentada. [ADR-072](decisoes.md#adr-072--adoção-da-rede-alvo-de-cinco-planos)
4. **Replicação M4.** Exercitar eventos e snapshots autorizados entre bancos independentes, retomada por prefixo, revogação de escopos e ausência de posição duplicada após partição.
5. **Borda M5.** Especificar e verificar relay autorizado, alternativa de conectividade e comportamento de nós que perdem o caminho direto.
6. **Aceite ponta a ponta.** Executar em pelo menos três hosts Linux independentes. Registrar identidade, versão, topologia, autorização, evidências do protocolo e resultados de queda, revogação, retomada e bloqueio de cópias fora do escopo.

A M4-A2 demonstrou réplica direta de metadados sintéticos em três processos locais, com retomada após partição e perda de ACK, revogação por destino e suspensão persistente por histórico indisponível. A M4-B2-B demonstrou snapshot assinado por QUIC e reconciliação após perda de ACK e efeito incerto em três processos locais, em IPv4 e IPv6. A M4-B2-C demonstrou restore planejado em três processos locais, com fence PostgreSQL por geração, decisão externa de promoção e retomada pelo checkpoint. Relay e o aceite em hosts independentes seguem pendentes. [Evidência M4-A2](../09-equipe/evidencias/2026-11-18-m4-a2-replica-quic.md#aceite-operacional-em-três-instâncias) · [Evidência M4-B2-B](../09-equipe/evidencias/2026-11-18-m4-b2-b-transporte-snapshot.md#aceite-operacional) · [Evidência M4-B2-C](../09-equipe/evidencias/2026-11-19-m4-b2-c-restore-planejado.md#aceite-em-três-processos)

## Programa D — Dados, sincronização e localização
PostgreSQL/PostGIS, sharding/autoridade por tenant, outbox/inbox, snapshots assinados, histórico, observações, dispositivos, retenção e consentimento.
**Gate:** partição, reconciliação idempotente, revogação e restauração testadas.

## Programa E — Universal Gateway e dispositivos
Perfis/firmware, discovery autorizada, capability router, LoRaWAN/ChirpStack, LoRa mesh, BLE, GPS/LTE/Traccar, MQTT, SMS/integrações permitidas onde aplicável.
**Gate:** múltiplas classes de hardware presentes sob identidade única com funcionalidades corretamente avaliadas.

## Programa F — Bridges e redes de rádio
Bridge SDK, cliente Mumble, cliente FRN, autenticação externa, tradução de áudio e PTT, anti-loop, isolamento por tipo de rede, failover exclusivo de ponte.
**Gate:** cada bridge pode ser instalada, desabilitada e derrubada sem impactar GodMesh Native; TX nunca acontece sem autorização.

## Programa G — Rede mundial e alta escala
Várias células regionais, roteamento global por saúde/residência, replicação por tenant, object storage, autoscaling, DR, upgrades com versões mistas, quotas, observabilidade e cobrança opcional.
**Gate:** testes de falha de região e carga, com degradação controlada, custo de egress e recovery mensurados.

## Programa H — Aplicações e ecossistema
Web; desktop macOS universal Apple Silicon/Intel com SwiftUI, Windows e Linux; mobile iOS com SwiftUI e Android; mapas, alertas, UX de rádio, portal de organizações, SDKs, plugins, documentação pública, testes de acessibilidade e interoperabilidade de terceiros. WinUI 3, Qt Quick e Jetpack Compose são recomendações técnicas propostas na ADR-055, sujeitas aos critérios ali definidos.
**Gate:** fluxos principais Personal, Teams e Node completos e observáveis.

## Qualidade em todos os programas
Threat modeling, política de dados, proteção anti-stalking, revisão legal RF, privacidade, métricas, testes de regressão e recuperação. **Nenhum programa pode afirmar confiabilidade para emergências antes de avaliação operacional independente.**

## Sequência de coordenação
Podem existir frentes paralelas de Voice, Devices e Federação depois que contratos/identidade estiverem estabelecidos. Dependências e blockers devem ser mantidos em [Matriz de rastreabilidade](matriz-rastreabilidade.md).
