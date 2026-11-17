# SPEC-003 — GMP e API

**Versão:** DRAFT-0.5 · **Estado:** DRAFT / OPEN · **Autor:** Graydon Hoare · **Revisor independente designado:** Erich Gamma, revisão de DRAFT-0.4 pendente · **Data:** 2026-11-17.

## Mudanças desta versão

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito coberto e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; casos de borda acrescentados em ADR-059, 060, 062 e 063. Nenhum caso executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com negativo e falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- ADR-056: N/N-1 obrigatório por perfil versionado, rejeição antes do efeito e sem downgrade silencioso; regras Protobuf explícitas. Valor da janela permanece OPEN, desconhecido — decisão de produto pendente.
- ADR-057: posição canônica, cursor versionado e vinculado a principal/escopo/filtros/perfil/retenção; cursor sem partição ou com epoch incomparável é rejeitado, nunca traduzido.
- ADR-058: seis classes de ACK; toda resposta exige `ack_class` e é proibido inferir classe superior.
- G3-03: o parecer citado refere-se à revisão de DRAFT-0.1; revisão de DRAFT-0.3 pendente.

Esta SPEC é um rascunho documental parcial: não constitui aprovação, implementação, contrato executável, evidência de teste ou conclusão do planejamento. Cases abaixo são planejados; execução `NOT_EXECUTED`. A criação deste arquivo foi autorizada explicitamente pelo usuário nesta conversa. IDs de casos abaixo são locais a este rascunho, sem associação a testes executáveis existentes.

## Escopo, rastreabilidade e autoridade

- RF-006: negociação, envelopes e semântica de aplicação GMP. Fonte: `../00-produto/requisitos.md:12`, `../06-planejamento/matriz-rastreabilidade.md:92`.
- RF-019: interface HTTP, erros, versionamento e fidelidade dos SDKs; cobertura de rotas e schemas ainda parcial. Fontes: `../00-produto/requisitos.md:25`, matriz:105 e `../07-contratos/api-inicial.md:3-5,54-65`.
- RF-041: subescopo de fronteira API Wi-Fi, permissões, idempotência e evidência; domínio e estados pertencem à SPEC Wi-Fi. Fontes: `../00-produto/requisitos.md:75`, matriz:139, `../07-contratos/wifi-autodiscover-autoshare-api.md:3-13,69-90`.
- RF-004: interface proposta de resolução de rota, sem assumir responsabilidade por algoritmo ou política de custo. Fontes: requisitos:10; matriz:90; `../07-contratos/gmp-api-v1.md:399-409,441`.

As ADRs 004, 009–011, 021, 029–030 e 033 registram direções ou decisões no índice `../06-planejamento/decisoes.md:10-17,27,35-39`; uma decisão aprovada é intenção, não prova de produto (linha 3). Arquitetura e ADRs prevalecem em conflitos segundo `../README.md:97-104`. As regras detalhadas C07/C09/C10 são propostas e suas decisões permanecem abertas. Identidade/autorização pertence a Kim/SPEC-002; eventos/sync a Martin Kleppmann/SPEC-004; autoridade a Leslie; retenção a C08/SPEC-005 e SPEC-020; Wi-Fi a Jouni/SPEC-009; esta SPEC não cria contratos concorrentes.

Fontes de contribuição: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-graydon-hoare.md`, SHA-256 `bec6399ae881d1dd7bd42d7191d599842ed46aab867d34440efbac4ea8e189f1` na leitura prévia; parecer de Erich `contribuicao-erich-gamma.md:35-60` refere-se à revisão de DRAFT-0.1, hash `1012461e4983db62dbaad6812f3f2cbaf9f508dd22bc34d4757228a248596643`. Revisão de DRAFT-0.3 pendente. `contribuicoes/revisao-proposta-b06-graydon.md` documenta lacunas de namespace, identidade, cursor, handoff e ACK. A versão desta SPEC deve ser revisada por seu próprio hash.

## Atores, plataformas e fronteiras

Principal humano, workload, nó ou dispositivo age conforme credenciais e políticas aplicáveis; elegibilidade por operação permanece dependente de C09. O serviço deriva principal e tenant do contexto autenticado; seleção enviada pelo cliente é pedido validado, não concessão. Não autorizar com base em tenant do body. Validar scope, ACL, finalidade, consentimento e autoridade quando pertinentes, sem revelar recursos de outro tenant. Estado de revogação indisponível impede efeito protegido quando a política exige confirmação.

O servidor é planejado para contêiner Linux. macOS arm64/x86_64, iOS, Windows, Linux desktop, Android e web são consumidores quando a operação e o perfil forem aplicáveis; sua inclusão não comprova SDK, transporte, permissões do SO ou compatibilidade. Wi-Fi depende das APIs permitidas por plataforma e controlador. Nenhuma plataforma é globalmente N/A; disponibilidade específica é OPEN por operação e versão. Não se impõe cliente full node a aplicações móveis.

## Contrato GMP e negociação — RF-006

Fontes: `../07-contratos/gmp-api-v1.md:7-14,24-43,87-117,123-140,230-234`. Transporte e versão GMP são camadas diferentes. N/N-1 é obrigatório para GMP wire, API pública e schema persistido dentro da janela declarada por perfil versionado. Com sessão autenticada, perfil publicado e recursos compatíveis, o servidor negocia somente versões/features admissíveis antes de aceitar mensagens da aplicação. Sem versão ou feature crítica comum, rejeita antes do efeito; downgrade silencioso é proibido. Exceção somente por perfil publicado que registre janela, impacto, autoridade autorizadora e plano de rollback. O valor da janela é desconhecido — decisão de produto pendente. Para schema Protobuf, número de campo é imutável e não reutilizável, campos desconhecidos são preservados e campo removido entra em `reserved`. Limites de bytes, concorrência e prazo definidos por perfil versionado permanecem OPEN.

Envelope de controle proposto inclui versão, ID e tipo de mensagem, contexto autenticado, prazo e identificação lógica da operação quando aplicável. Um envelope de evento tem identidade, origem, escopo, stream, sequência, epoch quando aplicável, perfil e digest conforme C10/C07; não é envelope de comando. Validar framing, presença, escopo e limites antes de efetuar operação. Epoch de autoridade não é epoch de dispatch; nenhum deles substitui autorização. Os schemas e tags finais dependem de revisão, e placeholders não autorizam payload arbitrário.

## API, erros e operação idempotente — RF-019

Fontes: `gmp-api-v1.md:268-304,306-360,371-395,421-451,477-502`; `eventos-sincronizacao.md:49-87`. Em mutação, autenticar e autorizar, validar schema e precondições, obter política eficaz e resolver a chave de operação no namespace pertinente antes de produzir efeito. A proposta C07 associa chave a representação canônica versionada; SHA-256/TLV e seus vetores são candidatos, não interoperação comprovada. Mesmo pedido e chave, sob autorização atual, retornam operação/resultado conhecido; alteração semântica sob a mesma chave gera conflito e nenhum novo efeito. Credenciais, trace, tentativa e metadados locais de dispatch não mudam fingerprint segundo proposta C07; destino semântico e payload podem mudá-la. Política ausente ou desconhecida impede garantia afetada.

Toda resposta da API carrega `ack_class`, com exatamente uma classe declarada para o estágio comprovado: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`. Cada classe exige emissor, evidência e escopo próprios; é proibido inferir classe superior por tempo, sucesso de camada inferior ou ausência de erro. `delivered_to_device` e `acknowledged_human` exigem evidência própria do destino/pessoa. `202` representa aceitação para processamento conforme `ack_class`; não implica persistência, entrega, conclusão, reconhecimento humano ou estado SOS. `Command.lifecycle`, `DeliveryAttempt` e incidente permanecem dimensões diferentes. `outcome_unknown` exige consulta/reconciliação; não repetir efeito sem prova de dedupe ou resultado. Expiração impede novo efeito, mas não inventa resultado nem apaga necessidade de reconciliação. Retry com IDs preservados só ocorre dentro da política, autorização e prazo aplicáveis; parâmetros de retry não estão escolhidos.

Erros candidatos são mapeados por operação sem revelar dados não autorizados: erro sintático/schema, autenticação, autorização, conflito de idempotência/epoch, indisponibilidade de autoridade ou retenção, e cursor inválido/expirado. A lista específica, distinção 403/404 e correspondência HTTP/GMP dependem de C09 e aprovação; não inferir um código para todas as operações. SDK não converte erro ou `202` em sucesso de entrega.

### Operações ilustrativas e cobertura

| Operação proposta | Entrada e autoridade | Saída/limite | Lacuna |
|---|---|---|---|
| `POST /v1/devices/{device_id}/commands` | Chave idempotente, tipo, payload, expiração e versão/autoridade quando aplicável; ACL e capability | `202 CommandReceipt` com `ack_class`; consulta posterior autorizada | Escopos e schemas finais, retenção e política de efeito |
| `POST /v1/routes/resolve` | Serviço, origem, destino, restrições; principal/tenant autenticados e autorização | Decisão de rota elegível com evidência e restrições, ou sem rota com causa sanitizada | Taxonomia pública, frescor e comparação de custo sob política aprovada |
| `GET /v1/events/stream` | Cursor opaco versionado e filtro dentro do escopo autorizado | Eventos autorizados; resposta declara `ack_class`, confirmação Inbox limitada ao prefixo durável conforme C10 | Framing e prova por destino |

Essas rotas são propostas documentais, não endpoints existentes. A matriz completa de métodos, schemas, autorização e negativos por operação ainda não foi integrada; RF-019 não está integralmente coberto. A análise das 34 operações em `Shared/GodMesh/Planejamento-Integral-2026-11-16/contribuicoes/semantica-api-graydon.md` precisa de reconciliação com C09 e versões posteriores.

## Cursor, sync e ACK — interface com SPEC-004

Posição canônica de evento: `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`, com campos nesta ordem. `authority_epoch`, `dispatch_epoch`, `event_sequence` e cursor são tipos distintos. Cursor é opaco, versionado e vinculado a `(principal, escopo, filtros, perfil, retenção)`, além do vetor de checkpoints contíguos conforme `eventos-sincronizacao.md:91-105,130-155` e C07:30,39-43,379. Cursor sem partição ou cujo epoch seja incomparável é rejeitado, nunca traduzido. Lacuna não avança prefixo; cursor de outro contexto não amplia acesso. Cursor além de retenção exige recuperação autorizada e snapshot verificável, se disponível; não saltar eventos nem afirmar completude. Toda resposta declara `ack_class`; confirmação Inbox corresponde a `persisted_local` somente quando a cópia/prefixo foi efetivamente persistida, sem implicar projeção, entrega final, descarte ou reconhecimento humano.

OPEN: B06/B12 precisam definir versões do mapeamento, compatibilidade de tokens, provas de corte/fencing e replay histórico, cobertura do digest e identificação autenticada do destino para classes que dependem dele. Não interpretar `ack_class` de uma cópia como confirmação de outra. Martin/Leslie/Michael são responsáveis pela semântica e prova; Graydon mapeia representação wire com Kim. Sem essas definições não declarar interoperabilidade nem fechamento de handoff.

## Wi-Fi — interface RF-041

Registrar AP não autoriza divulgar posição exata; exige prova e opt-in específicos. Estado `requested`, associação e `internet_verified` são distintos. Origem, método, instante, validade e revogação de evidência devem ser definidos por Jouni e Kim; sem prova vigente, não afirmar internet verificada. Revogação com controlador inacessível não prova remoção remota. Autorização, idempotência e retenção por cópia seguem contratos respectivos; não escolher prazo ou comportamento de SO ainda desconhecido.

## Critérios de aceite e casos planejados

| Caso | Pré-condição e estímulo | Resultado planejado e invariante |
|---|---|---|
| 003-A | Pares autenticados oferecem major compatível e recursos exigidos | Negociar apenas interseção autorizada antes de aceitar operação. |
| 003-B | Sem major comum ou feature crítica desconhecida | Rejeitar sem efeito e sem downgrade silencioso. |
| 003-C | Principal de outro tenant tenta ler ou mutar recurso | Negar ou ocultar segundo política aplicável; não produzir efeito nem vazar existência. |
| 003-D | Mesmo pedido/chave reapresentado após perda de resposta | Retornar mesma operação conhecida; sem segundo efeito, após revalidar autorização. |
| 003-E | Mesma chave com payload semanticamente diferente | Conflito; nenhuma alteração adicional. |
| 003-F | Tentativa anterior com resultado remoto desconhecido e prazo expirado | Não iniciar outro efeito; permitir consulta/reconciliação autorizada sem fabricar resultado. |
| 003-G | Sequência posterior chega antes da anterior | Checkpoint permanece no prefixo durável anterior; ACK não comprova projeção ou pessoa. |
| 003-H | Cursor de outro principal ou fora da retenção | Recusar; oferecer recuperação apenas quando autorizada e disponível, sem salto silencioso. |
| 003-I | Rota exige capability com evidência vencida ou readiness ausente | Não declarar rota elegível; apresentar separadamente a evidência de capability, quando aplicável, e informar motivo permitido; sem política e dados comparáveis não alegar ótimo. |
| 003-J | Cadastro AP sem opt-in, associação sem prova de internet ou controlador offline após revogação | Não publicar coordenada exata, não afirmar internet e não afirmar remoção remota, respectivamente. |

Para S3-2, ficam planejados, condicionados à definição por Jouni/Kim do método, origem, instante, validade e revogação: prova atribuível válida pode sustentar `internet_verified` somente quando o predicado aprovado e as demais permissões forem satisfeitos; prova expirada ou revogada não sustenta estado positivo. Os casos de plataforma e seus campos seguem OPEN até os contratos correspondentes estarem definidos.

Casos descrevem expectativas documentais; IDs executáveis, ambiente, ferramentas, limites, fixtures, resultados observados e evidências permanecem NOT_ASSIGNED/NOT_EXECUTED. A cobertura integral exige casos por operação, plataforma e modo de falha após conciliação de domínio.

## Decisões OPEN e fechamento

1. **Versão OpenAPI e cadeia SDK** — Graydon/Fowler: OAS 3.2.1 é recomendação C07, não decisão; alternativas schema-first com ferramentas comprovadas ou outro perfil explicitamente aprovado. Fechar com versão escolhida, compatibilidade de gerador/validador e demonstração futura, sem downgrade silencioso.
2. **Scopes, workload/pessoa, anti-enumeração e revogação** — Kim/Graydon: reconciliar matriz por operação, prova e erros; fechar quando fonte de autoridade e negativos forem consistentes entre C07/C09.
3. **Namespace, digest, cursor, classes ACK e handoff** — Martin/Leslie/Michael com Graydon: ADR-057 fixa posição/cursor canônicos e rejeição; ADR-058 fixa classes e `ack_class`. Permanecem OPEN mapeamentos wire, provas, migração e revisão de segurança, sem promessa de exactly-once.
4. **Janela de compatibilidade** — Produto: definir valor numérico da janela e perfis incluídos no v1; desconhecido — decisão de produto pendente.
5. **Wi-Fi e retenção** — Jouni/Kim/Michael: decidir método e validade da evidência, efeitos da revogação e cópias; fechar com transições e casos por plataforma elegível.
6. **Limites e operação** — Fowler/Graydon/Brendan: aprovar limites e matriz operacional por perfil; não inferir desempenho de uma especificação.
7. **Cobertura e revisão** — Graydon/Erich/Ada: completar catálogo de rotas e schemas, vínculo requisito e casos, revisar esta versão por hash e resolver divergências; aprovação pertence ao processo autorizado.

Nenhuma dependência foi instalada, nenhum produto, SQL ou teste foi executado. Fontes, pareceres e estados acima são evidência documental, não prova de funcionamento.
## Revisão independente e delta 0.2

Erich revisou DRAFT-0.1, SHA-256 `84f3c25845723d07d270dd523486cc025a4f9a35cc66a7052bcfa4e03675dba9`, em `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-erich-gamma.md:86-96`. S3-1 ajusta 003-I para distinguir elegibilidade de rota da capability operacional. S3-2 adiciona cenários condicionais de evidência válida, expirada e revogada; predicados e plataformas permanecem OPEN com Jouni/Kim. Essa revisão foi de DRAFT-0.1; revisão de DRAFT-0.3 pendente. Nenhuma aprovação ou teste é inferido.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC. `Estado` sempre `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`; nenhuma rota, SDK ou teste existe. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN produz negativa ou informação de desconhecido, nunca número inventado. As rotas citadas continuam propostas documentais.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S003-056-P1 | ADR-056 | Sessão autenticada; perfil publicado declara N e N-1; peer N-1 oferece major e features críticas comuns | Estabelecer sessão e enviar operação | Negociação aceita apenas na interseção e somente se o perfil declarar a janela; versão negociada registrada e resposta com `ack_class`; sem janela declarada, `param_not_configured` com motivo sanitizado e nenhum aceite por padrão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-056-N1 | ADR-056 | Sem major comum entre os pares | Enviar mensagem de aplicação | Rejeição antes do efeito; nenhum `202`, nenhum evento e nenhum downgrade silencioso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-056-N2 | ADR-056 | Major compatível, feature crítica desconhecida pelo destino | Enviar operação que depende da feature | Rejeitado antes do efeito; nenhum campo desconhecido tratado como equivalente da feature crítica | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-056-B1 | ADR-056 | Envelope de evento com campo Protobuf novo e campo removido marcado `reserved` | Receber e reenviar o evento | Campo desconhecido preservado sem alteração de semântica; número reservado nunca reutilizado ou renumerado; tentativa de reutilizar número é rejeitada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-056-F1 | ADR-056 | Schema persistido já gravado pela versão N; SDK em N-2 tenta reconectar | Chamar endpoint após rollback do cliente | Rejeitado com estado explícito de incompatibilidade; nenhum endpoint converte o registro, nenhum campo é reinterpretado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-056-F2 | ADR-056 | Janela numérica de suporte não decidida (produto OPEN) | Consultar suporte de uma versão específica | Resposta informa janela não declarada; sem aceite por padrão e sem número presumido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-057-P1 | ADR-057 | Cursor opaco versionado do mesmo principal, escopo, filtros, perfil e retenção; versão do cursor e janela de retenção podem não estar declaradas | `GET /v1/events/stream` com o cursor | Com versão e retenção declaradas, eventos autorizados a partir do prefixo contíguo e resposta com `ack_class`; sem versão de cursor ou janela de retenção declarada, `param_not_configured` recusando a leitura sem número presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-057-N1 | ADR-057 | Cursor sem `partition` ou montado a partir de timestamp/`seq` | Apresentar o cursor | Rejeitado como cursor inválido; nenhuma tradução e nenhum evento entregue | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-057-N2 | ADR-057 | Cursor emitido para outro principal ou escopo | Reutilizar o cursor em sessão própria | Rejeitado; nenhum acesso ampliado e nenhuma revelação de existência de eventos | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-057-B1 | ADR-057 | Duas partições com mesmo `authority_epoch` e `seq`; cursor de uma delas | Consumir e comparar posições | Posições distintas preservadas; nenhuma colisão aceita, nenhuma quarentena indevida da partição correta | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-057-F1 | ADR-057 | Cursor de epoch anterior após troca de autoridade; snapshot disponível e autorizado | Reapresentar o cursor | Rejeição fail-closed; oferta de rebootstrap/snapshot autorizado; quando snapshot ou prova faltar, recuperação declarada incompleta, sem saltar eventos | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-058-P1 | ADR-058 | `POST /v1/devices/{device_id}/commands` aceito e persistido, sem prova de entrega; nome wire de `ack_class` pode não estar aprovado | Ler o `202 CommandReceipt` | Com o campo aprovado, `ack_class=persisted_local` e `Command.lifecycle` separado, sem indicação de entrega, conclusão ou atendimento; sem campo/nome aprovado (D8), resposta reporta `param_not_configured` e estado `unknown`, sem classe presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-058-N1 | ADR-058 | Resposta de confirmação sem `ack_class` | Cliente e SDK processam | Resposta tratada como inválida; nenhuma classe presumida e registro `unknown` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-058-N2 | ADR-058 | SDK recebe `202` e posterior timeout | SDK converte para "entregue" | Proibido; SDK mantém `ack_class` e devolve `outcome_unknown` com instrução de consulta | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-058-F1 | ADR-058 | Mutação aceita com chave de idempotência; resposta perdida | Reenviar o mesmo pedido | Mesma operação e mesmo estado retornados; `ack_class` não sobe por reenvio; nenhum segundo efeito | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-058-B1 | ADR-058 | Cópia com Inbox em ledger separado confirmou o prefixo | Cliente tenta observar `ack_class=materialized` | ACK máximo `persisted_local`; projeção reportada `pending` | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-059-P1 | ADR-059 | Grant offline válido para operação enumerada; limite de frescor pode estar declarado ou não | Chamar operação local permitida pela API | Com frescor declarado, aceita localmente com `ack_class=persisted_local` e nenhum efeito federado; sem limite configurado, `param_not_configured` com negativa e nenhum TTL presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-059-N1 | ADR-059 | Grant offline válido | Chamar operação fora da lista enumerada (mutação federada, localização precisa, RF, transcrição remota) | Negada com `insufficient_scope` ou `authority_unavailable`; nenhum efeito e nenhum recurso revelado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-059-N2 | ADR-059 | Grant `stale`, epoch incomparável ou revogação desconhecida | Chamar operação condicional | Negada com `unknown`/`stale_epoch`; nenhum efeito, nenhuma marca de sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-059-B1 | ADR-059 | Limite de frescor da operação offline não declarado (OPEN) e grant offline válido | Chamar operação enumerada com chave de idempotência nova | Negada com `param_not_configured` informando frescor não definido; nenhum efeito, nenhum item persistido e nenhum TTL presumido | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-059-F1 | ADR-059 | Grant válido quanto à assinatura e já restaurado de snapshot | Reapresentar com chave de idempotência nova | Negado com `rollback_suspected`; nenhum efeito e nenhuma operação criada; repetição mantém a negativa | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-060-P1 | ADR-060 | Conteúdo de grupo protegido por MLS; resposta de leitura autorizada | Consultar evento ou comando relacionado | Resposta sem texto claro e busca/moderação indisponíveis; confidencialidade só é exposta com campo/rótulo aprovado; sem campo aprovado (D9), `param_not_configured`/`unknown` em vez de rótulo presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-060-N1 | ADR-060 | Perna terminada por bridge, ASR com consentimento ou gravação | Cliente envia ou consulta alegação de E2EE para a perna | Alegação rejeitada; `confidentiality=terminated` retornado e exibido; TLS/DTLS/SRTP não aceitos como prova de E2EE | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-060-N2 | ADR-060 | Derivado persistido sem metadado de confidencialidade | Ler o derivado pela API | Resposta assume `confidentiality=terminated` por falha fechada; nenhum E2EE inferido de cache ou ausência de erro | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-060-B1 | ADR-060 | Operação declara `confidentiality` no perfil, mas o campo/rótulo não está aprovado no contrato (D9) | Consultar evento de canal MLS | Resposta marca `param_not_configured`/`unknown` para o campo não aprovado e mantém ausência de texto claro; nenhum rótulo de E2EE é inferido de ausência de metadado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-060-F1 | ADR-060 | Consentimento de ASR do canal revogado durante processamento | Enviar novo segmento e consultar resultado | Novo segmento recusado por política; derivados anteriores mantêm `terminated`; segmento em voo reportado como interrompido ou desconhecido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-061-P1 | ADR-061 | Serviço com banco de domínio e outbox na mesma transação | Mutação idempotente | Commit único, publicação após commit, retry retorna o mesmo resultado conhecido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-061-N1 | ADR-061 | Autoridade `stale` ou `unknown` para o escopo | Mutação forte | Bloqueada sem efeito e sem `ack_class` de sucesso; motivo de indisponibilidade de autoridade | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-061-N2 | ADR-061 | Inbox em ledger separado da projeção | Resposta tenta declarar `materialized` | Recusado pelo contrato; máximo `persisted_local` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-061-B1 | ADR-061 | Tentativa anterior com resultado remoto desconhecido; prazo da operação expirado | Repetir a chamada | Nenhum novo efeito iniciado; consulta/reconciliação autorizada disponível; resultado não fabricado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-061-F1 | ADR-061 | Crash entre commit e publicação | Reenviar o mesmo pedido com a mesma chave | Mesma operação, posição e digest publicados uma vez; nenhum efeito externo duplicado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-062-P1 | ADR-062 | Classe de dado com política aprovada e `policy_id/version` vigente; prazo ou jurisdição podem faltar na política | Operação que persiste dado pessoal | Com política completa, aceita e `policy_id/version` registrado na evidência e no recibo; com prazo ou jurisdição ausentes, `param_not_configured` com recusa antes do efeito e sem valor presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-062-N1 | ADR-062 | Classe de dado sem política aprovada | Operação que persistiria o dado | Falha fechada com erro explícito de política ausente; nada persistido além do mínimo do efeito imediato | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-062-N2 | ADR-062 | Prazo de retenção do cursor/evento não decidido (OPEN) | Leitura além da retenção declarada | Recusada com instrução de recuperação autorizada; nenhum prazo presumido e nenhum salto silencioso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-062-B1 | ADR-062 | Política aprovada com prazo definido e jurisdição não definida para a classe | Persistir dado pessoal da classe e consultar retenção | Persistência condicionada: `param_not_configured` para jurisdição não definida, com recusa explícita do que dependa dela e nenhum prazo ou base legal presumidos | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-062-F1 | ADR-062 | Expurgo solicitado por classe; uma cópia é exportação fora de controle | Consultar resultado do expurgo | Recibo por destino; `out_of_control` reportado como limite de controle, sem declarar conclusão global | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-P1 | ADR-063 | SDK distribuído com SBOM, digest registrado e assinatura válida; `generation` acima do piso | Instalar o SDK | Com piso assinado disponível, assinatura verificada e instalação aceita com `generation` registrada; se o piso não estiver publicado, `param_not_configured` bloqueando a instalação | P | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-N1 | ADR-063 | Artefato de SDK adulterado | Instalar | Rejeitado na verificação; nenhum código executado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-N2 | ADR-063 | Dependência de geração de cliente OAS sem versão resolvida, fonte, licença, digest e análise de vulnerabilidade | Gerar o SDK | Geração bloqueada; nenhum artefato publicado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-B1 | ADR-063 | Piso de versão assinado pela autoridade de release vigente; SDK atual com `generation` igual ao piso | Instalar o SDK | Aceito; piso e `generation` registrados na verificação, sem rebaixamento implícito | B | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-F1 | ADR-063 | Release anterior que aceita downgrade silencioso, com `generation` inferior ao piso | Tentar instalar | Rejeitado pelo piso; instalação existente preservada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S003-063-F2 | ADR-063 | Rollback de SDK autorizado com decisão registrada | Aplicar o rollback | Aceito somente com auditoria e piso rebaixado explicitamente; sem exceção, permanece rejeitado | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) valor da janela N/N-1 — só "informar desconhecido"; (2) limites de bytes, concorrência e prazo por perfil; (3) versão OpenAPI e cadeia de SDK (OAS 3.2.1 é recomendação, não decisão); (4) nomes wire de `ack_class` para `replicated_to_authority` (D8) e o campo `confidentiality` (D9); (5) taxonomia pública de erros e distinção 403/404 (C09); (6) parâmetros de retry e expiração de chave de idempotência; (7) prova e identificação autenticada do destino para classes que dependem dele.

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. Cada célula referencia caso já existente ou cria ID local descrito na própria célula; `n/a` só com motivo. Estado de todo caso: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-006 | S003-056-P1 | 003-B, S003-056-N1, S003-056-N2 | S003-056-B1 | S003-056-F1, S003-056-F2 | Negociação e envelopes GMP; janela numérica OPEN, esperado reescrito com `param_not_configured`. |
| RF-019 | S003-058-P1 | 003-C, S003-058-N1 | S003-058-B1 | 003-F, S003-058-F1 | Cobertura de rotas e schemas permanece parcial (OPEN 7); SDK não converte `202` em entrega. |
| RF-041 | S003-RF041-P1 — nova: prova atribuível válida mais predicado aprovado sustentam `internet_verified`; sem predicado aprovado, `param_not_configured` e estado permanece não verificado | 003-J | S003-RF041-B1 — nova: prova expirada ou revogada em cache controlado não sustenta estado positivo nem prova remoção remota | 003-J (controlador inacessível após revogação) | Subescopo de fronteira API; método, origem, validade e revogação OPEN com Jouni/Kim. |
| RF-004 | S003-RF004-P1 — nova: capability atual, autorização permitida e readiness satisfeitos produzem decisão de rota elegível com evidência e `ack_class` | 003-I | n/a — variante de borda (custos não comparáveis, política ausente) já está contida em 003-I, coluna Negativo | n/a — motivo: invalidação e frescor de rota pertencem a SPEC-001 C05 e OPEN-02, sem caso próprio aqui | Interface de resolução; não assume algoritmo nem política de custo. |

Casos desta subseção são critérios documentais; nenhum foi executado, automatizado ou associado a teste, SDK ou endpoint existente.
