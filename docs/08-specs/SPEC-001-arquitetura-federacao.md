# SPEC-001 — Arquitetura do núcleo, federação e compatibilidade

**Versão:** DRAFT-0.6, 2026-11-17. **Estado:** DRAFT / OPEN; não aprovada nem pronta para implementação. **Autor documental:** Martin Fowler; **revisor independente designado:** Erich Gamma, revisão de DRAFT-0.4 pendente. Alan Turing consolida; Ada Lovelace mantém índice. IDs de casos futuros abaixo são locais a esta SPEC e descrevem cenários planejados, não testes executados. Observado: NOT_EXECUTED.

## Mudanças desta versão

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito coberto e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; caso de borda ADR-060 acrescentado. Nenhum caso executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)" com pelo menos um caso negativo e um de falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- ADR-056: N/N-1 obrigatório por perfil versionado, rejeição antes do efeito e sem downgrade silencioso; exceções somente em perfil publicado com janela, impacto, autoridade e rollback registrados. Regras Protobuf explícitas. O valor da janela permanece OPEN, desconhecido — decisão de produto pendente.
- ADR-057: posição canônica ordenada e rejeição de cursor sem partição ou com epoch incomparável, sem tradução.
- ADR-058: classes normativas de ACK, evidência por fronteira e proibição de inferência.
- Correção G3-03: parecer de Erich identificado como revisão de DRAFT-0.1; revisão de DRAFT-0.4 pendente.

## 1. Finalidade, alcance e precedência

Definir obrigações transversais para núcleo nativo, seleção de rota, fronteiras de integração, federação e evolução compatível. O contrato não escolhe implementação, wire definitivo, banco, topologia física nem política de risco. A arquitetura de referência e ADRs prevalecem sobre brainstorming anterior (arquitetura-referencia.md preâmbulo; decisoes.md preâmbulo). Direção arquitetural aprovada não comprova execução. `CONSOLIDACAO-CANONICA.md` registra a autorização posterior para produzir SPECs em paralelo às pendências; conclusão do planejamento e aprovação precedem implementação.

**Fontes verificadas:** `../00-produto/requisitos.md` tabela RF/RNF; `../06-planejamento/matriz-rastreabilidade.md` §Cobertura individual; `../01-arquitetura/arquitetura-referencia.md` §§Cinco planos, Modelo operacional, Transporte, Consistência, Normalização e Resiliência; `../06-planejamento/decisoes.md` ADR-006, 007, 024, 029, 033; `../01-arquitetura/estrategia-tecnologica.md` §§Zonas A–C e evolução GMP; `../01-arquitetura/transporte.md` §§Contrato e Seleção; `../01-arquitetura/replicacao.md` §§Snapshot e Autoridade; `../01-arquitetura/autoridade-escrita.md` §§1–3; `../03-integracoes/gateway-universal.md` §§Busca de rota e Avaliação; `../07-contratos/gmp-api-v1.md` §§2, 7.2–7.3; `../06-planejamento/aceites-comportamentais.md` §§RF-004, RF-024, RNF-008. Estes contratos contêm propostas e não demonstram wire ou produto existente.

**Insumos congelados na leitura:** `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-martin-fowler.md`, SHA-256 `632435acd2051458167cd6b717045302a32a65990454851762a5e21472d44e79`, §§A–C e resposta a Erich; `contribuicao-erich-gamma.md`, hash histórico observado em 16/11/2026 antes da revisão da SPEC: SHA-256 `63a7cac27d9ee9869b996432fb7ebe3e33dc72131e96b2d5d7894df0f8e5b6f9`, §§Revisão independente e rechecagem; o arquivo compartilhado foi atualizado posteriormente e não é um snapshot imutável preservado. Erich considerou F-1/F-2 respondidos documentalmente, mas schemas, frescor, validade e custo continuam abertos; essa rechecagem não revisou a presente SPEC.

**Insumos de plataforma confrontados nesta versão:** `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/parecer-macos-universal-fowler.md`, DRAFT-0.2, SHA-256 `e3b98817864412591cf50b220d433ca184206fb6a6a05bc7c2dce85daac50cdd`; `contribuicao-grady-booch.md`, SHA-256 `5f1c079fec306eca4fb0e29f06512ebfec90af81dfcea6a9f7c57b1d4079c17d`, §§Pesquisa desktop e respostas a revisões; `contribuicao-bruce-schneier.md` §§25–26 (parecer de segurança consultado junto ao recorte desktop); `contribuicao-alex-crichton.md`, SHA-256 `29c9d11ac89c132da5033776b34dfbf36e653d5033d281adc24cedc7dc298ed2`, §§Estado local/FFI e revisão. Esses arquivos são mutáveis; os hashes identificam apenas as versões lidas, não cópias imutáveis. Os pareceres declaram limites de especialização nativa, deixam suporte e testes em aberto e não aprovam APIs ou stack.

## 2. Rastreabilidade e divisão de responsabilidade

| Requisito | Subescopo tratado aqui | Interface e limite |
|---|---|---|
| RF-023 | independência nativa de bridges e isolamento de falha | SPEC-010 detalha integração e proveniência; não se confirma propriedade exclusiva na matriz. |
| RF-024, RNF-008 | negociação, evolução e falha segura; N/N-1 obrigatório por perfil versionado | Fowler integra; SPEC-003 contrato GMP, SPEC-017 firmware; valor da janela OPEN por decisão de produto. |
| RF-003, RF-004 | capability, autorização, readiness e resultado da seleção de rota | interface compartilhada; Erich integra roteador na matriz; decisões de autorização e custo são externas. |
| RF-012, RF-015, RF-016, RNF-004, RNF-005 | limites arquiteturais de ingresso, autoridade, réplica e degradação | SPEC-002/004/005/018/019 detalham semântica e operação; não representa cobertura integral. |
| RF-014, RNF-001 | fronteira IPv6/IPv4, transporte e fallback seguro | Vint e contratos de transporte detalham perfis; não presume capacidade universal. |
| RF-011, RF-020 | separação de confirmação e abrangência dos produtos | SPEC-004/006/012 detêm critérios específicos; esta SPEC não fecha jornada ou SOS. |

RF-033/prioridades é interface com experiência e conectividade, não cobertura integral nesta SPEC; ver OPEN-05. A matriz não confere propriedade operacional ao autor documental. Requisitos não listados não são declarados cobertos.

## 3. Atores, fronteiras e dados

Atores conceituais: principal autenticado, cliente autorizado, serviço de política, avaliador de capacidades, roteador, nó federado e adaptador externo. Cada operação tenant-scoped usa principal e contexto de tenant resolvidos/validados pelo servidor a partir de credencial e sessão; tenant enviado em corpo, caminho ou consulta é apenas alegação ou seletor sujeito a membership e autorização atuais. Ausência ambígua, divergência, credencial inválida ou autorização desconhecida impede efeito e não revela recurso cross-tenant. O formato de múltiplos tenants e a prova concreta pertencem à SPEC-002/003 e permanecem OPEN-01.

Entrada conceitual de resolução: operação e direção, origem/destino, restrições e contexto autenticado; avaliações independentes de capacidade, autorização e rota/readiness aplicável. Saída conceitual: rota elegível com serviço, destino, caminho, evidência e validade ou ausência de rota com motivo seguro; não é grant permanente. `POST /v1/routes/resolve` e `RouteDecision` aparecem como **proposta documental** em GMP §7.2, não endpoint implementado ou wire aprovado. O contrato de domínio determina taxonomia final e qual informação pode ser revelada.

Dados de federação incluem a posição canônica ordenada `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`; `authority_epoch`, `dispatch_epoch`, `event_sequence` e cursor são tipos distintos e não intercambiáveis. Cursor é opaco, versionado e vinculado a `(principal, escopo, filtros, perfil, retenção)`. Cursor sem partição ou com epoch incomparável é rejeitado, nunca traduzido. Snapshot referencia schema e digest conforme replicacao.md:36. Identidade e referências são distintas de prova de autorização; não expor tokens, payload privado ou localização sensível em telemetria. Origem externa mantém proveniência e projeções com perdas; nome externo não vira identidade verificada sem prova. Bridges são opcionais, isoladas e sujeitas a permissões e quotas próprias; descoberta não autoriza transmissão.

## 4. Comportamento, estados e transições

### 4.1 Seleção e fronteira nativa

Pré-condição: credencial válida, escopo e pedido avaliáveis, evidência de capacidade e política disponíveis. O roteador avalia capability suportada/verificada/operacional separadamente de `AccessDecision`, rota elegível e readiness do recurso quando exigida. Só retorna rota elegível se todos os gates requeridos passarem. Uma capability operacional não concede acesso; custo favorável não supera negativa ou estado desconhecido. Sem candidato retorna categoria segura de sem rota: autorização negada ou desconhecida, capacidade ausente/stale, caminho ausente ou dependência indisponível, conforme política de exposição. Não inventar enum ou HTTP final.

Comparar custo apenas com dados comparáveis e política aprovada; de outro modo não alegar rota ótima. Ao mudar autorização, capacidade, rota ou validade, reavaliar antes do efeito; quais eventos e janelas invalidam a decisão seguem OPEN-02. Falha de bridge afeta a rota que depende dela; serviço nativo continua quando suas próprias dependências estão íntegras. Reingresso após recuperação requer nova avaliação. Falha externa não altera identidade, ACL ou autoridade do núcleo; retry de efeito incerto segue SPEC-003/004 e não é legitimado por timeout.

Estados conceituais, não enum wire: `não avaliado` → `elegível` ou `sem rota`; mudança de evidência leva a `reavaliação necessária`; `unknown`, stale e negativa não se convertem em permissão. O estado específico e códigos finais são de responsabilidade dos contratos de domínio.

### 4.2 Federação e partição

Baseline textual de `replicacao.md:55`: para registro mutável sensível existe autoridade por tenant/partição com `home_region` e `authority_epoch`; a escrita remota é roteada à autoridade quando exige quórum. `arquitetura-referencia.md:66,74–79` prevê bloqueio de escritas críticas sem autoridade e admite eventos locais permitidos. Active-active entre regiões não implica múltiplos escritores livres no mesmo escopo.

Estados conceituais para raciocínio: autoridade comprovada; autoridade desconhecida/partição; recuperação e reconciliação; acesso novamente elegível após prova. Na transição para desconhecido, não confirmar mutação forte que exige autoridade; eventos locais permitidos não são declarados globais. Reconexão não promove automaticamente estado obsoleto: revalidar autorização e revogação antes de snapshot/replay/serviço de dados privados, e aplicar contrato aprovado de autoridade e reconciliação. Snapshot deve manter escopo, origem, schema, digest e cursor contíguo; checkpoint não ultrapassa lacuna segundo contrato de sync. Toda resposta deve declarar `ack_class` conforme a taxonomia ADR-058: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`. Cada classe exige emissor, evidência e escopo próprios; é proibido inferir classe superior por tempo, sucesso de camada inferior ou ausência de erro. `delivered_to_device` e `acknowledged_human` exigem evidência própria do destino/pessoa.

Eleição, transferência, quorum, prova, mecanismo de fencing no destino e efeito remoto não estão aprovados pela mera existência do baseline; `autoridade-escrita.md:1–3,18–28` é proposta. SPEC-004/005 e a frente de autoridade precisam definir transações, replay, retenção, restauração e limites por cópia; não inferir que ledger lógico exige armazenamento físico separado.

### 4.3 Compatibilidade e transporte

O perfil por serviço/plataforma/modalidade deve declarar versões, famílias de rede, opções implementadas e autorizadas, parâmetros e limites de compatibilidade. Preferir IPv6 comprovadamente alcançável, com IPv4 alternativo; QUIC/TLS preferencial quando suportado e TCP seguro conforme política, ou relay autenticado autorizado. Falta de caminho seguro implica indisponibilidade/unknown, não downgrade de autenticação. A negociação de transporte é independente da versão GMP; extensões desconhecidas só são ignoradas quando opcionais e sem alterar semântica crítica. Incompatibilidade crítica é recusada antes do efeito.

N/N-1 é obrigatório para GMP wire, API pública e schema persistido dentro da janela de suporte declarada por perfil versionado (protocolo, schema, firmware e cliente). O perfil declara versões, janela, depreciação, canary e rollback. Peer sem versão ou feature crítica comum é rejeitado antes de qualquer efeito, sem downgrade silencioso. Exceção só existe em perfil publicado que registre janela, impacto, autoridade que autorizou e plano de rollback. O valor da janela é desconhecido — decisão de produto pendente; nenhum outro aspecto desta regra fica aberto. Para schema Protobuf, número de campo não pode ser alterado ou reutilizado, campos desconhecidos são preservados e campo removido entra em `reserved`. Rollback incompatível/irreversível mantém componente isolado até recuperação segura; não presumir reversibilidade ou migração sem perda.

## 5. Segurança, falhas e plataformas

Isolamento tenant e finalidade são verificados antes de consultar ou persistir dados; revogação e frescor seguem SPEC-002/020. Não confundir autorização desconhecida com permissão. Restrição de cópias e residência depende de política aplicável, sem prometer expurgo fora de controle. Não fazer replay privado antes de revalidar autorização e retenção. Bridges externas não herdam alegação E2EE nativa; adaptação e perdas seguem SPEC-010/020. Logs e evidências são sanitizados segundo contrato-evidencias.md, sem credenciais ou conteúdo privado.

| Superfície | Aplicabilidade nesta SPEC | Limite específico |
|---|---|---|
| Servidor Linux | nós, serviços e federação quando habilitados | modalidade hospedada/autogerida e região parametrizável informadas pela coordenação; perfil e residência não aprovados. |
| macOS arm64/x86_64, iOS | clientes/integração quando suportados | macOS universal é direção documental, não evidência de fatias/ABI ou suporte. Não presumir full node, relay permanente, background irrestrito ou equivalência de hardware. |
| Windows e Linux desktop | clientes no escopo integral | stack e capacidades por plataforma ainda a qualificar; documentação de APIs/portais não prova backend, concessão, processo ativo ou suporte de runtime. |
| Android | cliente no escopo integral | permissões, lifecycle e meios dependem da plataforma e SPEC-015. |
| Web | cliente no escopo integral | APIs do navegador limitam transportes; não presumir sockets P2P irrestritos. |

Esta SPEC determina gates do núcleo para autorização, capacidade, elegibilidade de rota e compatibilidade; não especifica APIs nativas, sandbox, ciclo de vida, custódia local de chaves ou FFI de clientes. As permissões do sistema operacional e exceções de processo (app/helper/agent/daemon) não substituem consentimento, grant ou autorização GodMesh. A matriz desktop por operação × SO/arquitetura × API/versão, inclusive revogação, fallback e falha de backend, pertence às SPECs de plataforma/cliente; interoperabilidade local permanece condicionada a esse contrato. Para macOS, os pareceres Apple citados deixam matriz de processo efetor, assinatura, entitlement e versão mínima OPEN; para Windows/Linux, política por build/distribuição, backend, pacote e provedor permanece OPEN. Alex integra estado/FFI e Grady a pesquisa desktop; os recortes atuais não demonstram capacidade nativa especializada nem suporte validado. Isso delimita dependência entre SPECs e não introduz estado ou enum concorrente.

Para todas, capacidades, autorização e modo degradado são avaliados por operação. N/A só com justificativa contextual: por exemplo, dispositivo sem interface IP não participa diretamente da seleção IP; seu gateway é avaliado em separado. N/A não exclui produto ou plataforma inteira.

## 6. Aceites e casos planejados

Cada caso descreve pré-condição, estímulo e resultado esperado; IDs `SPEC-001-Cxx` são identificadores documentais aqui definidos, não execuções ou automação. Ambiente, versão, permissões, passo detalhado, evidência sanitizada, owner/reviewer e limites aprovados serão vinculados antes de execução conforme contrato-evidencias.md. Todos: **NOT_EXECUTED**.

| Caso | Pré-condição e estímulo | Resultado esperado |
|---|---|---|
| SPEC-001-C01 | credencial válida tenant A tenta selecionar recurso B por qualquer campo | negação sem efeito e sem divulgar existência; seletor não concede tenant. |
| SPEC-001-C02 | capability atual, autorização permitida, rota e readiness requeridos; solicitar resolução | rota elegível com referências e validade; concessão futura não é presumida. |
| SPEC-001-C03 | capability stale, grant negado/unknown, ausência de caminho ou dependência falha | sem rota e motivo seguro apropriado; nenhuma rota proibida ou efeito. |
| SPEC-001-C04 | custos não comparáveis ou política ausente | não declarar ótimo; manter distinção entre sem rota e falta de critério de otimização. |
| SPEC-001-C05 | resolver rota e em seguida revogar autorização ou expirar evidência | reavaliar antes do efeito; comportamento exato de invalidação a conciliar em OPEN-02. |
| SPEC-001-C06 | bridge autorizada desliga durante serviço nativo independente | rota externa degrada; nativo permanece elegível se suas dependências estão íntegras. |
| SPEC-001-C07 | partição sem prova de autoridade; tentativa de mutação forte e evento local permitido | mutação forte não confirmada; evento local não é anunciado globalmente. |
| SPEC-001-C08 | restore/reconexão com revogação ou cursor com lacuna | acesso privado só após revalidação; checkpoint não salta lacuna. |
| SPEC-001-C09 | IPv6 indisponível com IPv4 autorizado, ou UDP falha com TCP elegível | fallback de perfil sem perda de autenticação; se nenhuma rota segura, indisponibilidade explícita. |
| SPEC-001-C10 | peer sem versão/feature crítica comum, canary falho ou rollback inseguro | rejeitar antes do efeito, pausar progressão; isolar se recuperação segura não for demonstrada. |

Casos são critérios planejados e não substituem os de identidade, sync, firmware, produto ou operação. Aprovação futura requer cobertura por perfil aplicável, negativos, degradação e recuperação, com critérios e evidências de cada domínio.

## 7. Decisões abertas e critérios de fechamento

| ID | Questão, alternativas e responsável proposto | Condição de fechamento documental |
|---|---|---|
| OPEN-01 | Kim/Graydon: múltiplos escopos e fonte de prova; seleção explícita validada ou contexto restrito de sessão. Não permitir alegação como autoridade. | Schemas, sessão, mismatch, revogação e não enumeração conciliados entre SPEC-002/003 e esta. |
| OPEN-02 | Erich/Graydon: motivos, freshness, readiness e invalidação de rota; contrato comum com extensões ou perfis próprios com invariantes. Produto decide política de custo/risco. | Definir origem, validade, gate por operação, taxonomia segura, comparação e cenários de concorrência. |
| OPEN-03 | Leslie/Kleppmann/Michael/Kim: eleição, prova e fence, transação ou projeção recuperável, retenção e restore. | Autoridade, efeitos, cursores e provas por fronteira conciliados; risco e política de residência aprovados por autoridade apropriada. |
| OPEN-04 | Fowler/Vint/Graydon/Jack: escopo N/N-1, firmware, janela, exceção e rollback; compatibilidade uniforme ou por perfil com invariantes comuns. | Matriz e evidências propostas por perfil; qualquer redução de garantia submetida à decisão competente. |
| OPEN-05 | Lea/Vint/Radia/Nancy: classes de transporte e P0–P3, elegibilidade e desempate RF-033. | Crosswalk e política de risco revisados sem assumir prioridade de voz ou preempção; SPEC-013 e contratos vinculados. |
| OPEN-06 | Grady/Brendan/produto: escala, regiões, orçamentos, disponibilidade e responsabilidade por modalidade; Alex/Fowler/Grady integram a fronteira com clientes por plataforma. | Perfis e consequências fundamentados e aprovados; metas não apresentadas como medições. Para cada desktop: matriz operação × SO/build/arquitetura × API/versão, processo efetor, autorização/permissionamento, lifecycle, armazenamento/chaves, falhas/fallback e fonte oficial; contratos de estado/FFI conciliados sem transferir permissão do SO para o domínio. Revisão independente competente por recorte/versionamento e casos planejados negativos/recuperação; suporte continua OPEN até evidência própria. |

Próximas versões devem confrontar as SPECs especializadas após publicação e o índice de Ada, sem presumir que os nomes dos arquivos comprovem conteúdo. Critério de revisão: rastreabilidade, coerência de fronteira e cenários por perfil; Erich deve revisar o hash desta versão e registrar divergências. A aprovação depende de decisões e revisões aplicáveis. Nenhuma leitura de código, teste de produto, SQL ou infraestrutura foi executada; especificação é proposta documental, não evidência de implementação ou validação.

**Histórico de revisão:** Erich revisou DRAFT-0.1, SHA-256 `da844458cb6f913be73ff57a6e418dfe511b0f44323af4f98418e84609878024`, em `contribuicao-erich-gamma.md` §Revisão SPEC-001 DRAFT-0.1. Sem divergência material nos recortes examinados; M-1 apontou ambiguidade do hash histórico. DRAFT-0.2 esclareceu exclusivamente esse metadado; a revisão de 0.1 não equivale à revisão ou aprovação de 0.2. DRAFT-0.3 integra o limite de plataforma/cliente e a dependência documental dos pareceres desktop e estado/FFI; revisão independente desta versão pendente. Nenhuma revisão anterior equivale à aprovação desta versão.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC, propostos nesta versão. `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED`; nenhum caso foi executado, automatizado ou vinculado a teste existente. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Onde o parâmetro é OPEN, o esperado é negar ou informar desconhecido; nenhum valor numérico é inventado.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S001-056-P1 | ADR-056 | Perfil publicado declara N e N-1 para GMP wire com a mesma feature crítica | Peer N-1 negocia com peer N | Sessão aceita somente se o perfil declarar a janela: versão negociada registrada no perfil e resposta com `ack_class`; se o perfil não declarar janela, resposta `param_not_configured` informando janela desconhecida, sem aceite por padrão e sem número presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-056-N1 | ADR-056 | Janela declarada no perfil; peer oferece apenas N-2 | Tentativa de estabelecer sessão | Rejeição antes de qualquer efeito: nenhum evento, nenhuma mutação, motivo de incompatibilidade informado, sem downgrade | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-056-N2 | ADR-056 | Major compatível, feature crítica ausente no peer | Peer envia operação que exige a feature ausente | Rejeitado antes do efeito; nenhum campo opcional é promovido a equivalente da feature crítica | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-056-B1 | ADR-056 | Valor numérico da janela de suporte não decidido (produto OPEN) | Consultar se um peer N-1 específico é suportado | Resposta informa janela não declarada/desconhecida; não presume meses nem aceita por padrão | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-056-F1 | ADR-056 | Schema persistido já gravado por N; binário anterior não lê esse schema | Rollback para release anterior ao schema | Componente permanece isolado, sem servir tráfego, com estado explícito de incompatibilidade; não há registro de rollback concluído | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-057-P1 | ADR-057 | Cursor opaco do mesmo principal, escopo, filtros, perfil e retenção; prefixo contíguo existente | Retomar consumo pelo cursor | Retomada no prefixo contíguo, sem salto e sem evento duplicado; todos os seis componentes da posição canônica presentes | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-057-N1 | ADR-057 | Cursor legado gravado sem `partition` | Apresentar o cursor ao consumidor | Rejeição explícita do cursor; nenhuma tradução para `stream_id` e nenhum evento entregue | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-057-B1 | ADR-057 | Duas partições P1 e P2 do mesmo tenant/stream com `authority_epoch` e `seq` idênticos | Consumir ambas as partições | Posições tratadas como distintas; nenhuma sobrescrita e nenhuma quarentena por colisão | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-057-F1 | ADR-057 | `authority_epoch` trocado por transferência de autoridade; cursor do epoch anterior retido pelo cliente | Replay do cursor antigo | Rejeição fail-closed, nenhum evento aplicado, estado de recuperação incompleta exposto e rebootstrap autorizado exigido; `event_sequence` não é usado como substituto | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-058-P1 | ADR-058 | Comando persistido localmente, sem prova de entrega | Ler a resposta `202` e o estado na UI | `ack_class=persisted_local` e estado "não entregue"; nenhuma etapa superior é exibida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-058-N1 | ADR-058 | Resposta recebida com `ack_class=persisted_local` | Fluxo tenta renderizar "entregue" | Renderização recusada pelo contrato; campo de entrega permanece falso/desconhecido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-058-N2 | ADR-058 | Confirmação enviada sem `ack_class` | Cliente e API processam a resposta | Resposta tratada como inválida: nenhuma classe presumida e registro local `unknown` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-058-F1 | ADR-058 | Envio sem erro observável e sem recibo do destino | Tentar marcar `delivered_to_device` por ausência de erro ou por timeout | Recusado; resultado `outcome_unknown` com consulta/reconciliação obrigatória; classe não sobe | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-058-B1 | ADR-058 | Cópia com Inbox em ledger separado da projeção confirmou o prefixo | Pedido de ACK `materialized` | ACK máximo `persisted_local`; projeção permanece `pending` e não é inferida | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-059-P1 | ADR-059 | Grant offline válido para operação enumerada; limite de frescor do perfil pode estar declarado ou não | Abrir incidente SOS local e enfileirar | Com limite de frescor declarado, operação aceita localmente, item enfileirado com `ack_class=persisted_local` e nenhum efeito federado; sem limite configurado, `param_not_configured` e negativa sem efeito | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-059-N1 | ADR-059 | Grant offline válido | Pedir operação fora da lista enumerada (mutação federada, localização precisa, RF, transcrição remota) | Negado sem efeito; nenhuma rota federal criada e nenhum grant ampliado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-059-N2 | ADR-059 | Grant com `epoch` incomparável ou revogação desconhecida no destino | Executar uma das cinco operações locais permitidas | Negado com `stale_epoch` ou `revocation_state_unknown`; nenhum side effect e nenhuma marca de sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-059-B1 | ADR-059 | Limite de frescor por perfil ainda não medido (OPEN) | Operação condicional offline | Negado e informado como frescor não definido; nenhum TTL presumido ou configurado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-059-F1 | ADR-059 | Dispositivo restaurado de snapshot anterior com grant aparentemente válido | Repetir a operação condicional | `rollback_suspected`, negativa sem efeito local ou externo, orientação de revalidação; repetição mantém a negativa | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-060-P1 | ADR-060 | Mensagem de grupo entre endpoints GodMesh nativos com MLS estabelecido | Servidor transporta a mensagem | Servidor não obtém texto claro e busca/moderação de conteúdo permanecem indisponíveis; API/UI só expõem confidencialidade com rótulo/campo aprovado; sem campo aprovado (D9), resposta marca `param_not_configured`/`unknown` em vez de presumir rótulo | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-060-N1 | ADR-060 | Perna termina proteção em bridge ou em ASR com consentimento de canal | Cliente ou contrato declara E2EE para essa perna | Alegação rejeitada pela API e pela UI; `confidentiality=terminated` exibido de forma visível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-060-B1 | ADR-060 | Canal com perna nativa MLS e perna terminadora em bridge no mesmo fluxo | Consultar a classificação de confidencialidade do fluxo | Perna nativa permanece MLS e a terminadora reporta `terminated`; classificação não é agregada por canal, nenhuma perna herda rótulo da outra e derivado sem metadado cai em `terminated` | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-060-N2 | ADR-060 | Cópia de derivado sem metadado de confidencialidade | Consumidor lê a cópia | Tratada como `confidentiality=terminated` por falha fechada; ausência nunca suprida por inferência, cache ou ausência de erro | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-060-F1 | ADR-060 | Grupo MLS com membro/dispositivo a remover | Remover o membro e enviar mensagem posterior | Chaves atualizadas; removido sem acesso a mensagens futuras; derivados anteriores mantêm `terminated` e não são apagados; pedido de recuperação sem chave falha | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-P1 | ADR-061 | Serviço de domínio com banco de domínio disponível | Mutação que aloca posição e grava Outbox | Commit único; publicação assíncrona após commit uma única vez; ACK `persisted_local` | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-N1 | ADR-061 | Autoridade `stale` ou `unknown` para o escopo | Mutação forte no escopo | Bloqueada sem commit e sem `ack_class` de sucesso; estado de bloqueio explícito | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-N2 | ADR-061 | Cópia com Inbox em ledger separado | Tentativa de ACK `materialized` | Proibido; máximo `persisted_local`, com projeção em `pending` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-B1 | ADR-061 | Partição com dois líderes possíveis; escritor antigo admitido antes do fence | Escritor antigo tenta nova mutação após fence no efetor | Efetor recusa o escritor antigo; resultado anterior desconhecido é consultado; ambos os emissores ficam bloqueados em `authority_transfer_unknown` até prova | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-F1 | ADR-061 | Crash entre commit e publicação do Outbox | Retry do publicador | Mesmo evento, posição e digest publicados uma única vez; nenhum segundo evento e nenhum efeito externo duplicado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-061-F2 | ADR-061 | Restore de backup com Inbox em voo e eventos já publicados | Retomar serviço no destino restaurado | Reconciliação explícita antes de servir; operação dependente bloqueada; replay reconstrói projeção e não repete bridge, rádio, pagamento ou notificação | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-062-P1 | ADR-062 | Classe de dado com política aprovada identificada pela tupla `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)`; prazo e jurisdição podem estar ausentes | Gravar dado pessoal da classe | Com a tupla completa, persistência aceita e `policy_id/version` registrados na evidência; com prazo ou jurisdição ausentes, `param_not_configured` com negativa explícita e nenhum prazo presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-062-N1 | ADR-062 | Classe de dado sem política aprovada | Tentar gravar registro da classe | Falha fechada com erro explícito e operação degradada declarada; nada persistido além do mínimo do efeito imediato autorizado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-062-N2 | ADR-062 | Prazo ou jurisdição da classe não decididos (OPEN) | Consultar ou gravar a classe | Negar e informar política incompleta; nenhum prazo presumido e nenhum `null` tratado como infinito | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-062-F1 | ADR-062 | Prazo expirado em uma cópia específica | Acionar o descarte | Descarte executado por cópia com recibo de destino; `pending`/`unknown` não é sucesso e `delete_confirmed` só com evidência | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão identificados | Acesso comum durante o hold | Descarte suspenso somente no escopo do hold; acesso comum continua negado e o hold não reativa grant | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-P1 | ADR-063 | Release com SBOM, build reproduzível, digest registrado e assinatura válida; `generation` acima do piso | Instalação limpa do artefato | Assinatura verificada, digest idêntico ao publicado e instalação aceita | P | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-N1 | ADR-063 | Artefato adulterado ou assinatura inválida | Tentar instalar | Rejeitado na verificação; nenhum byte executado e motivo de integridade registrado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-N2 | ADR-063 | Dependência nova sem versão resolvida, fonte, data, licença, análise de vulnerabilidade e digest | Avaliar entrada da dependência no pipeline | Entrada bloqueada e release não gerado; nenhum artefato distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-B1 | ADR-063 | Piso de versão assinado pela autoridade de release vigente | Apresentar artefato com `generation` igual ou superior ao piso | Aceito; piso e `generation` registrados na verificação | B | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-F1 | ADR-063 | Release anterior com vulnerabilidade conhecida e `generation` inferior ao piso | Tentar downgrade | Rejeitado pelo piso; nenhuma instalação e nenhuma mensagem de sucesso | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-F2 | ADR-063 | Exceção de rollback autorizada e limitada no tempo | Aplicar o rollback aprovado | Aceito somente com decisão registrada, auditoria e piso rebaixado explicitamente; sem exceção, permanece rejeitado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S001-063-F3 | ADR-063 | Chave de assinatura de release comprometida e lista de revogação publicada | Cliente verifica artefato assinado pela chave revogada | Artefato rejeitado; artefato da chave reemitida aceito; revogação aplicada no cliente | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) janela numérica N/N-1 — o esperado só pode ser "informar desconhecido", porque o valor é decisão de produto; (2) limite de frescor do grant — sem medição, o esperado é negativa; (3) prazo/jurisdição de retenção — prazos não decididos; (4) prova concreta de `replicated_to_authority` — classe sem emissor, evidência e interface aprovados (D8); (5) enum/campo wire de `confidentiality` e de `ack_class` — nomes são rótulos documentais, não contrato aprovado; (6) mecanismo de fencing, inventário de efetores e prova de corte — OPEN em SPEC-004/C06; (7) janela de suporte de firmware e cliente por plataforma — OPEN em SPEC-017 e nas SPECs de cliente.

### Casos por requisito (P/N/B/F)

Uma linha por requisito listado na §2. Cada célula referencia caso já definido acima ou cria ID local descrito na própria célula. `n/a` só com motivo. Estado de todo caso: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`. Requisitos não listados na §2 não recebem caso e não são declarados cobertos.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-023 | SPEC-001-C02 | SPEC-001-C03 | SPEC-001-C06 | SPEC-001-C06 | Isolamento de falha de bridge; proveniência em SPEC-010. Nenhum caso prova propriedade exclusiva na matriz. |
| RF-024, RNF-008 | S001-056-P1 | S001-056-N1, S001-056-N2 | S001-056-B1 | S001-056-F1 | Janela N/N-1 numérica OPEN; P reescrito para `param_not_configured`. |
| RF-003 | SPEC-001-C02 | SPEC-001-C03 | SPEC-001-C04 | SPEC-001-C05 | Capability suportada/verificada/operacional avaliada separadamente de autorização. |
| RF-004 | SPEC-001-C02 | SPEC-001-C01 | SPEC-001-C04 | SPEC-001-C05 | Decisão de rota é temporária e não é grant; taxonomia pública OPEN-02. |
| RF-012 | S001-061-P1 | S001-061-N1 | S001-061-B1 | SPEC-001-C07, S001-061-F2 | Mutação forte exige autoridade; mecanismo de fence e inventário de efetores OPEN em SPEC-004/C06. |
| RF-015 | S001-057-P1 | S001-057-N1 | S001-057-B1 | S001-057-F1 | Réplica/retomada por prefixo; autoridade e recuperação integral em C06. |
| RF-016 | SPEC-001-C02 | SPEC-001-C03 | SPEC-001-C06 | SPEC-001-C09 | Degradação declarada; nunca downgrade de autenticação. |
| RNF-004 | S001-057-P1 | S001-057-N1 | S001-057-B1 | S001-057-F1, S001-061-F1 | Idempotência, duplicata e reordenação; semântica detalhada em SPEC-004. |
| RNF-005 | SPEC-001-C02 | SPEC-001-C03 | SPEC-001-C09 | SPEC-001-C08 | Disponibilidade/degradação e reingresso após recuperação com nova avaliação. |
| RF-014, RNF-001 | SPEC-001-C09 | SPEC-001-C09 (nenhuma rota segura ⇒ indisponibilidade explícita, sem downgrade) | S001-RF014-B1 — nova: IPv6 comprovadamente alcançável e IPv4 também disponível; preferência IPv6 registrada no perfil, IPv4 mantido como alternativa e nenhum bloqueio por ausência de preferência | SPEC-001-C09 | Perfis, famílias e fallback dependem de Vint/contratos de transporte; nenhum parâmetro numérico nesta SPEC. |
| RF-011 | S001-058-P1 | S001-058-N1, S001-058-N2 | S001-058-B1 | S001-058-F1 | Aqui só evidência por etapa; jornada e SOS pertencem a SPEC-006. |
| RF-020 | S001-062-P1 | S001-062-N1, S001-062-N2 | S001-062-B1 | S001-062-F1 | Abrangência por produto e modalidade hospedada/autogerida segue OPEN-06. |
| RF-033 | n/a | n/a | n/a | n/a | Interface com experiência/conectividade, não coberta: classes de transporte, P0–P3 e desempate são OPEN-05 com SPEC-013. |

Casos desta subseção são critérios documentais; nenhum foi executado, automatizado ou associado a teste existente.
