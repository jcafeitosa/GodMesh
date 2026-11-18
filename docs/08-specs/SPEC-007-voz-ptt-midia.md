# SPEC-007 — Voz PTT e mídia nativa

**Versão:** DRAFT-0.9 · **Estado:** DRAFT/OPEN · **Data:** 2026-11-17  
**Autora documental:** Radia Perlman · **Revisor designado:** Vint Cerf  
Pareceres de Vint existentes referem-se aos DRAFT-0.1 e DRAFT-0.2; revisão independente desta versão pendente. Execução: `NOT_EXECUTED`. Metas quantitativas: **OPEN — a medir**.

**Requisitos cobertos nesta versão:** RF-005 (voz nativa, PTT, grupos, prioridade e canais); RNF-007 (mídia não transportada pelo event bus; medições planejadas de jitter, perda, reconexão e floor control).  
**Cobertura:** parcial. A SPEC não pretende especificar toda a voz, transporte, bridges, ASR, SOS, QoS ou experiência de plataforma. Casos de teste ainda `NOT_ASSIGNED`; execução `NOT_EXECUTED`.


## Superfície nativa exigida (ADR-068)

O cliente é web empacotado em Tauri 2; a voz não depende de API web em segundo plano. Contratos obrigatórios dos plugins nativos:

| Necessidade | Plugin/superfície | Regra |
|---|---|---|
| Captura e reprodução de áudio com app em segundo plano | plugin nativo de áudio por plataforma (iOS: sessão de áudio + modo de background declarado; Android: serviço em primeiro plano com tipo `microphone`/`mediaPlayback`) | TX sem sessão de áudio ativa e comprovada é negada; a UI mostra `audio_session=unavailable` |
| Roteamento de dispositivo (fone, Bluetooth, alto-falante) | API do SO via plugin | troca de rota durante TX gera evento e reavaliação de floor; não reinicia áudio silenciosamente |
| Interrupção do SO (chamada, alarme, outro app) | evento de interrupção do plugin | TX é encerrado e o evento é registrado; retomada exige novo pedido de floor |
| Tecla física / push-to-talk externo | plugin de atalho/tecla | opcional por plataforma; ausência é declarada, não simulada |
| Foco de áudio e Duck/Ducking | API do SO via plugin | política declarada por canal; nunca presumir |

Nenhuma tela chama API de áudio do navegador para PTT: `getUserMedia` só é aceito no cliente web sem shell, e nesse caso o PTT é declarado **limitado a primeiro plano**. Teste negativo: tentar TX em segundo plano sem plugin → negado com estado observável, sem buffer fantasma.

## Mudanças desta versão (ADR-068)

- Voz deixa de depender de "cliente nativo SwiftUI/Android" e passa a exigir **plugin nativo de áudio** no cliente web+Tauri; PTT em segundo plano é capacidade declarada, não presumida.
- O que não muda: floor/lease (ADR-058/007-A…C), `confidentiality` (ADR-060), mídia fora do event bus (RNF-007), retenção de áudio (ADR-065, não retido por padrão).

## Mudanças desta versão

### DRAFT-0.9 — distinção de TX offline e estado stale/unknown

Reescreve §4.4 e §6.3 para separar indisponibilidade de rota de autoridade `stale/unknown`: TX isolado só pode ocorrer se uma autorização assinada ainda válida explicitar esse escopo e operação; `stale/unknown` segue negando, sem inferência pelo tempo. Frescor, duração, propagação de revogação e operação local efetivamente autorizada continuam OPEN-007-C. Alinha o caso de borda/negativo C007-059-P/N/F/B: enquanto frescor não estiver configurado, negar ou adiar TX offline. Nenhum tempo ou permissão nova foi decidido.

### DRAFT-0.5 — casos por requisito e casos P desbloqueados (C3-1)

Acrescenta a subseção `### Casos por requisito (P/N/B/F)` ao final, com uma linha por requisito declarado (RF-005, RNF-007) e as quatro colunas preenchidas ou `n/a` justificado, incluindo casos novos de separação bus/mídia descritos em uma linha. Reescreve os casos P cujo esperado dependia de valor não decidido — janela N/N-1 do perfil, frescor/lease do grant offline e prazo da política de gravação — para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID. Acrescenta o caso de borda `C007-061-B` (ADR-061), que faltava. Nenhum codec, prazo ou limite é criado; nada é aprovado, implementado ou executado.

### DRAFT-0.4 — casos por decisão (resposta ao veto C3-1)

O crítico C3 vetou as SPECs por ausência de casos vinculados às decisões ADR-056…063. Esta versão acrescenta a seção “Casos por decisão (ADR-056…063)”, com uma linha por decisão aplicável a esta SPEC, cobrindo ao menos um caso negativo e um de falha/recuperação por decisão. Todo esperado é observável (código, estado ou campo); todo caso permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Parâmetro `OPEN` produz comportamento de negar/informar desconhecido, nunca número inventado. Nenhuma decisão é alterada e nada é aprovado ou executado por este delta.

### ADR-058 — Evidência por fronteira

Toda resposta de API carrega `ack_class`. Cada confirmação identifica emissor, evidência e escopo. API e UI só podem usar as seis classes abaixo. Tempo decorrido, ausência de erro ou sucesso em outra fronteira nunca promovem uma classe. Se a confirmação se perder, o resultado da etapa fica `unknown`.

| Classe | Emissor | Evidência exigida | Escopo confirmado |
|---|---|---|---|
| `transport_accepted` | Transporte receptor | Recibo da tentativa de envio | Aceitação naquele salto |
| `persisted_local` | Dispositivo ou nó que gravou | Registro de persistência do objeto | Objeto e versão naquele armazenamento |
| `replicated_to_authority` | Autoridade de destino | Confirmação da réplica pela autoridade | Objeto e versão naquela autoridade |
| `materialized` | Responsável pela projeção | Aplicação observada do evento à projeção | Projeção e versão identificadas |
| `delivered_to_device` | Dispositivo destinatário | Recibo do próprio dispositivo vinculado ao objeto | Entrega àquele dispositivo |
| `acknowledged_human` | Pessoa autenticada e autorizada | Ação explícita atribuível à pessoa e vinculada ao objeto | Reconhecimento daquela pessoa, para aquele objeto |

Aceitar o transporte de um pedido de release não comprova cessação de TX no edge.

### ADR-060 — Voz nativa, terminação e identidade

Na voz nativa, E2EE é o padrão entre endpoints GodMesh. As chaves de sessão SRTP derivam do contexto MLS. TLS, DTLS ou SRTP isoladamente não demonstram E2EE. Toda perna em que bridge, ASR ou gravação termina a proteção expõe `confidentiality=terminated` na API e na UI. A transcrição de uma perna com bridge terminadora conserva essa indicação e não pode alegar confidencialidade ponta a ponta.

ASR requer consentimento explícito por canal. Antes de expor áudio ao servidor, o usuário deve ver que habilitar ASR retira E2EE daquele canal. A ratificação de produto para ASR no servidor continua pendente. Somente prova de identidade vinculada à sessão permite atribuir fala a uma pessoa. Na sua ausência, registrar apenas a origem verificável.

## 1. Fontes, precedência e rastreabilidade

Fontes normativas consultadas em `docs/`:

- `00-produto/requisitos.md:11` (RF-005) e `:43` (RNF-007).
- `01-arquitetura/midia-nativa.md`: componentes, transporte, TalkStart, fluxo PTT, invariantes, capacidade, criptografia, APIs, aceitação, ASR e SOS.
- `01-arquitetura/protocolo-nativo.md`, seção “Dados, sincronização e escalabilidade”: NATS/Gossipsub não transportam cada pacote de voz.
- `03-integracoes/bridges-voz.md`: independência/isolamento de bridge, política RX/TX e prevenção de loops.
- `06-planejamento/decisoes.md`: ADR-014/021/022/028/031/032/053. Arquitetura de referência e ADRs prevalecem conforme `README.md`, § “Regras de prevalência”. ADR-053 regula prioridade de recursos no app e não decide floor RF.
- `06-planejamento/pendencias.md:22-23,30-33,130-131`: interoperabilidade, cenários nativos e decisões em aberto de prioridade/floor.
- `06-planejamento/matriz-rastreabilidade.md:7,13,63,91,119` e `aceites-comportamentais.md:RF-005, RNF-007` (ver também seção RNF-007 localizada em `:5612-5678`).

**Contribuições lidas, com hash SHA-256 do arquivo observado nesta consolidação (16/11/2026):** `SPECS-EM-ANDAMENTO/contribuicao-radia-perlman.md` — `01fd6a8e938f793633690df4c84eaf9d6443b2f6f3d68ad904b30d955a426bbc`; `contribuicao-vint-cerf.md` — `5d2bd70f7c7693d0b4025c6866d8c11a4223f3b389edc429be21a1fd466a9112`. O manifesto de Ada registra para a contribuição Radia um snapshot-base hash `01fd6a8e…5a426bbc`; o hash calculado coincide com esse snapshot. O parecer de Vint (`contribuicao-vint-cerf.md:37`) recomenda que RNF-007 proíba qualquer payload de áudio no bus, não somente RTP/Opus, e mantenha metadados/event bus e autoridade a conciliar.

**Limite de evidência:** fontes descrevem arquitetura e contratos planejados. Não confirmam existência/execução das APIs, implementações, codec, interoperabilidade, suporte por SO, conformidade regulatória, métricas nem capacidade operacional. Não foi feita validação externa ou de campo.

**Revisão independente recebida:** Vint Cerf revisou DRAFT-0.1 SHA-256 `ff498a0619c755770f77e4f347649b813605d83f8a2f70fd5dbc378e09a42db4`. Não encontrou defeito bloqueador no recorte de transporte/mídia. Confirmou RNF-007 e separação controle/mídia, mantendo validade/fencing temporal, escopo do bus/metadados, falha/partição e perfil quantitativo como OPENs materiais. Parecer em `SPECS-EM-ANDAMENTO/contribuicao-vint-cerf.md`, seção “Revisão de SPEC-007 DRAFT-0.1 (Radia)”. Esta revisão não é aprovação da SPEC.

## 2. Propósito e fronteiras

Especificar comportamentos mínimos do PTT half-duplex nativo e separar o plano de controle GMP do caminho de mídia. O caminho obrigatório GodMesh↔GodMesh deve funcionar com Mumble/FRN desligados. Bridges são opcionais, isoladas e fora da autoridade nativa de canal. A mídia não é transportada por event bus persistente nem por NATS/Gossipsub pacote a pacote.

Fora do escopo desta versão: contrato GMP wire completo; seleção/implementação de WebRTC versus outro transporte nativo; ICE/STUN/TURN, NAT/relay e handover; codec profile/packetização; limites quantitativos de QoS/capacidade; detalhes de implementação de E2EE; bridge SDK completo; política de transcrição; SOS/ACK; certificação/licenciamento RF; APIs específicas de cada plataforma. Esses tópicos são dependências, não decisões implícitas.

## 3. Atores, domínio de autoridade e dados

- **Cliente PTT:** usuário/dispositivo autenticado que solicita e, sob grant válido, transmite ou recebe.
- **Autoridade de voz/floor:** serviço que autentica sessão, aplica ACL e política, arbitra floor e emite/revoga grants. Topologia/localidade e failover ainda OPEN.
- **Media edge:** valida independentemente a autorização e o epoch antes de aceitar frames; recebe/distribui mídia no caminho de mídia.
- **Media router:** posiciona salas/sessões e apoia reconexão conforme capacidade/saúde; algoritmo/limites OPEN.
- **Listener:** identidade autorizada a receber mídia do canal.
- **Bridge host:** autoridade separada e opcional para integração externa; não herda autoridade de canal nem converte prioridade interna em autorização RF.

Dados conceituais citados pelas fontes: canal/sessão, identidade e ACL, classe/prioridade decidida por política, `grant_id`, epoch, expiração/TTL, razão, `stream_id` efêmero, codec negociado e sequência de frames. Os nomes `RequestFloor`, `ReleaseFloor`, `RevokeFloor`, `CreateMediaSession`, `ResumeMediaSession` são APIs conceituais existentes apenas na documentação; assinatura, wire, erros e disponibilidade não estão comprovados.

`TalkStart` associa GMC ou alias autorizado, sessão, stream efêmero e grant. O frame carrega referência curta, sequência e codec, sem PII ou UUID permanente. Em WebRTC, SSRC/track → GMC depende de sinalização autenticada. A semântica final de identificadores e envelopes é dependência da SPEC GMP/identidade.

## 4. Contratos e invariantes

1. Em canal half-duplex, pode haver no máximo um grant TX válido por domínio de arbitragem. Grant identifica canal/escopo, titular, `grant_id`, epoch e validade; o formato e a autoridade que define validade seguem OPEN.
2. O media edge valida grant, titular, canal/escopo, epoch e validade antes de aceitar TX. Controle não delega essa checagem ao cliente.
3. Mudança de autoridade exige fencing. Se não houver prova de invalidação do grant anterior no controlador e no ponto de mídia, preempção deve ser negada ou adiada. Fencing distribuído e seus efetores não estão definidos por esta SPEC.
4. Perda de controle impede TX externo novo, exceto operação offline previamente autorizada por grant assinado, ainda válido e explicitamente delimitado para aquele TX/canal; essa condição permanece sujeita a OPEN-007-C. Indisponibilidade de rota, por si só, não torna válido um grant. Se autoridade estiver `stale/unknown`, se escopo TX local não estiver explícito, ou se validade/frescor não puder ser demonstrada, TX é negado ou adiado; nenhum estado é promovido por tempo decorrido (`pendencias.md:126,128`). Não presumir propagação instantânea de revogação. O tratamento de frames já em trânsito e a cessação temporal no edge isolado permanecem OPEN.
5. Release/revoke torna o grant inutilizável conforme política e remove a autorização efetiva no edge. Recibo de transporte do release (`transport_accepted`) e confirmação de cessação são sinais distintos; protocolo/ordem ainda OPEN.
6. Reconexão não ressuscita grant expirado, revogado ou epoch antigo. Criar sessão/stream atual ou revalidar sessão conforme política e exigir autorização vigente antes de novo TX.
7. O controle e a voz podem degradar separadamente. Eventos e SOS não dependem do codec; estado do codec não cria autoridade de floor.
8. Nenhum payload de áudio/mídia, em qualquer codec, representação ou encapsulamento, pode ser serializado, publicado ou persistido como payload no event bus. RTP, Opus e AudioFrame são exemplos, não lista exaustiva. A fronteira exata do bus e a classificação de metadados permanecem OPEN. Eventos de controle/timeline só contêm metadados explicitamente contratados; nunca frames.
9. Mídia usa caminho próprio. Falha do bus não deve transferir áudio para bus, log ou fila persistente. Se a mídia pode continuar quando o bus isolado falha depende de contrato de autoridade e sessão vigente; não presumir.
10. Nenhum payload de voz é convertido em áudio sobre endpoint de dados sem capacidade (p.ex. LoRaWAN/BLE data-only).
11. Bridges não podem afetar o núcleo nativo quando desligadas; mídia transcodificada pode ficar legível na bridge. Não declarar E2EE entre redes externas.

## 5. Estados e transições propostas

Estados de controle propostos (nomes provisórios; não são enumeração GMP aprovada):

`idle` → `request_pending` → `granted` → `transmitting` → `releasing` → `idle`.

Saídas negativas/degradação: `denied`, `expired`, `revoked`, `control_unavailable`, `media_unavailable` e `reconnect_pending`. Após falha de mídia, a recuperação volta a `reconnect_pending`; sessão/stream atuais e autorização são revalidados antes de TX. Estado terminal e publicação na timeline dependem do contrato de eventos/UX.

Transições normativas que a implementação futura deve preservar: pedido somente após autenticação/canal acessível; grant somente após ACL/política; TX somente se grant válido e edge validar; prioridade maior não preempta sem política e fencing comprovado; release/revoke/expiry bloqueiam novos frames de TX; controle ou autorização `unknown/stale` não são sucesso. Detalhes temporais permanecem OPEN.

## 6. Fluxos comportamentais e cenários

### 6.1 PTT nativo normal

**Pré-condição:** dois ou mais clientes autenticados, ACLs e canal definidos, serviços/rota de controle e mídia elegíveis, transporte negociado, estado inicial sem concessão conflitante.  
**Estímulo:** cliente solicita floor; autoridade decide; cliente inicia `TalkStart` e frames; listener autorizado recebe; transmissor libera.  
**Esperado:** autoridade retorna grant autenticado com epoch/validade; edge aceita somente o titular/escopo válidos; listener autorizado recebe fluxo identificado; release invalida TX de acordo com política; caminho opera com bridges desligadas.  
**Negativo:** sem ACL/canal elegível, grant não é emitido; chamada com grant ausente/expirado/revogado/epoch antigo é rejeitada no edge.

### 6.2 Concorrência, prioridade e preempção

**Pré-condição:** dois solicitantes com políticas/ACLs/classes explícitas no fixture futuro.  
**Estímulo:** pedidos simultâneos, mesma prioridade, prioridade diferente, duplicata/reordenação; repetir com grant ativo.  
**Esperado:** decisão e desempate observáveis, no máximo um TX utilizável por domínio. Preempção somente se política explícita permitir e se o grant anterior for cercado antes da nova autorização ser utilizável no edge.  
**Negativo/degradação:** prioridade desconhecida, empate não resolvido ou fence não comprovado não resulta em segundo grant utilizável; devolver resultado/motivo sem declarar precedência universal. Se a autoridade ficar `stale/unknown`, não converter grant antigo em sucesso; grant offline só vale para ações e limites previamente aprovados.

### 6.3 Release, expiração, revogação e partição

**Estímulo:** release normal; expiração; revoke; isolamento separado de cliente, controlador e media edge; failover durante grant; mensagens atrasadas/reordenadas.  
**Esperado:** estado antigo não autoriza novo TX; autoridade não alcançável não cria concessão conflitante; edge nega grant stale conforme regra vigente. TX offline somente se grant ainda válido e explicitamente delimitado para esse escopo e a autoridade não estiver stale/unknown; se frescor, permissão ou epoch não puderem ser demonstrados, negar/adiar. Ao reconectar, sessão e grant revalidam.  
**OPEN:** duração/validade/frescor, revogação propagada durante partição, autoridade local permitida e tratamento/cessação de áudio já em trânsito. Não afirmar corte instantâneo sem critério aprovado.

### 6.4 Separação bus/mídia — RNF-007

**Pré-condição:** ambiente futuro permite observar media path, controle GMP e publicação/persistência do event bus independentemente.  
**Estímulo:** mídia e controle/timeline simultâneos; tentar inserir amostras sintéticas de payload de áudio no bus; interromper bus, controle e media path separadamente; incluir mídia com codecs/representações diversos quando estes forem definidos.  
**Esperado:** bus não contém payload de áudio, nem como RTP/Opus/AudioFrame nem outras representações; metadados autorizados permanecem distinguíveis; nenhum fallback persiste áudio. Falha de bus não migra áudio para bus/log. Continuação da mídia requer grant, controle e path vigentes conforme contrato aprovado.  
**Negativo:** publicações de áudio no bus são impedidas na fronteira definida; pedido por endpoint sem capacidade de mídia é negado.

### 6.5 Jitter, perda, reconexão e failover

Planejar ensaios futuros com topologia, plataforma/transporte/versão, população, duração/janela, método, carga, perdas de coleta e eventos de rota declarados. Introduzir jitter/perda controlados, pausa/queda do caminho, troca de rota/edge e retorno; cruzar com grant/floor ativo e falha de controle. Coletar medições reais de latência/jitter/perda e experiência percebida sem limiares até aprovação de perfil. Reconexão exige stream/sessão atual e revalidação. Não prometer troca inaudível nem replay de frames antigos.

## 7. Aplicabilidade por plataforma

| Plataforma | Aplicabilidade nesta SPEC | Limites/validação requerida |
|---|---|---|
| Servidor Linux | Controle, media edge/router conforme modalidade | Deployment/capacidade, fencing e observabilidade não verificados |
| macOS arm64/x86_64 | Cliente web+Tauri com plugin nativo de áudio (ADR-068); transporte/versionamento a selecionar | WebView WKWebView; áudio em segundo plano depende do plugin; não verificado |
| iOS | Cliente web+Tauri com plugin nativo de áudio e sessão de áudio de background (ADR-068) | WKWebView; entitlements de background exigem prova oficial futura |
| Windows desktop | Cliente de voz se incluído no produto/rota suportada | Tecnologia de UI/transportes e capacidades não decididas/validadas |
| Linux desktop | Cliente de voz se incluído no produto/rota suportada | Stack cliente, permissões e mídia não decididas/validadas |
| Android | Cliente web+Tauri com plugin nativo de áudio e serviço em primeiro plano (ADR-068) | Android WebView; tipo de serviço e suspensão exigem prova oficial/futura |
| Web | Cliente PTT; arquitetura aponta WebRTC/Opus | Compatibilidade, signaling, navegador/SO, permissões, ICE/STUN/TURN e interoperabilidade não comprovados |

Todas as plataformas listadas mantêm aplicabilidade ao contrato de controle/autorização quando oferecem voz. N/A de capacidade ou serviço deve ser decidido por produto/perfil; não se exclui plataforma implicitamente.

## 8. Segurança e privacidade

Aplicar ACL e escopo no controle e no edge; grant é de validade limitada e associado a epoch; identidade pública/GMC é indicação, não credencial autossuficiente. Minimizar metadados, evitar UUID/PII em frames e registrar somente evidência necessária. Áudio/gravação exigem permissões e consentimento quando aplicável, sob política de retenção própria. Bridge com transcodificação pode acessar áudio claro. Transporte protegido (TLS/DTLS conforme transporte descrito) não comprova E2EE. Licenciamento/autorização de TX RF é gate externo e por jurisdição; prioridade de software não concede direito de transmissão.

## 9. Critérios de aceite planejados e evidência

Critérios documentais para futuros testes autorizados:

- PTT entre clientes autorizados estabelece voz GodMesh sem bridge; desligar Mumble/FRN não interrompe fluxo nativo.
- Um grant/epoch válido por domínio half-duplex; media edge aceita somente o titular e rejeita autorização inválida.
- Release, expiry, revoke, mudança de autoridade e perda de frescor não permitem novo TX inválido; reconexão requer sessão/stream atuais e grant revalidado.
- Testes concorrentes/preempção confirmam que nova concessão só se torna utilizável após fencing confirmado no edge.
- Inspeção do bus comprova ausência de payload de áudio em todas as formas exercitadas; falha do bus não persiste nem migra áudio.
- Jitter/perda/floor/reconnect são medidos com método/população/janela e integridade do coletor registrados. Sem limiar aprovado, não classificar qualidade como PASS.
- Estados de erro, unknown/stale e recuperação são observáveis sem alegar entrega/recepção de áudio sem evidência do destino.

Evidência planejada inclui configuração/perfil, versão de contratos/cliente/servidor/transporte, ACL e consentimentos sintéticos, rota/topologia, IDs de sessão/stream/grant e epoch, decisões/fencing, observação do edge, contagem e classificação de payload no bus, medições e perdas de coleta, resultados por cliente e limpeza. Não coletar conteúdo de voz real para provar separação.

### Crosswalk requisito → seção → aceite → caso planejado

| Requisito | Seções desta SPEC | Aceite planejado | Cenários planejados | ID de caso / observado |
|---|---|---|---|---|
| RF-005 | §§2–7, 9–11 | §9: PTT nativo sem bridge; titular com grant/epoch válido é o único TX; edge bloqueia grant inválido; release/revoke/expiry/reconexão sem reuso obsoleto | §§6.1–6.3, 6.5: normal; concorrência/preempção; expiração/revogação/partição; jitter/perda/reconexão/failover | `NOT_ASSIGNED` / `NOT_EXECUTED` |
| RNF-007 | §§2, 4 (itens 8–10), 6.4–6.5, 9–10 | §9: nenhum payload de áudio no bus em qualquer codec/representação; falha não migra/persiste mídia; jitter/perda/floor/reconnect medidos sem PASS por threshold inexistente | §§6.4–6.5: inspeção mídia/bus, tentativas sintéticas de payload, falhas isoladas, transporte/jitter/perda e reconexão | `NOT_ASSIGNED` / `NOT_EXECUTED` |

Casos concretos seguem `NOT_ASSIGNED` até definição de ambientes e IDs no índice central. Execução `NOT_EXECUTED`.

## 10. Decisões OPEN e condições de fechamento

| OPEN | Dono proposto | Alternativas / consequência | Evidência/condição de fechamento |
|---|---|---|---|
| OPEN-007-A — domínio e autoridade do floor, failover e concorrência | Alan/arquitetura; Leslie para semântica distribuída; Radia interface mídia | autoridade regional/local ou serviço dedicado; consequência para disponibilidade, fencing e operação sob partição | decisão de autoridade e efetores/escopo conciliada com SPECs de identidade/eventos; cenário sem grant duplo revisto |
| OPEN-007-B — classes, precedência, empate, fila, política de preempção | usuário decide política; produto/voz documentam | permitir só classe superior autorizada, nunca preemptar, ou habilitar por perfil; sem matriz, desempate não pode ser inferido | matriz por perfil, autoridades, feedback e consequência acordados; teste futuro planejado; gate RF preservado |
| OPEN-007-C — lease, renovação, clock/frescor, revogação no edge isolado | identidade/autorização e arquitetura; Radia interface de voz | expiração conservadora ou lease limitado a autoridade local previamente autorizada; risco de disponibilidade versus grant stale | semântica temporal e de fencing por camada e partição explícita, sem prazo inventado; compatibilidade com grants limitados offline conciliada |
| OPEN-007-D — independência de mídia durante falha do event bus e definição de bus | Martin Kleppmann + Radia | media path pode continuar com controle vigente ou cessar se bus/autoridade compartilhada for requisito; risco distinto para disponibilidade e stale authority | definição do bus (inclui efêmero/durável), autoridade observável e regra de continuidade/revogação aprovadas; inspeção futura planejada |
| OPEN-007-E — metadados permitidos no event/timeline | Martin Kleppmann + produto/privacidade; Radia para classificação de mídia | allowlist mínima de eventos/metadados ou contrato mais amplo com classificação; risco de payload de mídia ou exposição de dados | schema/classificação aprovada e casos de tentativa de payload bloqueado sem tratar dado arbitrário como permitido |
| OPEN-007-F — codec, transporte, packetização e budgets | Vint lidera transporte; Radia mídia; produto/SRE aprovam budgets | WebRTC para mais clientes vs transportes nativos versionados; sem benchmarks não declarar superioridade ou interoperabilidade | escolha/negociação documentada, suporte demonstrado por plataforma e perfil de QoS com valores aprovados e fontes |
| OPEN-007-G — fronteira com SOS e direito de TX RF | Nancy, operador e usuário; Lawrence consulta jurisdição | prioridade de fala pode ser pedido sem garantir transmissão; bridge TX bloqueada sem autorização | contrato não confunde prioridade, grant, RF, entrega e atendimento; autorização competente por jurisdição antes de TX externo |
| OPEN-007-H — bridge e elegibilidade de RX/TX | Gregor; operadores/autorizadores | bridge desligada ou ativa por endpoint/perfil; risco de loop, transcodificação clara e autorização externa | SPEC-010 conciliada; teste de isolamento e opt-in fica planejado até endpoint/permissão reais |
| OPEN-007-I — critérios de UX/lifecycle por plataforma | Lea; Donny/Chet; Radia contrato | parar TX no background/suspensão ou política explícita por plataforma/capability; não presumir background permitido | critérios por SO/versão e APIs oficiais consultadas quando escolhidas; evidência de suspensão/reconexão futura |
| OPEN-007-J — metas quantitativas jitter/perda/latência/espera/capacidade | produto + SRE + arquitetura | sem metas: somente caracterização medida; budgets aprovados permitem comparação por perfil | população, janela, método, limites e consequência aprovados; benchmark reproduzível posterior, sem preencher números por analogia |

## 11. Dependências e estado de aprovação

- SPEC-002 identidade/autorização e SPEC-003 GMP: autenticação, grants, epochs, erros, compatibilidade e revogação.
- SPEC-004 eventos/sync: semântica do event bus, timeline, evento persistente e autoridade por partição.
- SPEC-001 arquitetura/federação e decisões de Alan/Leslie: domínio de autoridade, failover e roteamento.
- SPEC-010 integrações/bridges: autorização RX/TX, isolamento, transcodificação e prevenção de loops.
- SPEC-006 SOS e SPEC-008 ASR: prioridade/ACK separadas e transcrição sem degradar PTT.
- SPECs de plataformas e operação: lifecycle, permissões, carga, observabilidade e evidência.
- Decisões de produto e RF/jurisdição continuam pertencendo aos responsáveis competentes/usuário.

**Estado final desta versão:** DRAFT-0.3, OPEN, cobertura parcial dos requisitos explicitamente listados. A revisão de Vint está vinculada a DRAFT-0.1; revisão desta nova versão/hash solicitada. Revisões posteriores necessárias após dependências. Sem código, SPEC aprovada, capacidade medida, interoperabilidade comprovada, ensaios, conformidade regulatória ou validação de produto.

## Casos por decisão (ADR-056…063)

Casos planejados, sem ID formal: `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED` em todas as linhas. `Tipo`: **P** positivo, **N** negativo, **B** borda, **F** falha/recuperação. Cada decisão aplicável tem ao menos um caso N e um F. Onde o parâmetro é `OPEN`, o esperado é negar ou informar desconhecido — nunca um número, prazo, codec ou código inventado. Estes casos complementam §9 e não a substituem.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| C007-056-P | ADR-056 | Cliente e autoridade de voz com perfis N e N-1; **janela de suporte não declarada** no perfil (`OPEN — decisão de produto`) | Estabelecer sessão de controle e solicitar floor | Esperado reformulado com parâmetro ausente: sessão recusada com `param_not_configured` e perfil exibido como desconhecido; nenhum `grant_id` emitido e nenhum frame aceito. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-056-N | ADR-056 | Peer cujo perfil não possui a feature crítica usada no grant (`authority_epoch`/fencing) | Solicitar floor contra esse peer | Rejeição antes de qualquer efeito: nenhum `grant_id` emitido, nenhum frame aceito no media edge; incompatibilidade declarada, sem downgrade silencioso | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-056-B | ADR-056 | Evento de controle gravado por versão nova com campo adicional; leitor N-1 | Ler e regravar o evento no leitor N-1 | Campo desconhecido preservado; número de campo não reutilizado; campo removido aparece como `reserved`, nunca reciclado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-056-F | ADR-056 | Perfil de protocolo/mídia sem janela N/N-1 declarada (`OPEN — decisão de produto`) | Iniciar sessão entre versões distintas | Sessão recusada com indicação de perfil não suportado e estado `control_unavailable`; nenhuma sessão “experimental” aceita por omissão | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-057-P | ADR-057 | Timeline de voz com eventos de controle em partição conhecida e `authority_epoch` vigente | Consultar a timeline com cursor versionado da tupla completa | Eventos daquela partição/epoch devolvidos; cursor com versão declarada e vínculo a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-057-N | ADR-057 | Cursor de timeline construído sem `partition` | Apresentar o cursor à leitura | Cursor rejeitado com erro explícito; nenhum evento devolvido; nenhuma tradução para outra forma | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-057-B | ADR-057 | Dois canais distintos com eventos de mesma `seq` | Consultar ambas as partições | Eventos não colapsam; cada um retorna com sua partição; nenhuma deduplicação indevida entre canais | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-057-F | ADR-057 | Cursor emitido antes de failover da autoridade de floor, com `authority_epoch` alterado | Reapresentar o cursor após o failover | Rejeição fechada por epoch incomparável; nenhuma página servida; `dispatch_epoch`/`event_sequence` não tratados como o mesmo tipo | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-058-P | ADR-058 | `ReleaseFloor` submetido e aceito pela camada de transporte | Consolidar o envio do release | Resposta com `ack_class=transport_accepted` e evidência daquele salto; UI mantém separado o estado de cessação efetiva no edge | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-058-N | ADR-058 | Release com `transport_accepted` apenas | Exigir prova de cessação de TX no edge | UI não promove: exibe “release aceito no transporte” e “cessação não confirmada” em campos distintos; nenhum texto de TX encerrado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-058-B | ADR-058 | Resposta de `RequestFloor` sem `ack_class` | Consumir a resposta | Resposta tratada como inválida; estado da etapa `unknown`; nenhum grant exibido como concedido (nome do código `OPEN` em SPEC-003) | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-058-F | ADR-058 | Recibo de grant/release perdido por queda de conexão após envio | Reconectar e consultar o recibo | Estado `unknown`; cliente não presume grant nem cessação; nova tentativa do mesmo efeito com `grant_id` idêntico, sem segundo efeito | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-059-P | ADR-059 | Rede local, grant de floor delimitado, `epoch` comparável, sem revogação conhecida e contador anti-rollback íntegro; **frescor/lease não decidido** (`OPEN-007-C`) | Transmitir PTT offline | Esperado reformulado com parâmetro ausente: a avaliação de frescor é reportada como `param_not_configured` e o TX fica negado ou adiado, sem validade presumida; nenhum efeito federado e nenhum segundo transmissor. Com o lease decidido, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-059-N | ADR-059 | Sem grant de floor válido ou fora da rede local autorizada | Solicitar TX offline | TX negado; nenhum frame enviado; motivo `denied` registrado, sem fallback para transmissão sem grant | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-059-N2 | ADR-059 | Grant de floor com `epoch` anterior ao observado, ou revogação conhecida chegando após o snapshot | Transmitir com esse grant | TX negado no edge antes do primeiro frame; grant não reutilizado; estado `revoked`/fresco desconhecido exibido | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-059-F | ADR-059 | Relógio do dispositivo incerto ou limite de frescor do perfil não fixado (`OPEN`) | Solicitar TX offline | TX negado por frescor indeterminado, com motivo explícito; nenhuma validade inventada; PTT nativo com controle disponível não é afetado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-059-B | ADR-059 | Autoridade de floor em estado `stale`/`unknown` durante isolamento | Pedir floor offline | Pedido não convertido em sucesso: estado `control_unavailable`; grant antigo não promovido; nenhum segundo transmissor criado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-060-P | ADR-060 | Dois endpoints GodMesh nativos, sem bridge, sem ASR e sem gravação | Estabelecer sessão de voz | Chaves de sessão derivadas do contexto MLS registradas como base; API e UI não declaram terminação; nenhuma alegação além do que o perfil comprova | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-060-N | ADR-060 | Bridge habilitada com transcodificação de áudio | Consultar estado da sessão e da bridge na API e na UI | `confidentiality=terminated` presente com o terminador identificado; proibido exibir “E2EE ponta a ponta”; áudio legível na bridge declarado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-060-N2 | ADR-060 | ASR em servidor com consentimento explícito por canal | Habilitar ASR no canal | Canal marcado `confidentiality=terminated`; aviso de retirada de E2EE exibido antes de expor o áudio; consentimento registrado por canal | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-060-B | ADR-060 | Evento/mídia sem metadado de confidencialidade | Consumir o artefato | Artefato tratado como `terminated` (falha fechada); ausência declarada; nenhuma suposição de E2EE por TLS/DTLS/SRTP | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-060-F | ADR-060 | Remoção de membro/dispositivo durante a sessão | Remover um listener autorizado | Chaves atualizadas de modo que o removido não decifre conteúdo futuro; estado da sessão registra a mudança; nenhuma entrega de mídia futura a ele | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-061-P | ADR-061 | Serviço de controle com transação única e `outbox` para eventos de timeline | Registrar `TalkStart`/`Release` na timeline | Evento de timeline persistido e publicado após commit; nenhum payload de áudio no outbox; `event_id` único | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-061-N | ADR-061 | Outbox com evento de timeline pendente e bridge configurada | Executar replay do outbox | Projeção reconstruída sem reemitir comando à bridge, rádio ou notificação; nenhuma transmissão externa repetida | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-061-N2 | ADR-061 | Media edge recebendo em Inbox/ledger separado da projeção | Consolidar a recepção do evento no edge | ACK máximo `persisted_local`; `materialized` não emitido para essa etapa; UI não exibe timeline aplicada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-061-B | ADR-061 | Dois registros do mesmo `event_id` no outbox de timeline (duplicata por retry do publicador) | Executar replay do outbox | Uma única publicação observada; duplicata descartada pela chave idempotente e registrada; nenhum `TalkStart`/`Release` repetido na timeline e nenhum payload de áudio no outbox | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-061-F | ADR-061 | Crash entre o commit do evento de controle e sua publicação | Reiniciar o serviço de controle | Evento recuperado e publicado uma única vez; áudio não migra para bus, log ou fila persistente; estado `stale`/`unknown` bloqueia novo grant forte até reconciliação | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-062-P | ADR-062 | Política com classe, cópia e finalidade declaradas para gravação de áudio, com prazo 0 por padrão — áudio não retido; gravação opt-in 30 dias (ADR-065) | Gravar uma sessão autorizada | Esperado reformulado com parâmetro ausente: gravação aceita com a política identificada e `retention_policy_ref` presente; prazo e gatilho reportados como `param_not_configured`, sem expiração automática anunciada; consulta restrita conforme ACL e `confidentiality` coerente | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-062-N | ADR-062 | Nenhuma política aprovada para a classe de áudio/gravação daquela cópia | Iniciar gravação | Persistência negada com erro explícito; nenhum áudio gravado nem replicado; PTT continua funcionando (invariante 1 e §2) | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-062-B | ADR-062 | `legal hold` solicitado sobre gravação existente | Aplicar hold sem autoridade ou escopo declarados | Hold negado por falta de autoridade/escopo/revisão; nenhuma retenção estendida silenciosa; acesso comum não é reativado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-062-F | ADR-062 | Expiração/disposição de uma cópia falha após bloqueio lógico | Consultar a cópia e a evidência de disposição | Estado da cópia `unknown`/`pending` com objeto e superfície identificados; nenhuma declaração de descarte global; outras cópias mantêm estado próprio | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-063-P | ADR-063 | Artefato do cliente de voz com SBOM, build reproduzível, assinatura válida e `generation` igual ou superior ao piso | Instalar e iniciar PTT | Instalação aceita; digest do build comparado; `generation` registrada; sessão PTT opera com binário verificado | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-063-N | ADR-063 | Release anterior com vulnerabilidade conhecida, corretamente assinado | Instalar sobre o piso atual | Rejeição por `generation` inferior ao piso; binário existente não é substituído; cliente de voz não inicia sessão com a versão rebaixada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-063-N2 | ADR-063 | Pacote de atualização adulterado mantendo o número de versão | Instalar o pacote | Rejeição por assinatura/digest antes de qualquer execução do código novo; nenhuma sessão ou biblioteca carregada do pacote | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-063-F | ADR-063 | Chave de assinatura comprometida, com lista de revogação publicada | Atualizar com artefato assinado pela chave revogada | Lista verificada no cliente e artefato rejeitado; atualização suspensa até reemissão de chave; evento auditável registrado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C007-063-B | ADR-063 | Rollback autorizado como exceção limitada no tempo, com piso rebaixado por decisão registrada | Instalar release anterior autorizado | Instalação aceita e auditada com autoridade e janela temporal; sem a decisão registrada, o mesmo artefato permanece rejeitado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §9 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-005 | C007-056-P (reformulado) | C007-056-N, C007-059-N, C007-059-N2 | C007-059-B, C007-063-B | C007-059-F, C007-061-F | Autoridade de floor, fencing e lease seguem `OPEN-007-A/C`; reconexão exige sessão/stream atuais |
| RNF-007 | C007-RNF007-P1 (novo) | C007-RNF007-N1 (novo) | C007-RNF007-B1 (novo) | C007-RNF007-F1 (novo) | Sem metas de jitter/perda (`OPEN-007-J`), só caracterização medida; sem limiar aprovado não há PASS de qualidade |

Casos novos, descritos em uma linha: **C007-RNF007-P1** — sessão de voz e publicação de timeline simultâneas com o bus inspecionado: nenhum payload de áudio em qualquer codec/representação e metadados autorizados distinguíveis. **C007-RNF007-N1** — amostra sintética de `AudioFrame`/Opus/RTP enviada ao bus: rejeitada na fronteira definida, nada serializado, persistido ou enfileirado. **C007-RNF007-B1** — mídia em codec ou encapsulamento não enumerado: classificada como mídia vedada ao bus, sem fallback para log, fila ou bus. **C007-RNF007-F1** — falha isolada do bus durante sessão ativa: áudio não migra para bus/log/fila e o estado da mídia é exibido conforme controle vigente, sem prometer continuidade nem corte instantâneo.
