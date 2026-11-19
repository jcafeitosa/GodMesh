# Registro de decisões arquiteturais — versão oficial

> Decisão aprovada significa **intenção arquitetural**, não funcionalidade já entregue. Qualquer documento anterior conflitante fica substituído por estas ADRs e pela [Arquitetura de referência](../01-arquitetura/arquitetura-referencia.md).

| ID | Decisão | Status |
|---|---|---|
| ADR-001 | Nome do projeto GodMesh; rede federada chamada Global Mesh | Aprovada |
| ADR-002 | Rust no núcleo/servidores Linux | Aprovada |
| ADR-003 | Ativos, dispositivos e transportes são entidades distintas | Aprovada |
| ADR-004 | Núcleo multi-tenant para Personal, Teams e Node | Aprovada |
| ADR-005 | Full nodes com serviços completos e regiões Active-Active | Aprovada |
| ADR-006 | IPv6 preferencial, IPv4 alternativo | Aprovada |
| ADR-007 | QUIC/libp2p como base candidata para federação (validar) | Em validação |
| ADR-008 | Descoberta automática e ingresso apenas autorizado | Aprovada |
| ADR-009 | Snapshots e eventos persistentes por escopo; sem SQL global multi-writer livre | Aprovada |
| ADR-010 | Espelhamento de software/serviços, dados só com autorização/residência | Aprovada |
| ADR-011 | Governança replicada e quórum somente onde necessário | Aprovada; implementação a especificar |
| ADR-012 | Balanceamento por capacidade e saúde nas regiões e globalmente | Aprovada |
| ADR-013 | Capacidades verificadas e motor de roteamento de serviços | Aprovada |
| ADR-014 | Separação estrita de mídia, eventos, governança e acesso | Aprovada |
| ADR-015 | Mumble como motor PTT principal | **SUPERADA por ADR-021/022** |
| ADR-016 | FRN como rede externa opcional | Aprovada |
| ADR-017 | ChirpStack/LoRaWAN como adaptador para dados | Aprovada |
| ADR-018 | LoRaWAN clássico para dados curtos, não voz PTT em tempo real | Aprovada |
| ADR-019 | Monólito modular para domínio; processos dedicados onde mídia/segurança exigem | Aprovada |
| ADR-020 | Conectores externos só com autorização e regras de espectro | Aprovada |
| ADR-021 | GodMesh Protocol (GMP) e motor Voice nativos em Rust | Aprovada |
| ADR-022 | Mumble/FRN somente por Bridge SDK opcional e isolado | Aprovada |
| ADR-023 | GodMesh é sistema **integral**, não reduzido a uma versão mínima | Aprovada |
| ADR-024 | Uma arquitetura de referência normativa prevalece sobre brainstorm histórico | Aprovada |
| ADR-025 | Clientes podem contribuir, mas não são relays permanentes garantidos | Aprovada |
| ADR-026 | Não construir cifras, codecs nem camada física próprias | Aprovada |
| ADR-027 | Readiness por serviço e separação software instalado/hardware necessário | Aprovada |
| ADR-028 | Pontes RF/FRN usam autoridade exclusiva com fencing e TX opt-in | Aprovada |
| ADR-029 | APIs e transportes versionados; futuros protocolos só via adaptadores testados | Aprovada |
| ADR-030 | Privacidade, revogação, E2EE condicionado e prevenção de rastreamento abusivo por padrão | Aprovada |
| ADR-031 | Tecnologia própria significa contratos de aplicação, voz e federação GodMesh sobre padrões modernos, abertos e auditáveis; não reinventar IP, QUIC, TLS ou codecs | Aprovada |
| ADR-032 | Sistemas externos (Mumble, FRN, ChirpStack e outros) pertencem exclusivamente à camada de bridges/adaptadores opcionais, isolados do núcleo | Aprovada |
| ADR-033 | Adoção de tecnologias futuras é extensível e negociada, mas condicionada à implementação real, avaliação de segurança, compatibilidade e rollout testado | Aprovada |

| ADR-034 | GMI: identificador de usuário imutável UUIDv7 (`urn:uuid:`), `acct:apelido@domínio` via WebFinger, OIDC (`iss`+`sub`) e WebAuthn/passkeys; identidade GodMesh reconhecida apenas por federação autenticada | Aprovada |

| ADR-035 | GMC Callsign: indicativo digital global pseudônimo, único e verificável; `gmi_id` interno e `acct:` opt-in; sem exposição de identidade civil nas comunicações nativas por padrão | Aprovada como direção; formato final a padronizar |
| ADR-036 | Identidade em mensagens/PTT: indicativo na mensagem/sinalização de `TalkStart`, `stream_id` efêmero nos frames; vinculação criptográfica e rejeição de impersonação/replay | Aprovada como direção; implementação pendente |

| ADR-037 | Camada Universal de Normalização de Dados (UDNL) bidirecional no núcleo: identidade externa escopada, evidência de confiança, eventos canônicos, projeção com perdas declaradas; áudio PTT transportado separadamente do log de eventos | Aprovada como direção; contratos a padronizar |
| ADR-038 | Cada canal de áudio tem timeline de texto e suporte a transcrição ASR identificada por GMC/identidade externa; políticas por canal, consentimento, retenção e plano de mídia separado | Aprovada como direção; implementação/legislação a validar |

| ADR-039 | SOS é evento estruturado independente de ASR, com roteamento prioritário, ACK por camada, confirmação humana explícita, operação offline de melhor esforço e integração pública somente por acordo | Aprovada como direção; serviço ainda não validado |

| ADR-040 | Toda transmissão PTT observável gera evento/timeline independente de transcrição; indisponibilidade ASR apresenta motivo e preserva origem/certeza, inclusive sem atribuir falante a rádio analógico sem identificação | Aprovada como direção; implementação pendente |

| ADR-041 | Localização no chat/PTT/transcrição: somente região consentida com incerteza de no mínimo 50 km de raio; SOS explicitamente acionado: posição mais precisa disponível exclusivamente a responsáveis autorizados e auditados, em canal separado, nunca no chat público | Aprovada como direção; implementar e testar |

| ADR-042 | Guardian: contexto terra/ar/mar é da atividade corrente; acompanhamento temporário é voluntário, autorizado, com expiração e aviso de atraso que **não equivale a SOS** | Aprovada como arquitetura; testes pendentes |
| ADR-043 | Guardian Beacon/CrowdFind: smartphone pode anunciar token BLE rotativo temporário e participar como observador mediante opt-in e limites Android/iOS; rede colaborativa **própria** | Aprovada como arquitetura; interoperabilidade em teste |
| ADR-044 | Privacidade CrowdFind: sem GMC/GMI/coords em beacon; observação protegida, detecção consentida, prevenção de rastreamento não autorizado e revogação | Aprovada como arquitetura; auditoria necessária |
| ADR-045 | Monitoramento de rede móvel e transporte satelital pelo DICE: sinais de fonte/certeza, sem declarar satélite ativo sem evidência; Starlink Direct-to-Cell ≠ terminal Wi-Fi | Aprovada como arquitetura; APIs e operadores a validar |
| ADR-046 | Automação somente com API/consentimento; quando indisponível, interface orienta **ação manual**; não fecha apps terceiros, não altera assinatura/operadora/satélite e preserva funções essenciais de Guardian e SOS | Aprovada como regra de produto |

## Procedimento de mudança
Criar ADR detalhada com contexto, alternativas, decisão, consequências, status, data, relação com requisitos, contratos e testes. Marcar decisões anteriores como **SUPERADA**, sem apagá-las do histórico. Alterações precisam atualizar README, documentação relacionada e changelog no mesmo commit.

## ADRs — operação cotidiana, Wi-Fi colaborativo e Live Presence (2026-11-15)

| ID | Decisão | Status |
|---|---|---|
| ADR-047 | App tem modos de atividade **simultâneos** (PTT, Guardian, SOS, conectividade), com núcleo Rust, interfaces Android/iOS e funcionamento offline honesto | Aprovada; implementação pendente |
| ADR-048 | Wi-Fi AutoDiscover e AutoConnect ativados como preferências; AutoShare ativado como intenção, mas redes de terceiros nunca são publicadas sem prova de autoridade e consentimento | Aprovada; APIs móveis restritas |
| ADR-049 | Cada ponto de acesso físico administrado possui registro geográfico preciso quando medido, origem, qualidade, quem cadastrou e quem publicou; localização exata no mapa somente por opt-in específico | Aprovada; revisão de privacidade necessária |
| ADR-050 | Wi-Fi Access Registry registra conexões e consumo apenas com evidência da fonte, preferindo atestação de controlador; não coletar histórico de navegação nem expor visitantes publicamente | Aprovada; infraestrutura de AP a homologar |
| ADR-051 | Android usa APIs de sugestão de redes sob escolha do SO; iOS usa configuração aprovada pelo usuário, sem tentar enumerar senhas/redes salvas fora das APIs oficiais | Aprovada; homologação por SO |
| ADR-052 | Live Presence usa ActivityKit/Live Activities no iOS e Live Updates ou notificações adequadas no Android apenas para atividades elegíveis; sem Dynamic Island permanente para presença simples | Aprovada; modelos/versões a testar |
| ADR-053 | SOS, Guardian e PTT recebem priorização explícita no app; Wi-Fi discovery, transcrição remota opcional e telemetria cedem recursos sob bateria/link restritos | Aprovada; medir impacto |
| ADR-054 | Ponto de acesso geográfico ≠ localização pessoal. A regra de 50 km para chat/PTT permanece; a posição precisa de um AP pode ser publicada voluntariamente, sem expor dados civis do administrador ou visitantes | Aprovada; política distinta |

## ADR-055 — Plataformas de entrega e seleção de interfaces

**Estado:** **SUPERADA por ADR-068 (17/11/2026).** O usuário decidiu unificar todos os clientes em tecnologia web. Este registro permanece como histórico da decisão anterior; plataformas e SwiftUI nativas deixam de ser o alvo.

**Estado original:** plataformas e SwiftUI aprovados por instrução explícita do usuário; recomendações específicas de Windows/Linux/Android propostas, não validadas. Registro documental; não inicia SPECs nem implementação.

**Origem:** nesta conversa, o usuário definiu servidor em contêiner Linux, macOS universal Apple Silicon e Intel com SwiftUI, Windows/Linux e mobile iOS com SwiftUI e Android; em seguida pediu usar o que há de melhor em Windows, Linux e Android. A segunda instrução estabelece o objetivo de seleção; não nomeia um framework específico.

### Decisão aprovada
- Servidor: núcleo Rust distribuído em contêiner Linux. A separação de mídia, gateways e bridges da ADR-019 permanece; contêiner não significa concentrar todos os processos/dependências em uma única imagem.
- macOS: interface SwiftUI e artefato universal com arm64 e x86_64; dependências nativas distribuídas também precisam atender às arquiteturas requeridas.
- Windows e Linux: clientes desktop obrigatórios.
- iOS: interface SwiftUI. Android: cliente obrigatório, preservando integração Kotlin com o núcleo Rust.
- Desenvolvimento: dependências no OrbStack; backend, frontend e apps locais, conforme diretriz anterior do usuário. O contêiner é o alvo de distribuição do servidor, não uma exigência de executar o backend dentro dele em cada ciclo local.

### Recomendação e alternativas
| Plataforma | Recomendação técnica | Fundamento e contrapartida |
|---|---|---|
| Windows | WinUI 3 / Windows App SDK; linguagem da camada de interface e binding Rust ainda a definir | Framework recomendado pela Microsoft para novos apps Windows. C# e C++ são suportados; escolher exige avaliar custo de manutenção, fronteira com Rust e consumo real. Qt Quick é alternativa se manutenção compartilhada com Linux tiver maior peso. |
| Linux | Qt 6 / Qt Quick / QML; mecanismo de integração com Rust ainda a definir | Qt documenta suporte Linux e UI QML extensível por C++. Recomendação de engenharia para o escopo Linux amplo, não prova de superioridade. GTK 4 com gtk-rs é alternativa forte para integração Rust e orientação GNOME; libadwaita segue a HIG GNOME. Qt exige avaliação dos módulos e termos de licença antes da adoção. |
| Android | Kotlin / Jetpack Compose | Toolkit moderno recomendado pelo Android para UI nativa. Mantém adaptação ao SO em Kotlin e regras compartilhadas em Rust. Desempenho e funcionamento em aparelhos reais continuam não medidos. |

A opção por interfaces diferentes aumenta o trabalho de UI, acessibilidade, build, distribuição e testes. Compartilhar o núcleo Rust busca evitar duplicar regras de negócio; não implica reaproveitar toda a interface nem um único binário entre plataformas. Nenhuma opção foi demonstrada como a mais leve, rápida ou segura no GodMesh.

### Critérios de fechamento
- Definir matriz de SO/arquitetura, hardware mínimo, acessibilidade, instalação/assinatura/atualização e política de suporte por plataforma.
- Comparar startup, memória, CPU, tamanho distribuído, energia, mapas, áudio/PTT, notificações e background em jornadas representativas, com metas aprovadas e evidência futura.
- Definir bindings do núcleo Rust, propriedade de memória, threads, cancelamento, erros e compatibilidade; nenhuma biblioteca de binding foi escolhida aqui.
- Conferir licença por módulo/dependência, manutenção, custo operacional e compatibilidade. Usar latest estável na resolução e registrar versão efetivamente resolvida, fonte, data e digest para imagens; não confundir documentação rolling com dependência instalada.
- Propor responsáveis: Martin Fowler/Grady Booch (arquitetura), Lea Verou (UI/acessibilidade), Chet Haase (Android), Donny Wals (Apple), Alex Crichton (núcleo/bindings), Brendan Gregg (build/validação) e Lawrence Rosen (licenças). São propostas de articulação, não novas atribuições aceitas.

### Fontes oficiais consultadas
- [Microsoft — WinUI 3](https://learn.microsoft.com/en-us/windows/apps/winui/winui3/): recomendação nativa e suporte C#/C++.
- [Android — Jetpack Compose](https://developer.android.com/compose): toolkit recomendado, APIs Kotlin.
- [Qt — Linux](https://doc.qt.io/qt-6/linux.html), [Qt Quick](https://doc.qt.io/qt-6/qtquick-index.html) e [licenciamento](https://doc.qt.io/qt-6/licensing.html): páginas rolling exibiam Qt 6.12 durante a consulta; versão estável a resolver, nenhum pacote instalado.
- [gtk-rs — libadwaita](https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html): bindings Rust e integração GNOME.
- [Apple — binário universal](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary): consulta via representação Markdown oficial; contém código por arquitetura e tratamento das dependências.

**Rastreabilidade:** requisito de plataforma = instrução direta do usuário; requisitos numerados específicos, SPECs, código e IDs de testes: não atribuídos nesta decisão. Relacionados: ADR-002, ADR-019, ADR-023 e ADR-047; arquitetura de referência, serviços, implantação, roadmap e matriz de rastreabilidade. Execução e desempenho: não verificados.

## ADR-056 — Compatibilidade e evolução: N/N-1 por perfil, rejeição antes do efeito

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026; janela numérica de suporte depende de decisão de produto |
| **Contexto** | RNF-008 exige compatibilidade N/N-1; `estrategia-tecnologica.md` a condiciona a "quando viável"; SPEC-001 §4.3 registra a tensão sem resolvê-la (achado do pod P1 e do crítico C2) |
| **Decisão** | N/N-1 é obrigatória para GMP wire, API pública e schema persistido **dentro de uma janela de suporte declarada por perfil versionado** (protocolo, schema, firmware, cliente). "Quando viável" deixa de ser qualificador normativo. Peer sem versão/feature crítica comum é rejeitado **antes de qualquer efeito**, sem downgrade silencioso. Exceção só existe em perfil publicado, com janela, impacto, autoridade que autorizou e plano de rollback. Schema persistido segue as regras de evolução do Protobuf: número de campo não é reutilizado nem alterado; campo desconhecido é preservado; campo removido entra em `reserved`. |
| **Evidência externa** | [RFC 9369 QUIC v2](https://www.rfc-editor.org/rfc/rfc9369.html) (mai/2023) mantém v1 para interoperar e exercita negociação de versão; [RFC 8446 TLS 1.3](https://www.rfc-editor.org/rfc/rfc8446.html) adverte contra fallback alternativo por risco de downgrade; [Protocol Buffers proto3](https://protobuf.dev/programming-guides/proto3/) — "This number cannot be changed once your message type is in use" e campo desconhecido preservado. Consulta em 17/11/2026. |
| **Consequências** | Custo de matriz de compatibilidade e testes por perfil; nenhum provedor pode anunciar suporte nominal. Rollback e canary passam a ser pré-condição de release. |
| **Verificação** | Teste de matriz por perfil (N, N-1, N-1 sem feature crítica) com resultado esperado: aceita/rejeita sem efeito; teste negativo de downgrade silencioso; teste de leitura de registro antigo por binário novo. |
| **Pendente do usuário** | Tamanho da janela (ex.: 6, 12 ou 24 meses) e quais perfis entram no v1. |

## ADR-057 — Posição canônica de evento, cursor e namespace

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026 |
| **Contexto** | SPEC-004 §5 propõe `origem + tenant + stream + partição + epoch + sequência`; SPEC-005 §6 e SPEC-021 §6 ilustram chaves sem partição explícita. C2 registrou colisão possível entre partições e cursor legado aceito após troca de epoch. |
| **Decisão** | Posição canônica é a tupla ordenada `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. `authority_epoch`, `dispatch_epoch`, `event_sequence` e cursor permanecem tipos distintos e não intercambiáveis. Cursor é opaco, versionado e vinculado a `(principal, escopo, filtros, perfil, retenção)`. Cursor sem partição ou de epoch incomparável é **rejeitado**, nunca traduzido. |
| **Evidência interna** | SPEC-004 §§3–5 e 8; SPEC-005 §6; SPEC-021 §§3 e 6; veto C2-2; achado P3 sobre uso genérico de "epoch" em SPEC-019. |
| **Consequências** | Migração de cursor exige operação explícita e autenticada; contratos de API precisam expor a versão do cursor. |
| **Verificação** | Teste de colisão com duas partições e mesma sequência; teste de replay de cursor após mudança de epoch (deve falhar fechado); teste de rejeição de cursor sem partição. |

## ADR-058 — Taxonomia normativa de ACK por fronteira

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026 |
| **Contexto** | Documentação distingue ACK em vários lugares (SPEC-001 §4.2, SPEC-004 §4, SPEC-006, README regra 12) sem tabela única; C2 (C2-3) e C4 mostraram risco de a UI tratar sucesso técnico como entrega ou atendimento. |
| **Decisão** | Conjunto fechado de classes, cada uma com emissor, evidência e escopo obrigatórios: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device`, `acknowledged_human`. Toda resposta de API carrega `ack_class`. É proibido usar "ACK" sem classe; é proibido inferir classe superior por passagem de tempo, por sucesso de camada inferior ou por ausência de erro. `delivered_to_device` e `acknowledged_human` exigem evidência própria do destino/pessoa. |
| **Evidência interna** | SPEC-001 §4.2; SPEC-004 §4 (ACK limitado a prefixo contíguo na Inbox); SPEC-006 (ACK humano separado); veto C2-3; achado G3-06. |
| **Consequências** | Contrato de eventos, contratos de API e UI passam a precisar do campo; resumos em outros documentos deixam de ser normativos. |
| **Verificação** | Teste negativo: resposta com `persisted_local` não pode renderizar estado de entrega; teste de perda de confirmação (resultado `unknown`, não sucesso); teste de escalada de ACK proibida. |

## ADR-059 — Escopo do grant offline e negações explícitas

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026; valores de frescor ficam pendentes de medição |
| **Contexto** | `pendencias.md` §128 registra a direção aprovada "autorização assinada e limitada". SPEC-002 §§6.1–6.2, SPEC-019 §5 e SPEC-012 dependem dela. C2-4 e C4 mostraram que "grant válido" sem predicado vira replay. |
| **Decisão** | Grant offline autoriza apenas operações enumeradas e reversíveis: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) enviar mensagem a canal já autorizado; (4) registrar observação de localização local; (5) PTT apenas em rede local com grant de floor válido. Negado sempre: divulgar localização precisa, mutação federada, criação/alteração de grant, mudança de identidade, transmissão RF, transcrição remota. Condicional exige `epoch` comparável, frescor dentro do limite do perfil, ausência de revogação conhecida e estado anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. |
| **Evidência interna** | `pendencias.md` §128; SPEC-002 §§6–7; SPEC-004 (replay/restore); veto C2-4; achados C4 sobre replay de grant. |
| **Consequências** | Cliente precisa armazenamento protegido e contador monotônico; o limite de frescor só pode ser fixado após medição de uso legítimo em campo. |
| **Verificação** | Testes negativos: restore de snapshot antigo com grant na validade; grant com epoch anterior; grant de ação não enumerada; revogação conhecida chegando após o snapshot. |

## ADR-060 — Confidencialidade: E2EE nativo por padrão, exceção explícita, proibição de alegação

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026; ratificação de produto pendente para ASR no servidor |
| **Contexto** | RF-022 pede E2EE onde suportada; ADR-030/037/038/040 tratam privacidade e transcrição; SPEC-020 §5 e SPEC-008 deixam perfis e autorização de ASR OPEN; C4 apontou risco de alegação indevida. |
| **Decisão** | Mensagem 1:1 e de grupo entre endpoints GodMesh nativos usam MLS ([RFC 9420](https://www.rfc-editor.org/rfc/rfc9420.html)) como base de acordo de chaves. Voz nativa usa chaves de sessão SRTP ([RFC 3711](https://www.rfc-editor.org/rfc/rfc3711.html)) derivadas do mesmo contexto. ASR no servidor só existe com consentimento explícito por canal, que **retira** E2EE daquele canal e é mostrado ao usuário. Toda perna que termine proteção (bridge, ASR, gravação) expõe `confidentiality=terminated` na API e na UI. É proibido chamar TLS/DTLS/SRTP de E2EE, prometer anonimato, apagar cópia já entregue ou recuperar conteúdo E2EE sem chave. |
| **Evidência externa** | [RFC 9420 MLS](https://www.rfc-editor.org/rfc/rfc9420.html) (jul/2023, Standards Track): chaves acordadas entre endpoints, servidores fora do acesso ao conteúdo; [RFC 3711 SRTP](https://www.rfc-editor.org/rfc/rfc3711.html) (mar/2004). Consulta em 17/11/2026. |
| **Consequências** | Busca, moderação e ASR em conteúdo E2EE deixam de ser possíveis por padrão; exigem decisão de produto por canal. Bridges ficam visivelmente marcadas. |
| **Verificação** | Teste negativo: servidor não decifra mensagem de grupo; remoção de dispositivo/membro atualiza chaves e nega acesso a mensagens futuras; `confidentiality` presente e fiel em toda resposta; bridge marcada. |

## ADR-061 — Fronteira transacional, Inbox/Outbox e recuperação

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026; RPO/RTO numéricos dependem de custo e decisão de produto |
| **Contexto** | SPEC-004 §§8–9 mantém aberta a escolha entre transação única e ledger/projetor; SPEC-005 §6 e SPEC-021 §6 dependem dela; C2-2/C2-3 e achado G4-13 mostram risco de ACK de ledger virar prova de projeção. |
| **Decisão** | Por serviço de domínio, o padrão é transação única no banco de domínio com `outbox` na mesma transação (publicação assíncrona após commit). Inbox durável em ledger separado com projetor idempotente é permitido **apenas** onde recepção e projeção não podem compartilhar transação, e nesse caso o ACK máximo é `persisted_local`, nunca `materialized`. Nenhum efeito externo irreversível é derivado de replay: replay reconstrói projeção, não repete chamada a bridge, rádio, pagamento ou notificação. Estado de autoridade `stale/unknown` bloqueia mutação forte. |
| **Evidência externa** | PostgreSQL: [WAL](https://www.postgresql.org/docs/current/wal-intro.html) e [PITR/timelines](https://www.postgresql.org/docs/current/continuous-archiving.html) recuperam o cluster, não provam efeito externo; [restrições de replicação lógica](https://www.postgresql.org/docs/18/logical-replication-restrictions.html) (DDL e sequences não replicam); [RLS](https://www.postgresql.org/docs/18/ddl-rowsecurity.html) nega por padrão sem política, mas dono/superusuário/BYPASSRLS contornam; [Debezium Outbox Event Router](https://debezium.io/documentation/reference/stable/transformations/outbox-event-router.html) encaminha, não atomiciza consumidor; [Kafka design](https://kafka.apache.org/44/design/design/) limita "exactly once" ao escopo Kafka; [Raft](https://raft.github.io/raft.pdf) alerta que lease depende de relógio e não impede escritor antigo. Consulta em 17/11/2026. |
| **Consequências** | Publicador idempotente e consumidor idempotente são obrigatórios; retenção de outbox precisa de política; restore exige reconciliação entre estado restaurado e eventos publicados. |
| **Verificação** | Teste de crash entre commit e publish; restore de backup com Inbox em voo; partição com dois líderes (fencing deve negar o antigo); replay que não reemite efeito externo. |

## ADR-062 — Retenção fail-closed e política obrigatória por classe

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026; prazos e bases legais são decisão jurídica/produto por jurisdição |
| **Contexto** | SPEC-005 §10, SPEC-020 §§5–10 e `pendencias.md` mantêm prazos, finalidades e jurisdição abertos; G5-24 mostra que "preservar auditoria" pode virar retenção indefinida. |
| **Decisão** | Nenhuma classe de dado pode ser persistida sem política aprovada identificada por `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)`. Sem política, a persistência **falha fechada** e a operação degrada de forma explícita, preservando apenas o mínimo necessário para o efeito imediato autorizado. Auditoria tem prazo e revisão próprios; hold exige autoridade, escopo e revisão, e nunca reativa acesso comum. Revogação é declarada por destino/cópia, nunca global. |
| **Evidência interna** | `pendencias.md` (linha de base 3A: bloquear mutações fortes sem autoridade/fence; piso global com restrições locais); SPEC-005 §10; SPEC-020 §10; achados G5-22, G5-23, G5-24. |
| **Consequências** | Desenvolvimento não pode "deixar para depois" a política de retenção: a tabela de classes precisa existir antes de gravar dados pessoais. Pode bloquear features que dependem de histórico. |
| **Verificação** | Teste de escrita em classe sem política (deve falhar com erro explícito); teste de expiração e descarte por cópia; teste de hold com autoridade e liberação; teste de revogação por destino. |

### Emenda à ADR-060 (17/11/2026) — autoridade da herança e campo `confidentiality`

Achado do crítico C4: a regra de herança de confidencialidade existia apenas em `07-contratos/autoridade-contratos.md` §3, documento que declara não aprovar política, e o campo não existia em contrato nem em wire. Correção normativa:

1. **Obrigação de herança**: todo derivado — transcrição, gravação, projeção de timeline, snapshot de incidente, payload entregue a bridge — herda `confidentiality=terminated` quando **qualquer** perna de origem ou de destino terminar a proteção. Basta uma perna. Cadeia de duas bridges preserva a marca.
2. **Campo obrigatório e enum fechado**: `confidentiality` ∈ {`e2ee`, `terminated`, `unknown`}, obrigatório em resposta de API que exponha conteúdo ou estado de conteúdo. Ausência do campo ou valor fora do enum invalida a resposta. `unknown` é tratado como `terminated` por qualquer consumidor.
3. **Proibição de alegação**: API e UI rejeitam rótulo de E2EE quando `confidentiality != e2ee`. TLS, DTLS e SRTP não promovem a classe.
4. **Verificação**: teste negativo de cadeia bridge→ASR→transcrição mantendo `terminated`; teste de campo ausente → resposta inválida; teste de rótulo E2EE sobre perna terminadora → rejeitado.

## ADR-063 — Baseline de cadeia de suprimento e atualização assinada

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026 |
| **Contexto** | G5-17 e G5-18: não há evidência documental de SBOM, builds reproduzíveis, assinatura de artefato, verificação no cliente, canal de atualização ou resposta a chave de assinatura comprometida. RF-015 exige atualização assinada como direção. |
| **Decisão** | Todo artefato distribuído (servidor, desktop, mobile, firmware) tem: SBOM por release, build reproduzível, assinatura verificada no cliente, canal de atualização autenticado, política de rollback e procedimento de revogação de chave. Dependência nova entra com versão resolvida, fonte, data, licença, análise de vulnerabilidade e digest registrados. |
| **Evidência interna** | RF-015; `docs/01-arquitetura/descoberta-ingresso.md`; AGENTS.md (registrar versão resolvida, fonte e data); achados G5-17, G5-18. |
| **Consequências** | Adiciona etapa obrigatória ao pipeline; releases sem assinatura não são distribuíveis. |
| **Decisão (emenda 17/11/2026 — achado C4)** | Assinatura prova origem e integridade, **não** atualidade nem ausência de revogação. Por isso todo artefato carrega `generation` monotônica autenticada e o cliente mantém um **piso de versão** assinado pela autoridade de release. Cliente rejeita artefato com `generation` inferior ao piso. **O piso autoritativo vive na autoridade de release, não no cliente**: toda atualização exige consulta online (ou instalação por operador) que devolva o piso vigente; o valor local é cache, restaurável, e nunca autoriza sozinho. Sem verificação online, atualização não é aplicada. Rollback só como exceção autorizada, limitada no tempo, auditada e com rebaixamento registrado do piso. Chave de assinatura comprometida tem procedimento de revogação e reemissão documentado, com lista de revogação verificada no cliente. |
| **Verificação** | Assinatura verificada em instalação limpa; build repetido comparando digest; artefato adulterado rejeitado; **downgrade para release vulnerável rejeitado pelo piso**; rollback autorizado aceito e auditado; revogação de chave aplicada. |

## ADR-064 — SOS offline: negar sem fonte de frescor confiável

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada 17/11/2026 (decisão do usuário: "acatar a sugestão") |
| **Contexto** | A ADR-059 condiciona ação offline a frescor e prova anti-rollback; SPEC-002 §6.3 exige alta marca na autoridade; o crítico C4 mostrou que aparelho sem armazenamento protegido ou sem relógio confiável pode repetir grant. |
| **Decisão** | Se o dispositivo não puder demonstrar fonte F1 (contador monotônico protegido por hardware) nem F2 (âncora de tempo confiável) e não houver F3 (verificação online), **toda operação offline condicional é negada**, incluindo abertura de incidente SOS por grant. A UI deve declarar o estado real ("modo offline indisponível neste aparelho") antes do incidente, não depois. Negar cobre inclusive o registro local mínimo: sem raiz de confiança não há fila, não há estado `queued` e a UI não pode sugerir que algo foi salvo. Negar não encerra o incidente: o app oferece o caminho disponível — chamada de emergência do sistema, mensagem por rede presente, ou nova tentativa com verificação online. **Risco residual aceito explicitamente:** em aparelho sem F1/F2 e com rede degradada, o SOS por grant offline fica indisponível; o único caminho é o discador do sistema. F2 só conta como fonte se funcionar offline; se exigir rede, é F3. |
| **Consequências** | Preferimos não entregar SOS offline em aparelho sem raiz de confiança a aceitar replay. Isso reduz a cobertura de emergência em hardware barato; a mitigação é F3 (verificação online obrigatória quando houver qualquer conectividade) e orientação explícita de uso do discador do sistema. |
| **Verificação** | Caso negativo: snapshot restaurado com grant válido → negado com estado observável. Caso negativo: sem F1/F2 e sem rede → negado, sem promessa de persistência. Caso positivo: com F3 disponível → verificação ocorre antes do efeito e o resultado é registrado. |
| **Pendente** | Quais plataformas expõem F1/F2 (levantamento por SO/versão); texto de UI por idioma. |

## ADR-065 — Política inicial de retenção por classe

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente 17/11/2026 pelo CTO a pedido do usuário ("selecione o melhor para os usuários e o sistema"); revisão jurídica por jurisdição obrigatória antes de operação comercial |
| **Contexto** | A ADR-062 exige política por classe para não bloquear persistência. Faltavam prazos. Critério de escolha: o mínimo que sustenta a função declarada, com folga para disputa, suporte e auditoria — não o máximo que a lei permitiria. |
| **Decisão** | Tabela em [retenção por classe](../04-seguranca/retencao-revogacao.md#política-inicial-de-retenção-por-classe-adr-065), agora com as seis dimensões da ADR-062 por linha (finalidade, classe, cópia, jurisdição, prazo, gatilho). A tabela fixa **prazo padrão por classe**; a política só é efetiva quando cada linha declara finalidade, cópia e jurisdição aplicáveis — até lá vale o fail-closed. Regras gerais: (1) prazo é por classe e por cópia; (2) tenant pode reduzir, nunca ampliar sem nova decisão; (3) ao expirar, descarte é verificado e registrado, não presumido; (4) hold legal suspende descarte com autoridade, escopo e revisão a cada 90 dias; (5) conteúdo já entregue a destinatário ou bridge não é recolhível — a política vale para cópias sob nosso controle. |
| **Consequências** | Histórico longo deixa de ser padrão; produtos que dependem de memória longa (ex.: retrospectiva de trajeto) exigem classe própria aprovada. Custo de armazenamento cai e a superfície de vazamento também. |
| **Verificação** | Teste por classe: gravação aceita com política; expiração descarta e registra; classe sem política falha fechada; redução por tenant aplicada; hold impede descarte e é revisado. |

## ADR-066 — Operação mundial com base legal no Brasil e na Colômbia

| Campo | Conteúdo |
|---|---|
| **Status** | Direção aprovada pelo usuário 17/11/2026; entidade controladora, contratos e pareceres por país pendentes de revisão profissional |
| **Contexto** | O produto é global, mas não existe operação sem controlador identificado, foro e região de dados. O usuário definiu base no Brasil ou Colômbia. |
| **Decisão** | **Região primária: Brasil (São Paulo)** — idioma do time, maior mercado inicial e lei de proteção de dados com autoridade constituída. **Região secundária: Colômbia (Bogotá)** para latência, resiliência regional e expansão em espanhol. Usuário de qualquer país usa o serviço aceitando o contrato e a política de dados da região que o atende; residência de dados é declarada por tenant, e a réplica entre regiões só existe conforme classe e autorização (ADR-062/061). Foro e entidade controladora são definidos com parecer jurídico; até lá, o contrato declara explicitamente qual região atende qual país. |
| **Consequências** | Duas regiões full-stack desde o início elevam custo e exigem DR testado entre elas; a promessa de residência só pode ser feita para dados cuja cópia esteja mapeada. Usuários fora dessas regiões aceitam transferência internacional declarada. |
| **Cópias e classes** | Replicam para a região secundária **apenas**: identidade e chaves públicas, catálogo de configuração, metadados de roteamento e cópias de recuperação declaradas por classe na ADR-065. **Não replicam** sem autorização explícita: conteúdo de mensagem E2EE, áudio, posição, transcrição ASR, auditoria de acesso a dado sensível. Backup, telemetria, push e logs seguem a mesma regra de classe e são declarados por cópia. |
| **Verificação** | Dois testes **complementares**, não exclusivos: (a) tenant brasileiro não tem conteúdo restrito replicado na Colômbia sem autorização; (b) falha da região primária mantém continuidade degradada apenas dos serviços cuja cópia é autorizada — o resto informa indisponibilidade. Teste de residência declarada no contrato e na API. |
| **Pendente do usuário/jurídico** | Entidade e foro; bases legais por finalidade; contrato de transferência internacional; obrigações de radioamador e de emergência por país. |


### Emenda à ADR-067 (17/11/2026) — shadcn/ui permanece, com regra de convivência

Decisão do usuário: "shadcn fica". Regras para não virar dois sistemas:
1. **Um componente, um dono.** Botão, dialog, menu, tabela, input, toast, tabs, tooltip, popover, sheet, avatar, badge, progress, skeleton: **Astryx**. shadcn entra apenas para padrões que o Astryx não oferece — e cada adoção é registrada em `web/ui/README.md` com motivo.
2. **Tema único.** shadcn é configurado para consumir as mesmas CSS custom properties do tema Astryx; não existe paleta paralela.
3. **Fronteira.** Nenhuma tela importa shadcn direto: sempre por `web/ui/`. Isso permite remover um dos dois sem tocar em telas.
4. **Acessibilidade e atualização** são verificadas por componente adotado, não por biblioteca.
5. **Critério de revisão:** se um padrão do shadcn passar a existir no Astryx, o shadcn correspondente é removido no ciclo seguinte.

## ADR-067 — Stack da interface web: Astryx + Tailwind, sem segunda biblioteca de componentes

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 17/11/2026 por instrução do usuário ("vamos usar o https://github.com/meta/astryx"); substitui a menção anterior a shadcn/ui como base de componentes. Ratificação de produto pendente apenas quanto a shadcn conviver ou não. |
| **Contexto** | O usuário indicou shadcn/ui + Tailwind "latest" com referência de padrões em coss.com e alignui.com e, em seguida, Astryx (`meta/astryx`). shadcn e Astryx são **sistemas de componentes distintos**: shadcn distribui código Radix+Tailwind para copiar; Astryx é uma biblioteca de 150+ componentes construída em StyleX, com tema por CSS custom properties e override por `className`. Adotar os dois como base significa manter dois vocabulários de componentes, dois catálogos de acessibilidade e duas curvas de atualização. |
| **Decisão** | **Componentes: Astryx** (`@astryxdesign/core`). **Utilitários e layout: Tailwind** por `className`, que o Astryx suporta explicitamente sem lock-in. **Tokens de marca**: CSS custom properties do tema Astryx, com os mesmos nomes expostos ao Tailwind. **shadcn/ui não é adotado como base**; só entra, caso a caso, se um padrão de composição específico faltar no Astryx — e nesse caso a decisão é registrada como exceção, não como segundo sistema. **coss.com e alignui.com ficam como referência de padrão visual e de composição**, não como dependência: coss.com é um provedor de API (e-mail, SMS, inbox de agente, formulários, webhooks) — potencial integração da família G2, não biblioteca de UI. |
| **Versões resolvidas em 17/11/2026** | `@astryxdesign/core` 0.7.16 (npm `latest`; CLI 0.7.16) · React 19.3.1 e react-dom 19.3.1 (peer `>=19`) · `@stylexjs/stylex` 0.19.2 (peer `^0.19.0`) · Tailwind CSS 4.3.5 · shadcn CLI 4.23.0 (referência, não adotado) · Next.js 16.4.6 · TypeScript 7.0.2. Fontes: `npm view <pacote> version` e README do repositório, consultados em 17/11/2026. Registro de versão resolvida, não de instalação. |
| **Consequências** | (1) Astryx está em **beta** (0.7.x): API pode mudar; por isso o consumo fica atrás de uma camada de composição própria em `web/ui/`, nunca espalhado pelas telas. (2) StyleX vira dependência de runtime apenas para componentes; telas usam Tailwind, sem misturar os dois no mesmo arquivo além do `className` público. (3) Um sistema de componentes, um catálogo de acessibilidade, uma atualização. (4) Perde-se o ecossistema de blocos prontos do shadcn; ganha-se 150+ componentes mantidos e CLI orientado a agente, que é o modo de trabalho deste repositório. |
| **Verificação** | Instalação limpa resolvendo as versões acima com digest registrado; build com Astryx + Tailwind sem conflito de reset; auditoria de acessibilidade em 3 telas; teste de tema claro/escuro só por CSS vars; teste de que nenhuma tela importa componente de duas bibliotecas; verificação de que um upgrade minor do Astryx não exige edição fora de `web/ui/`. |
| **Pendente do usuário** | Confirmar adoção exclusiva do Astryx como base (shadcn fora), ou autorizar shadcn como segunda base e assumir o custo duplo. |
| **Escopo** | Vale para a interface **web**. macOS/iOS seguem SwiftUI e Android segue Kotlin/Compose conforme ADR-055; se a intenção for unificar todos os clientes em tecnologia web, isso revoga a ADR-055 e exige decisão explícita — não é o que este ADR decide. |
"
## ADR-068 — Todos os clientes em tecnologia web, com núcleo Rust embarcado

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada pelo usuário em 17/11/2026 ("sim" à unificação). **Supera a ADR-055.** Impacto em SPEC-013, 014, 015 e 016: esses documentos precisam de nova versão; SwiftUI, WinUI 3, Qt Quick e Jetpack Compose saem como alvo de UI. |
| **Contexto** | A ADR-055 fixava interfaces nativas por plataforma (SwiftUI, WinUI 3, Qt Quick, Compose) e um núcleo Rust exposto por FFI. O usuário decidiu uma única tecnologia web para todos os clientes, com Astryx + shadcn + Tailwind (ADR-067). Isso reduz quatro bases de UI a uma e **elimina a necessidade de FFI Rust↔Swift↔Kotlin para a camada de aplicação**. |
| **Decisão** | Cliente único em **React 19 + Astryx + shadcn + Tailwind**, empacotado com **Tauri 2** para macOS, Windows, Linux, **iOS e Android**. O núcleo Rust (GMP, sync, identidade, mídia) roda **no processo do app** como backend Tauri, exposto por comandos/eventos tipados — não por FFI manual. Onde o sistema operacional não permite web API, o recurso é implementado como **plugin nativo do Tauri**, com contrato tipado e teste próprio: BLE/beacon, localização em segundo plano, sessão de áudio/PTT em segundo plano, notificações push, Live Activities/Dynamic Island, NFC/biometria, atalhos e integração com o discador. UI nativa por plataforma deixa de existir como código de produto; plugins nativos são a única superfície nativa. |
| **Versões resolvidas em 17/11/2026** | `@tauri-apps/cli` 2.12.5 e `@tauri-apps/api` 2.12.5 · `@astryxdesign/core` 0.7.16 · Tailwind CSS 4.3.5 · shadcn CLI 4.23.0 · React 19.3.1 · Next.js 16.4.6 (web) · TypeScript 7.0.2 · `@stylexjs/stylex` 0.19.2. Fontes: `npm view`, README de `meta/astryx` e documentação oficial `v2.tauri.app` (início, pré-requisitos, plugins: barcode, biometric, NFC, geolocation, notifications, deep linking), consultados em 17/11/2026. |
| **Consequências e riscos** | (1) **Ganho:** uma UI, um design system, um pipeline; núcleo Rust único em todas as plataformas; FFI some do caminho crítico. (2) **Risco de WebView:** WKWebView (iOS/macOS), WebView2 (Windows), WebKitGTK (Linux) e Android WebView divergem em APIs, mídia e desempenho — a matriz de suporte passa a ser por *engine*, não por toolkit. (3) **Segundo plano é o ponto fraco:** PTT contínuo, rastreamento e BLE exigem plugin nativo e política de bateria própria; sem isso, RF-005/RF-028/RF-030/RF-042 não são entregáveis. (4) **Live Activities/Dynamic Island** exigem extensão nativa iOS, não são web. (5) Tauri mobile está em 2.x estável, mas o ecossistema de plugins móveis é menor que o nativo puro — cada plugin usado precisa de dono e teste. (6) Tamanho e assinatura de app mudam por plataforma. |
| **Verificação** | Build do mesmo código para as 5 plataformas; teste de PTT em segundo plano por plataforma; teste de BLE e localização em background; teste de push; teste de Live Activity; auditoria de acessibilidade em WebView; medição de memória/CPU/bateria por engine; teste de que nenhum código de tela conhece a plataforma, só capacidades declaradas. |
| **Impacto documental obrigatório** | ADR-055 sai de vigência; SPEC-013 (experiência por plataforma), SPEC-014 (cliente offline/FFI) e as SPECs de plataforma 015/016 precisam de nova versão **substituindo "Kotlin/Compose" e "Swift/SwiftUI" por "cliente web + plugins Tauri"**, preservando os contratos de lifecycle, permissões e capacidades que já estavam especificados. SPEC-007 (PTT), SPEC-012 (Guardian) e SPEC-024 (CrowdFind) ganham requisito de plugin nativo em vez de API de plataforma nativa. |

## ADR-069 — Versões latest e documentação oficial como pré-requisito de implementação

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada pelo usuário em 17/11/2026 |
| **Contexto** | As decisões de stack (ADR-067, ADR-068) usam versões `latest` e bibliotecas em beta, cuja API muda. Registro de versão resolvida já existia (ADR-063, AGENTS.md), mas não havia regra de que a implementação depende de consultar a documentação oficial daquela versão. O resultado seria código escrito por memória de API, sem fonte verificável. |
| **Decisão** | (1) Usar **somente versões latest estáveis**, com versão resolvida, fonte, data e digest quando for imagem registrados. (2) **Documentação oficial da versão resolvida é pré-requisito de implementação**: nenhum pod ou worker implementa com base em memória de API; se a documentação não estiver acessível, o item é declarado "não verificado" e a API não é presumida. (3) Atualização de dependência exige nova resolução de versão e nova evidência. (4) O CTO orquestra — decisão e gate são dele; a execução é dos pods e workers. |
| **Consequências** | Ganho: API usada tem fonte datada, e não memória de modelo. Risco: `latest` pode trazer **breaking change** entre resoluções e quebrar código que passou ontem. Mitigação: lockfile com versão exata e digest, mais **testes de contrato** na fronteira de cada biblioteca adotada; upgrade entra por ciclo com nova resolução e nova evidência, nunca silencioso. Custo: uma etapa de consulta antes de codar e um registro por dependência. |
| **Verificação** | (a) PR de dependência cita versão resolvida, fonte e data; imagem cita digest. (b) Item implementado sem documentação acessível aparece marcado "não verificado", sem API inventada. (c) Build reprodutível a partir do lockfile. (d) Teste de contrato falha quando a API muda de forma incompatível em upgrade de `latest`. (e) Atualização sem nova resolução/evidência é rejeitada na revisão. |
| **Escopo** | Vale para todo código de produto e infraestrutura. Não altera as decisões técnicas anteriores; apenas condiciona a execução delas a fonte documental verificável. |

## ADR-071 — Malha entre servidores: identidade de nó, QUIC mTLS e ingresso autorizado

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 18/11/2026 por instrução do usuário ("servidores como rede mesh e/ou p2p"); implementação em fatias |
| **Contexto** | A documentação proíbe ingresso anônimo (ADR-008) e trata libp2p como **candidato a validar** (ADR-007). Uma malha sem identidade de nó é só um conjunto de portas abertas. |
| **Decisão** | (1) **Identidade de nó**: par Ed25519 persistente, `node_id` derivado da chave pública; toda mensagem entre nós é assinada. (2) **Transporte**: QUIC com mTLS (RFC 9000/8446), certificado de nó autoemitido e validado por chave pública registrada — não por CA pública; QUIC v2 só se a biblioteca implementar e passar nos testes. (3) **Topologia**: malha parcial — cada nó conhece os pares autorizados do seu cluster/região e mantém heartbeat; não é broadcast global. (4) **Ingresso**: `join-request` assinado + aprovação explícita → `join_certificate` com escopos e validade; sem aprovação o nó fica `isolated` e não recebe dado privado. (5) **libp2p fica como provider alternativo futuro** (descoberta Kademlia/Gossipsub), atrás da mesma interface de transporte; não entra nesta fatia para não acoplar descoberta a ingresso. |
| **Versões resolvidas em 18/11/2026** | `quinn` 0.11.12 · `rustls` 0.23.46 · `ed25519-dalek` 3.0.0 · `rcgen` 0.14.10 · `bytes` 1.12.2. Fonte: crates.io API, consulta em 18/11/2026. `libp2p` 0.57.1 registrado como alternativa não adotada. |
| **Consequências** | Dependências novas de criptografia e QUIC (ADR-026/031 permitem: são padrões auditados, não cifra própria). Certificado autoemitido exige disciplina: validação é por chave registrada, nunca por nome. NAT/relay ainda não cobertos — nó atrás de NAT precisa de relay autorizado, que fica para fatia posterior. |
| **Verificação** | Dois nós locais: `A` sem aprovação não recebe dado privado; `A` aprovado completa handshake assinado, registra peer, mantém heartbeat; certificado de chave não registrada é rejeitado; replay de `join_certificate` expirado é recusado; queda de `B` marca peer `unknown` (não `ready`). |

### Emenda à ADR-071 (18/11/2026) — quem autoriza um servidor

**Instrução do usuário:** "os servidores principais autorizam outros servidores". Regra normativa:

1. **Autoridade de ingresso é de nó, não de token solto.** Aprovar um `join-request` exige que o aprovador seja um **nó já `authorized`** no mesmo `cluster`; um grant humano sozinho não basta — ele autoriza a *operação*, não o nó.
2. **Aprovação é assinada.** O aprovador assina `{join_request_id, node_id, public_key, cluster, scopes, not_before, expires_at, epoch}` com sua chave Ed25519. O registro guarda `approved_by_node` + `approval_signature`, verificável por qualquer nó que conheça a chave do aprovador.
3. **Cadeia de confiança verificável.** O candidato recebe o `join_certificate` assinado e o conjunto de aprovações; qualquer nó valida a assinatura contra o registro de peers autorizados — sem confiar em quem entregou.
4. **Revogação também é assinada e propagada** por nó autorizado (`revoked_by_node` + assinatura + `epoch`); revogação com `epoch` anterior não reativa acesso.
5. **Bootstrap não é confiança automática.** Os primeiros nós de um cluster (genesis) são declarados por configuração do operador (`GODMESH_FEDERATION_GENESIS`), fora de banda. A partir deles, todo ingresso vem de aprovação assinada — nada de *trust on first use*.
6. **Quórum.** Cluster com `control-voter` exige aprovação de quórum ímpar (3 ou 5) para mudanças de governança; cluster sem votantes exige **1 aprovação de nó autorizado**. O valor efetivo é configuração declarada, nunca presumida.
7. **Nó não autorizado permanece `isolated`**: pode apresentar manifesto e pedir ingresso; não recebe snapshot, eventos, nem rota de cliente.

**Efeito na implementação:** a rota de aprovação deixa de aceitar apenas o escopo `federation:admin` e passa a exigir **identidade de nó autorizado + assinatura de aprovação**; o escopo humano permanece como autorização secundária para o operador que dispara a operação. A fatia M1 implementa o registro; a verificação por assinatura de nó entra em M1b.

**Estado em 18/11/2026.** A M1B/I foi integrada localmente em `e060b4e` para decisão assinada em cluster explicitamente declarado sem votantes. A evidência inclui ingresso e ingest assinados e transporte QUIC/mTLS entre três instâncias locais. A M1C-B integrou a verificação e o consumo do certificado portátil e a distribuição explícita de recibos de revogação. O quórum de bundles segue na M3. [Aceite local da M1C-B](../09-equipe/evidencias/2026-11-18-m1c-ingresso-operacional.md#aceite-local-da-v12). [Evidência da M1B/I](../09-equipe/evidencias/2026-11-18-integracao-m1b.md#aceite-local-da-v13)

## ADR-072 — Adoção da rede-alvo de cinco planos

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada pelo usuário em 18/11/2026 ("vamos seguir com esse desenho") |
| **Contexto** | O desenho de rede do CTO ([rede-alvo](../../docs/09-equipe/rede-alvo-cto.md)) organiza entrada, controle, dados, mídia e borda em planos separados, com dez invariantes testáveis e ordem de implementação explícita. |
| **Decisão** | Adotar o documento como **alvo de arquitetura de rede**. Ordem de implementação vinculante: identidade de nó → ingresso assinado → transporte QUIC/mTLS → sync escopado entre nós → mídia → borda. Cada mudança normativa derivada (topologia, SLO, residência, relay) exige ADR própria; o mapa não substitui a arquitetura de referência. |
| **Consequências** | Fatias de federação passam a ser avaliadas contra os invariantes do §2 do documento (mídia fora do event bus, ingresso assinado, `unknown` não promovido, fencing no efetor etc.). Nenhuma fatia pode declarar "malha pronta" sem prova de nó não autorizado ficando isolado. |
| **Verificação** | Cada invariante precisa de teste automatizado ou consulta SQL que o comprove; o mapa é considerado cumprido apenas quando os dez invariantes tiverem evidência registrada. |
### Emenda à ADR-072 (18/11/2026) — distribuição de configuração e ordem das fatias

**Aprovação do usuário:** "perfeito, vamos acatar". Normas derivadas:

1. **Quatro namespaces com autoridades distintas**: `federation` (nossos control-voters, quórum) · `regional` (autoridade da região) · `tenant` (o tenant) · `local` (operador local, **nunca replicado**). No nó de terceiro, `federation` é somente leitura; divergência local → `degraded` com motivo e recusa das operações afetadas.
2. **Distribuição por pull assinado**: `GET /v1/federation/config` devolve bundle com `version`, `epoch`, `cluster`, `applies_at`, `entries[]`, `hash`, `signatures[]`. O receptor verifica assinatura → quórum → anti-rollback (`version` ≥ piso) → compatibilidade (`min_version`) → aplica atômico por `effective_after` → audita.
3. **Quórum obrigatório** para mudança que altera confiança (nó, raiz, piso de proteção); assinatura única com auditoria para configuração operacional não sensível. O bundle declara o quórum exigido e o nó não aceita menos.
4. **Fronteira do que nunca cruza**: chaves privadas, dados de outro tenant, segredos operacionais (credenciais de bridge, DSN, topologia interna, custo/capacidade bruta) e telemetria de nó de terceiro sem contrato.
5. **Ordem de implementação vinculante**: **M1** identidade/ingresso → **M2** transporte QUIC/mTLS → **M3** config bundle assinado (este §13) → **M4** replicação de eventos entre nós → **M5** borda/relay. Fatia fora de ordem exige justificativa registrada e não pode declarar prontidão sem os invariantes do §2 do documento de rede.

**Estado:** desenho aprovado. O [codec M3-A](../09-equipe/evidencias/2026-11-18-m3-a-bundle-assinado.md#aceite-do-codec), o [serviço durável M3-B1](../09-equipe/evidencias/2026-11-18-m3-b1-servico-duravel.md#aceite-do-serviço-durável) e o [pull e aplicação do piso M3-B2](../09-equipe/evidencias/2026-11-18-m3-b2-integracao-operacional.md#aceite-em-três-instâncias) foram integrados localmente. A M3-B2 foi validada em três instâncias. M4 e M5 seguem a ordem desta ADR.

## ADR-073 — Agente de recursos do nó: observar primeiro, agir por último

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 18/11/2026 por instrução do usuário ("monitorar e gerenciar todos os recursos da máquina: cpu, memória, redes, conexões, firewall etc."); fatias 1–2 implementáveis, fatia 3 exige autorização explícita por operação |
| **Contexto** | O servidor hoje expõe métricas de aplicação (requisições, pool, outbox). Não conhece o host: CPU, memória, pressão de cgroups, descritores, conexões, conntrack, interfaces, firewall. Sem isso, admissão por classe (SEC1) e readiness (SPEC-018) decidem no escuro. |
| **Decisão** | Três camadas, nesta ordem, com fronteira dura entre elas. **(1) Observar (read-only, sempre ligado):** coletar CPU, load, memória, swap, pressão (PSI/cgroup), disco por volume, descritores, conexões TCP por estado, conntrack, interfaces e contadores de rede. Expor em `/metrics` com prefixo `godmesh_host_*` e um resumo em `/healthz` de capacidade. **(2) Decidir (política, local e reversível):** alimentar admissão por classe e readiness com histerese; sob pressão, classes baixas recebem `503 admission_control` e **SOS mantém reserva**. Nunca "desligar tudo" para salvar métrica. **(3) Agir (privilegiado, enumerado, auditado):** só ações explicitamente listadas, invocadas por operador autorizado com escopo `host:admin`, registradas com antes/depois: ajustar limites de conexão, drenar nó, pausar publicador, reiniciar listener, ajustar `ulimit` do processo. **Mudança de firewall, rota, interface ou regra de conntrack nunca é automática**; exige ação humana autorizada, `dry-run` obrigatório, confirmação e registro; se a API do SO não permitir, o sistema orienta e não executa (ADR-046). |
| **Evidência obrigatória por leitura** | Toda métrica declara fonte (`/proc`, `/sys`, cgroup, netlink), unidade, janela e o que ela **não** prova. Em contêiner, leitura de host é a do *namespace* (cgroup), não do hipervisor — declarar isso no rótulo. |
| **Versões resolvidas em 18/11/2026** | `sysinfo` 0.40.4 (leitura portátil de CPU/memória/disco/rede) · `procfs` 0.18.0 (detalhe Linux: `/proc/net`, PSI) · `if-addrs` 0.15.0 (interfaces) · `netlink-packet-utils` 0.6.0 (se necessário para conntrack). Fonte: crates.io API, consulta em 18/11/2026. Preferir leitura direta de `/proc` onde o crate não agrega. |
| **Consequências** | Métricas de host entram no orçamento de observabilidade (cardinalidade controlada por agregar por interface/volume, nunca por conexão). Ação privilegiada cria superfície: exige capability separada, auditoria e teste negativo de "sem autorização não executa". |
| **Verificação** | Leitura: valores conferem com `top`/`free`/`ss` numa janela; `/metrics` mantém cardinalidade limitada; cgroup em contêiner lido corretamente (não host). Decisão: pressão de memória sintética rebaixa classe baixa e preserva SOS. Ação: sem `host:admin` → 403; `dry-run` devolve o que faria sem fazer; auditoria grava antes/depois; falha de privilégio retorna erro explícito, nunca sucesso silencioso. |

**Registro da implementação da camada 1 (18/11/2026):** as versões resolvidas
acima foram **confirmadas** como estáveis mais recentes no crates.io em
18/11/2026 (`sysinfo` 0.40.4, `procfs` 0.18.0, `if-addrs` 0.15.0,
`netlink-packet-utils` 0.6.0 — consulta literal em
[evidência da fatia](../09-equipe/evidencias/2026-11-18-host-telemetria.md) §2).
Da lista, apenas `sysinfo` foi introduzido, com `default-features = false,
features = ["disk"]`: espaço livre de filesystem é `statvfs(2)`, que `/proc` não
expõe, e chamar a syscall diretamente seria `unsafe`, proibido no workspace.
`procfs` e `if-addrs` **não** entraram, aplicando o critério desta decisão
("preferir leitura direta de `/proc` onde o crate não agrega valor"): os
contadores pedidos são os de `/proc/net/dev`, e os 16 arquivos lidos têm formato
estável documentado em `proc(5)`. Netlink continua fora (conntrack por arquivo).
Inodes por volume **não** são publicados: `sysinfo` 0.40.4 não expõe
`f_files`/`f_ffree`; a série fica ausente de propósito e a `HELP` diz isso —
ver a mesma evidência §6.

## ADR-074 — Superfície de uso para agentes e humanos: contrato antes de interface

| Campo | Conteúdo |
|---|---|
| **Status** | Aprovada tecnicamente pelo CTO em 18/11/2026 por instrução do usuário ("o sistema deve ser completamente desenvolvido para que agentes e humanos possam usar") |
| **Contexto** | O servidor já expõe 20 rotas com erros estáveis, mas não há contrato publicado, catálogo de erros legível por máquina, descoberta de capacidades, idempotência declarada nem CLI. Um agente que descobre a API por tentativa e erro produz chamada destrutiva por engano; um humano sem cliente reproduz `curl` de memória. |
| **Decisão** | A superfície de uso é derivada do contrato, nesta ordem. **(1) Contrato legível por máquina:** OpenAPI 3.1 gerado a partir do código (fonte única), com esquemas de erro, códigos, escopos, limites e exemplos; `GET /v1/meta` publica versão da API, versões suportadas (N/N-1, ADR-056), catálogo de rotas com escopo exigido, limites de payload/cota, catálogo de erros (`error` → HTTP → quando ocorre), e capacidades efetivas do nó (federação ligada/desligada, retenção configurada). **(2) Semântica segura para agente:** toda rota mutante aceita `Idempotency-Key`; toda rota destrutiva tem `dry_run=true` que valida e devolve o que faria sem efeito; erros trazem `error`, `detail`, `hint` acionável e `docs_url`; paginação por cursor documentado em `next_cursor`; nada de semântica implícita de retry. **(3) Ferramentas de uso:** CLI `godmesh` cobrindo as rotas (mesma autenticação, saída JSON e `--dry-run`), e servidor MCP que expõe as mesmas operações como tools com descrição, schema e escopo — nunca uma capacidade que a API não tenha. **(4) Interface humana:** web com Astryx/Tailwind (ADR-067) consumindo a mesma API; nenhuma tela com poder que a API não conceda. |
| **Consequências** | OpenAPI gerado pode divergir do comportamento real se o handler não declarar tipo/erro; teste de contrato passa a comparar `router` real contra `openapi.json` (rota, método, escopo, status de erro). `Idempotency-Key` exige tabela e retenção própria. MCP expõe superfície de execução: cada tool declara escopo e limite, e o agente não recebe escopo que o humano não tenha. CLI e MCP **não** podem contornar autorização — são clientes, não atalhos. |
| **Verificação** | Contrato: `openapi.json` gerado cobre todas as rotas do `router` (teste que falha se faltar rota ou status). Meta: `GET /v1/meta` lista exatamente as rotas registradas e os escopos exigidos. Idempotência: repetir POST com mesma chave devolve o mesmo resultado sem segundo efeito. Dry-run: não escreve linha em nenhuma tabela. MCP: tool sem escopo → erro de autorização, igual à API. Docs: quickstart executável que sobe o servidor local e faz o fluxo de 21 passos. |
| **Versões resolvidas em 18/11/2026** | `utoipa` 6.0.0 e `utoipa-axum` 0.3.0 (OpenAPI derivado do axum) · `schemars` 1.2.3 (schemas) · `clap` 4.6.8 (CLI). Fonte: crates.io API, consulta em 18/11/2026. MCP: implementação própria sobre JSON-RPC 2.0 stdio, sem SDK, para não acoplar a versão de framework. |

## ADR-075 — Aprovação por fatia da federação

**Status:** aprovada pelo usuário em 18/11/2026.

A federação pode avançar por fatias. Antes de implementar cada fatia, fechar o planejamento no escopo correspondente, concluir a revisão independente e registrar a aprovação da SPEC rastreável daquela fatia. A primeira frente é o ingresso e a confiança entre servidores, começando pela integração da M1B.

Esta decisão substitui, para a federação, a exigência anterior de aprovar todas as SPECs do produto antes de iniciar qualquer fatia. O teste de ponta a ponta deve envolver pelo menos três servidores. O escopo integral do GodMesh e a ordem da ADR-072 ficam preservados.

## ADR-076 — Prova portátil e revogação de ingresso

**Status:** direção adotada pela coordenação em 18/11/2026. O usuário pediu que a execução continuasse após a apresentação desta proposta.

A prova portátil de ingresso é o certificado v2. A assinatura do aprovador vincula identidade, chave e manifesto do candidato. Cada servidor verifica a cadeia a partir de uma raiz genesis configurada para o cluster. As aprovações v1 preservam seu uso local.

Em cluster explicitamente declarado sem votantes, um genesis do mesmo cluster ou o emissor autorizado da aprovação pode revogar aquela prova de ingresso por declaração assinada. Uma cadeia com elo revogado deixa de habilitar seus descendentes. Um descendente pode voltar a ser habilitado quando apresentar outra cadeia válida. A prova revogada permanece inválida. A verificação inclui evidência de frescor da revogação, conforme o contrato da fatia.

O quórum dos bundles de confiança segue a ADR-072. A [SPEC M1C](../08-specs/federacao/m1c-cadeia-confianca.md) detalha a emissão, o consumo e o teste em três servidores.

## ADR-077 — Relay QUIC restrito de laboratório

A ADR-072 ordena o relay depois do aceite operacional da replicação M4. O recorte M5 adota quatro processos em loopback, ingressados por M1C, para verificar alternativa de conectividade, autorização e isolamento entre dois destinos. A coordenação aplica a aprovação por fatia da ADR-075 à [SPEC M5 v1.2](../08-specs/federacao/m5-relay-laboratorio-v1.md) revisada de forma independente. O [plano da fatia](../09-equipe/evidencias/2026-11-19-m5-relay-laboratorio.md) vincula a versão e os aceites.

E e R usam um enlace QUIC com mTLS para controle da rota e transporte de datagramas opacos. O QUIC interno preserva o mTLS E↔D, com a identidade pinada de D. R aceita apenas destinos, serviços e endereços fixados no cadastro da fixture. A rota alternativa depende de evidência tipada, local, vigente e de uso único de falha direta. A tentativa anterior precisa estar recolhida e a arbitragem de promoção precisa estar resolvida.

Os guards M1C e M3, a ACL da rota e as cotas vinculam cada envio à instância admitida. O ensaio verifica revogação, saturação, encerramento e recuperação pelo checkpoint do receptor. A ADR autoriza a fixture sintética. Operação em produção, outros perfis, metas de capacidade e rotas reais dependem de decisões próprias.
