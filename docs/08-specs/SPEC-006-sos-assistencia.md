# SPEC-006 — SOS e assistência

**Versão:** DRAFT-0.9 · **Estado:** DRAFT/OPEN · **Data:** 2026-11-17  
**Autora documental:** Nancy Leveson · **Revisor designado:** Edsger Dijkstra  
O parecer existente refere-se ao DRAFT-0.1. Revisão independente do DRAFT-0.4 por Edsger Dijkstra **pendente**. Execução: `NOT_EXECUTED`. Metas quantitativas: **OPEN — a medir**.

Documento proposto, não aprovado, implementado nem validado. Não habilita operação de emergência, atendimento, despacho ou entrega garantida. Casos formais `NOT_ASSIGNED`; execução `NOT_EXECUTED`. Nenhum parâmetro quantitativo é aprovado por este texto.

## Mudanças desta versão

### DRAFT-0.8 — ADR-064 na negativa offline e alinhamento à ADR-065

- `C006-059-P` deixa de esperar incidente aberto e enfileirado: conforme ADR-064, sem fonte de frescor verificável (F1/F2) e sem verificação online (F3), a abertura de incidente por grant offline é **negada**, nada é enfileirado e o estado observável informa indisponibilidade offline. O caminho positivo é preservado em **`C006-059-P2`**, restrito ao caso com F3 verificado antes do efeito. O ID e a posição do caso original foram mantidos para rastreabilidade; o `Tipo` migrou de P para N.
- A subseção “Retenção mínima de SOS” deixa de declarar prazo, jurisdição e gatilho como `OPEN`: o prazo padrão é o da **ADR-065** e o que falta é **confirmação jurídica por jurisdição**, não o prazo.
- `C006-062-F2` passa a falhar fechado quando a cópia específica não tem prazo declarado, em vez de tratar o prazo como `OPEN`.
- Nenhum valor numérico novo é criado; nada é aprovado, implementado ou executado por este delta.

### DRAFT-0.7 — casos por requisito e casos P desbloqueados (C3-1)

Acrescenta a subseção `### Casos por requisito (P/N/B/F)` ao final, com uma linha por requisito declarado nesta SPEC e as quatro colunas preenchidas ou `n/a` justificado. Reescreve os casos P cujo esperado dependia de valor não decidido — janela N/N-1 e limite de frescor do perfil, prazo/jurisdição/gatilho da CME-SOS — para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID. Acrescenta o caso de borda `C006-061-B` (ADR-061), que faltava. Nenhum valor numérico é criado; nada é aprovado, implementado ou executado. Complementa, sem substituir, as seções de aceite e de casos por decisão.

### DRAFT-0.5 — casos por decisão (resposta ao veto C3-1)

O crítico C3 vetou as SPECs por ausência de casos vinculados às decisões ADR-056…063. Esta versão acrescenta a seção “Casos por decisão (ADR-056…063)”, com uma linha por decisão aplicável a esta SPEC, cobrindo ao menos um caso negativo e um de falha/recuperação por decisão. Todo esperado é observável (código, estado ou campo); todo caso permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Parâmetro `OPEN` produz comportamento de negar/informar desconhecido, nunca número inventado. Nenhuma decisão é alterada e nada é aprovado ou executado por este delta.

### DRAFT-0.4 — retenção mínima e falha observável (achado C4)

Achado do crítico C4: aplicar a falha fechada da ADR-062 sem exceção explícita podia **descartar o SOS sem estado observável** ou levar a **gravação improvisada sem prazo**. Esta versão acrescenta a subseção “Retenção mínima de SOS e comportamento de falha”, com quatro partes: (1) classe mínima de emergência (`CME-SOS`) persistida por dever de cuidado, com finalidade declarada, prazo `OPEN — decisão do usuário` e proibição de posição precisa, áudio e conteúdo sem política própria; (2) regra de falha observável — “não salvo / não enfileirado”, nunca “acionado” — e a evidência volátil que sobrevive em memória; (3) casos de teste planejados de retenção disponível, ausente, partição, energia perdida antes da persistência e fallback não autorizado; (4) `ack_class` por etapa com quatro negativas, incluindo classe ausente e classe inferior. Nenhum prazo, jurisdição ou duração é fixado; nada é aprovado, implementado ou executado por este delta. Duas linhas novas entram na tabela de dependências OPEN.

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

No SOS, `acknowledged_human` exige prova da ação da pessoa para o incidente. Nenhum ACK técnico autoriza exibir “atendimento”. Mesmo o reconhecimento humano só permite exibir atendimento em curso se houver evidência específica dessa ação.

### ADR-059 — Abertura de SOS offline

No fluxo SOS, o grant offline assinado só permite abrir o incidente local e enfileirá-lo. Exigir ação enumerada no escopo do grant, `epoch` comparável e vigente, frescor dentro do limite do perfil, ausência de revogação conhecida e prova anti-rollback/anti-clone nos termos de SPEC-002 §6.3 (fonte F1, F2 ou F3; contador no mesmo armazenamento do cliente **não** é prova; sem fonte confiável a operação é negada) por armazenamento protegido e contador monotônico. Negar se qualquer condição falhar, se o estado for `stale` ou `unknown`, se o relógio for incerto ou se a política estiver indefinida. O limite de frescor permanece **OPEN — a medir**.

É proibido compartilhar posição precisa offline, inclusive com responsável previamente autorizado. A fila não transmite por si só. Na reconexão, revalidar autoridade, audiência e política antes do envio. No aceite planejado, restore de snapshot antigo, epoch anterior ou revogação conhecida devem impedir o uso do grant mesmo dentro da validade nominal.

## Objetivo e cobertura

Definir o comportamento documental de um incidente SOS, desde o acionamento autorizado até a apresentação de evidências por etapa e destino, incluindo falhas, recuperação, posição e atenção humana. RF-026 e RNF-012 são cobertos em seu subescopo SOS, ainda com decisões abertas; RF-011 e RNF-009 são interfaces de eventos e evidência sob integração de Martin Kleppmann. RF-028 pertence a Guardian e RF-033, RF-042, RNF-016 e RF-043 às frentes de experiência e cliente; aqui apenas suas interfaces SOS são tratadas. Cobertura integral por requisito, modalidade e plataforma permanece parcial.

Fontes: [requisitos](../00-produto/requisitos.md) linhas 17, 33, 45, 49, 57, 62, 76, 77 e 84; [matriz](../06-planejamento/matriz-rastreabilidade.md) linhas 97, 112, 121, 124, 126, 131, 140–145 e 166. [Serviço SOS](../02-dominio/servico-sos.md) linhas 3–7, 20–35, 45–57, 66–74 e 105–112; [contrato inicial SOS](../07-contratos/transcricao-sos.md) linhas 3, 42–48, 64 e 87, ainda não congelado; [testes móveis](../05-operacao/testes-mobile-guardian.md) linhas 3, 13–15 e 33–35, planejamento sem execução. [Decisões](../06-planejamento/decisoes.md) linhas 49 e 53: ADR-039/041 aprovadas como direção, não validação do serviço. Arquitetura e ADRs prevalecem conforme [README](../README.md) linhas 97–98.

Base de elaboração: contribuição local SOS inicial, posteriormente revista nos adendos Edsger SOS-R01–R03, resposta Raphael S-03, quatro achados Edsger e rechecagem; consultar `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-nancy-leveson.md` e `contribuicao-edsger-dijkstra.md`. Revisões de Edsger em `contribuicao-edsger-dijkstra.md:25–48` e Raphael em `contribuicao-raphael-mudge.md:79–87` são focais de versões anteriores, não aprovação deste arquivo. `DECISAO-PROPOSTA-SOS-ACK.md` §§ Delta e Convergência registra convergência documental do princípio, com fronteiras e risco regional abertos; a fonte direta da resposta do usuário relatada por Ada não foi acessada nesta revisão.

## Atores, permissões e objetos

O solicitante ou dispositivo autorizado inicia o SOS explicitamente; o Edge e o serviço SOS encaminham dados dentro de seus escopos; destinatários autorizados recebem o que suas permissões permitem; somente uma pessoa autenticada e autorizada pode assumir `claimed`. A fonte SOS prevê `incident_id`, `event_id`, origem, tempos, audiência, tentativas, confirmações, status e localização opcional. O significado, formato e fronteiras de cada identificador são dependências dos contratos de eventos e API, não wire aprovado por esta SPEC. Localização precisa exige autorização específica por incidente, canal protegido e auditoria. Sem autoridade atual para operação sensível, preservam-se as negativas de [identidade e autorização](../07-contratos/identidade-autorizacao.md) linhas 113–118 e 238–247; não há exceção SOS automaticamente aprovada.

## Estados e fronteiras de evidência

| Dimensão documentada | Estados da fonte | Evidência e limite |
|---|---|---|
| Registro | `created_local`, `accepted_by_node`, `replicated` | `created_local` representa persistência local; `accepted_by_node` significa recebido por servidor. Correspondência exata a commits e Edge permanece OPEN. |
| Transporte por destino | `queued`, `submitted`, `gateway_accepted`, `endpoint_confirmed`, `unconfirmed`, `failed`, `expired` | Cada estado descreve sua camada e destino; enfileiramento não demonstra recepção final. |
| Atenção humana | `notified`, `viewed`, `claimed`, `response_in_progress`, `resolved` | `claimed` exige ação de pessoa autorizada; estado técnico não prova atendimento. |
| Encerramento | `cancelled`, `resolved`, `expired`, `test_completed` | Desfecho auditável; não apaga silenciosamente histórico permitido. |

Dispositivo e Edge podem emitir `persisted_local` apenas para sua própria gravação. A proteção contra perda de nó e zona é uma propriedade adicional cuja prova permanece aberta: uma proposta de confirmação durável depende de autoridade válida e prova de commit nos domínios independentes necessários à proteção relatada de nó e zona. `accepted_by_node` não implica esse predicado. Objeto e versão protegidos, prova, frescor, fronteira e correspondência entre estados estão OPEN; cópias ou nomes de zonas não provam independência. Falhas comuns e dependências compartilhadas precisam de análise.

Proposta de transições: após persistência local comprovada mostrar apenas registro local; após prova da camada técnica mostrar a confirmação daquela camada e destino; após ação humana autorizada mostrar claim. Se resposta ao commit se perder, o resultado é desconhecido para o cliente, não rejeição. Consultar e, quando autorizado, repetir a mesma operação com identidade e conteúdo consistentes; conflito e efeitos externos seguem contrato de sync. Ausência de prova impede afirmar sucesso. Se uma confirmação histórica existir, preservar sua evidência disponível e escopo conforme política, e mostrar separadamente proteção atual degradada ou desconhecida. Perda comprovada de incidente ou evidência é perda conhecida, distinta de desconhecido, com escopo e consequência comunicados; nunca inferir recuperação do que se perdeu.

## Fluxos e falhas

1. **Criação:** validar ação e autoridade aplicáveis; registrar o pedido sem depender de ASR. Offline, exibir persistência local apenas com prova e destacar falta de confirmação externa. Sem prova de salvamento, informar falha ou desconhecido.
2. **Envio e escalonamento:** reavaliar autoridade, audiência, consentimento, revogação, validade e rota antes de divulgação. Manter tentativa e evidência por destino. Ausência de destinatário elegível ou claim permanece visível. Ordem, cobertura, duração e responsabilidade de escalonamento são OPEN; não inventar equipe de plantão.
3. **Posição:** conservar origem, `observed_at` e precisão disponíveis; indicar idade e incerteza. Posição expirada não encerra incidente. Encaminhar pedido sem coordenada, quando autorizado, é alternativa proposta; entregar posição antiga depende de política e autorização explícitas. Expiração não reinicia em replay. Não divulgar coordenada precisa em chat ou bridge por padrão.
4. **Cancelamento e recuperação:** cancelamento explícito é auditável, com notificação segundo política. Após partição ou restore, reconciliar autoridade, revogação, expiração, eventos e evidências antes de novas divulgações ou alegações. Histórico ausente permanece desconhecido. Perda de sinal não encerra automaticamente incidente. Efeitos externos não são considerados únicos apenas por deduplicação local.
5. **Teste:** `test_mode` deve impedir efeitos e destinatários reais; isolamento e identificação do modo dependem de contrato e verificações futuras. Falta de elegibilidade satelital ou ausência de conectividade não é entrega.

Sem prova dos domínios exigidos, inclusive em instalação autogerida, a confirmação protegida fica indisponível; operações comprovadamente independentes e autorizadas podem continuar. Não reduzir silenciosamente a proteção nem presumir que o operador possui zonas. RPO 15 min, RTO 1 h e ACK em 5 s são hipóteses de comparação não aprovadas nem demonstradas; RTO de persistência/encaminhamento não é prazo de atendimento humano.

> **ADR-064 (17/11/2026):** sem F1/F2 e sem verificação online, a operação offline condicional é **negada**, inclusive abertura de incidente por grant. A UI declara o estado antes do incidente e oferece o discador do sistema como caminho disponível.

## Retenção mínima de SOS e comportamento de falha

### Por que existe esta subseção

A ADR-062 exige política aprovada identificada por `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)` e manda falhar fechada sem política. Aplicada sem exceção ao acionamento de SOS, essa regra produz duas condutas inaceitáveis: descartar o próprio SOS sem estado observável, ou gravar de improviso sem prazo declarado. A ADR-062 já admite “preservando apenas o mínimo necessário para o efeito imediato autorizado”; a classe abaixo é a aplicação desse trecho ao SOS. Ela **não** é política de retenção aprovada por si: a finalidade é declarada e o **prazo padrão é o da ADR-065** (tabela por classe em `04-seguranca/retencao-revogacao.md`, linha de incidente SOS, com prazo por classe e por cópia, gatilho e descarte verificado). O que permanece pendente é a **confirmação jurídica por jurisdição** — não o prazo. Registro documental, sem efeito de aprovação.

### Classe mínima de emergência (CME-SOS)

| Elemento | Conteúdo admitido | Limite |
|---|---|---|
| Identificador | Um identificador único e opaco do incidente, mais o identificador da tentativa de acionamento | Formato, geração e imutabilidade dependem de C07/C08; nada de novo é definido aqui |
| Estado | Estado documental do próprio registro, de `created_local` ao estado corrente da tentativa | Um estado por registro; não resume entrega, proteção, claim ou atendimento |
| Evidência de acionamento | Ação explícita do solicitante ou dispositivo autorizado: tipo de ação, horário do acionamento, origem (perfil e dispositivo) e referência do grant quando offline | Sem posição, sem áudio, sem conteúdo |
| Finalidade declarada | Preservar prova de que um SOS foi acionado, para dever de cuidado e responsabilização da operação | Finalidade única; não autoriza uso secundário, telemetria, analytics, treino de modelo ou marketing |
| Prazo e descarte | **Definido pela ADR-065**: incidente SOS 12 meses; evidência em memória = vida do processo; revisão jurídica por jurisdição pendente | Expiração é verificada e registrada; hold legal suspende com autoridade e revisão a cada 90 dias. Nenhuma promessa de descarte antes da revisão jurídicaplementada |
| Cópias admitidas | Dispositivo de origem em armazenamento protegido e, só quando houver autoridade e política daquela cópia, nó ou Edge | Cada cópia exige a própria entrada de política; cópia autorizada não autoriza outra |

Regras da CME-SOS:

1. A CME-SOS é **piso**, não política completa. Qualquer dado além dela exige política própria aprovada por classe, cópia e jurisdição.
2. É **proibido** gravar sob a CME-SOS: posição precisa, inclusive a que a ADR-041 autoriza para responsável auditado; áudio ou PTT; ASR, transcrição ou qualquer derivado de conteúdo; texto livre do solicitante; contatos e conteúdo de mensagens. A ausência de política própria **nega** esses campos; não os improvisa, nem por “utilidade” do atendimento.
3. Falta de política para uma classe enriquecida **não** pode impedir a gravação da CME-SOS. O registro degrada para o piso e a degradação é informada.
4. A CME-SOS não atesta recepção, replicação, entrega, atendimento ou socorro. Retenção não é ACK; `persisted_local` não é `delivered_to_device` (ADR-058) e não vira `acknowledged_human` por passagem de tempo.
5. Gravação no piso é auditável e reversível: excluir ou expurgar a CME-SOS exige a política de descarte que está OPEN, não uma decisão local do operador.

### Falha observável

Se nem a CME-SOS puder ser persistida — armazenamento protegido indisponível, escrita negada, espaço esgotado, política ausente para a cópia exigida — o resultado é falha declarada, nunca SOS silencioso.

| Situação | Texto obrigatório na UI | Log local | Proibido |
|---|---|---|---|
| Nada persistido nem enfileirado | “não salvo / não enfileirado” | mesmo veredito, com horário, motivo e código de erro; classe da etapa ausente ou `unknown` | “acionado”, “registrado”, “enviado”, “em análise”, “a caminho” |
| Registro local parcial (metadado sem registro íntegro) | “salvo localmente de forma parcial” | motivo e o que falta gravar | tratar como registro completo ou como enfileirado |
| Registro persistido, sem enfileiramento | “salvo localmente; não enfileirado” | separa as duas etapas com evidência própria | “enfileirado”, “enviado”, “a caminho” |

Regras de falha:

1. UI, log local e qualquer texto de ajuda usam o **mesmo** veredito. Não existe mensagem tranquilizadora divergente do log.
2. Fallback para outra cópia, outro armazenamento ou outro formato só com autorização e política daquele destino. Tentativa de fallback não autorizado é **negada e informada**; nunca executada em silêncio, nunca registrada na cópia vedada.
3. O veredito de não salvamento não apaga nem substitui a oferta de ação oficial de emergência do aparelho descrita em “Revisão focal e salvaguardas adicionais”; oferecer o discador não prova que houve chamada.
4. Falha de persistência não encerra o incidente nem o marca como inexistente. Se um registro anterior existir, ele é preservado e exibido com sua evidência disponível.

### Evidência mínima em memória

Quando nada é persistido, uma evidência volátil pode sobreviver — e precisa ser exibida como volátil:

- **Conteúdo:** identificador gerado na sessão, quando houver; horário do acionamento; estado; motivo da falha; número de tentativas.
- **Duração:** enquanto o processo do aplicativo viver. Suspensão, término pelo sistema operacional ou reinício apagam o conjunto; não há gravação, journal ou recuperação associada. Limite documentado de retenção em memória permanece **OPEN — decisão do usuário**; nenhuma duração é prometida aqui.
- **Apresentação:** rotulada “memória volátil, não persistida”; não sobrevive a reinício, não é exportável por padrão e não sustenta alegação de entrega, de enfileiramento ou de atendimento.
- **Após reinício:** a ausência de registro é declarada como desconhecido — nunca “provavelmente salvo”, nem inferida de notificação, histórico de notificação ou cache.

### `ack_class` por etapa e negativas

Toda resposta de API carrega `ack_class` (ADR-058). Esta subseção fixa onde a retenção aparece na cadeia e as negativas que UI e API devem implementar.

| Etapa | Classe máxima admissível | Negativa exigida |
|---|---|---|
| Gravação da CME-SOS no dispositivo | `persisted_local` | Sem commit comprovado não existe classe: resultado “não salvo”, etapa `unknown` |
| Envio a nó ou gateway | `transport_accepted` | Sem recibo daquele salto, não promover |
| Aceite por autoridade | `replicated_to_authority` | Depende de B06/B12 e C06; hoje sem classe disponível |
| Entrega ao dispositivo destinatário | `delivered_to_device` | Exige recibo do próprio destino, vinculado ao objeto |
| Claim ou reconhecimento humano | `acknowledged_human` | Exige ação de pessoa autenticada e autorizada, vinculada ao incidente |

Negativas obrigatórias:

1. **Classe ausente:** resposta sem `ack_class` é resposta inválida; a etapa fica `unknown` e nada é exibido como confirmado. O código de erro e a representação de “inválida” dependem de C07/C08 e permanecem `OPEN`; o que se fixa é a regra, não o nome do erro.
2. **Classe inferior:** se a resposta traz `persisted_local` e a etapa exige `delivered_to_device` ou `acknowledged_human`, a UI **não promove**. Mostra a etapa comprovada e a etapa aberta, separadas e nomeadas.
3. **Classe acima do emissor:** classe cujo emissor não corresponde à fronteira declarada (por exemplo `materialized` sem prova de projeção) é tratada como inválida.
4. **Promoção por tempo:** ausência de erro, silêncio prolongado, nova tentativa ou sucesso em outra fronteira nunca promovem classe.
5. **Retenção não promove:** persistir a CME-SOS não cria `delivered_to_device` nem `acknowledged_human`; e falha de retenção não rebaixa uma classe já comprovada e exibida com sua evidência histórica.

### Casos de teste planejados

Todos `NOT_EXECUTED`; nenhum caso formal atribuído. Complementam a tabela de aceites e não a substituem.

| Caso | Estímulo | Resultado esperado proposto |
|---|---|---|
| Retenção disponível | Política da cópia de origem presente; armazenamento protegido grava | CME-SOS persistida com `persisted_local` e evidência de commit; posição, áudio e conteúdo ausentes do registro |
| Retenção ausente | Nenhuma entrada de política para a cópia | Apenas o piso é gravado; classe enriquecida negada com motivo; degradação informada; nada improvisado |
| Retenção ausente e piso indisponível | Escrita negada ou espaço esgotado | UI e log dizem “não salvo / não enfileirado”; nenhum texto “acionado”; ação oficial de emergência continua oferecida |
| Partição | Dispositivo isolado durante o acionamento | Piso com `persisted_local`; enfileiramento declarado separadamente e conforme a verdade; nenhuma classe superior |
| Energia perdida antes da persistência | Corte de energia entre a ação e o commit | Após reinício, estado desconhecido declarado; evidência volátil não reapresentada como registro; nada inferido de cache ou notificação |
| Fallback não autorizado | Destino alternativo sem política ou autorização | Tentativa negada e informada; nada gravado na cópia vedada; registro existente preservado |
| `ack_class` ausente | Resposta de API sem classe | Resposta tratada como inválida; etapa `unknown`; UI não confirma |
| `ack_class` inferior | Resposta `persisted_local` para etapa que exige entrega ou humano | UI não promove; exibe etapa comprovada e etapa pendente |
| Prazo OPEN | Registro cujo prazo não foi decidido | Nenhuma expiração ou descarte automático anunciado; revisão aplicada antes de qualquer descarte |

## Revisão focal e salvaguardas adicionais

A interface deve oferecer, quando aplicável, ação explícita para usar o discador nativo ou funções oficiais de emergência do aparelho. A elegibilidade e o comportamento quando indisponível dependem da plataforma; não presumir acesso irrestrito a API, integração direta com autoridades ou execução de chamada. A conciliação cabe a Nancy, Lea e responsáveis por plataforma (`servico-sos.md:9,112`).

A hipótese de RPO de 15 minutos descreve um possível objetivo geral de recuperação, não demonstra o predicado de proteção por incidente após ACK. Um ponto recuperável anterior pode omitir incidentes com ACK histórico; tratar tal perda como conhecida somente quando comprovada, comunicar escopo e consequências e não rotulá-la conforme à proteção escolhida sem prova específica. Risco regional e eventual perda tolerada por modalidade exigem decisão explícita; não se infere RPO zero. Nancy, Brendan e dados/sync conciliam objeto, falhas e evidências.

A política futura de sobrecarga e antiabuso deve preservar autenticação, limites e prioridade SOS sem impedir solicitações legítimas, conforme `servico-sos.md:55,69–74` e `transcricao-sos.md:64`. Não há bypass de autorização ou garantia de atendimento. Limites, reservas, comportamento ao esgotamento e observabilidade são OPEN para Nancy, Brendan, segurança e transporte.

## Aplicabilidade

Personal, Teams, Node e Platform/SDK estão no escopo; servidor Linux e clientes macOS arm64/x86_64, iOS, Windows, Linux desktop, Android e web requerem mapeamento por modalidade e capacidade. Nenhuma plataforma é declarada não aplicável sem justificativa revisada. Permissões, background, persistência, energia e acesso a recursos oficiais de emergência dependem das SPECs de cada plataforma e evidência futura. Guardian com check-in atrasado gera aviso, não SOS automático. Projeções de Live Activities/Updates devem refletir evidência da fase e destino; não constituem prova independente.

## Aceites e casos planejados

Os seguintes critérios são propostas parciais; nenhum caso formal foi atribuído ou executado. Antes da aprovação da SPEC, completar autoridade, fonte da evidência, plataforma, política e parâmetros aplicáveis.

| Condição e estímulo | Resultado esperado proposto | Cobertura |
|---|---|---|
| Pedido autorizado sem rede e com persistência local comprovada | Exibir registro local e ausência de confirmação externa, sem alegar entrega | RF-026, RNF-012 |
| Commit ocorre e resposta se perde; operação reapresentada | Manter resultado desconhecido até evidência; reconciliar identidade e impedir duplicação conforme contrato | RF-011, RNF-009, B06 |
| Edge confirma etapa local; falta prova de domínios | Exibir somente camada comprovada, sem afirmar proteção contra perda | RF-026, RNF-012 |
| Confirmação histórica seguida de degradação ou perda comprovada | Distinguir evidência histórica, estado atual desconhecido/degradado e perda conhecida com escopo | RNF-009, RNF-012 |
| Destino técnico confirma sem claim; pessoa não autorizada tenta assumir | Apresentar confirmação técnica e ausência de claim; rejeitar claim não autorizado | RF-026, RNF-012 |
| Revogação ou posição expirada enquanto a fila está offline | Revalidar na reconexão; não divulgar dado vedado, não renovar observação nem encerrar automaticamente incidente | RF-026; política OPEN |
| Restore, falha de domínio ou replay duplicado | Reconciliar provas e efeitos conforme autoridade; não presumir recuperação ou efeito externo único | RNF-009; B06/B12 OPEN |
| Plataforma elegível oferece função oficial; função indisponível em outra condição | Oferecer ação explícita quando aplicável; sinalizar indisponibilidade sem alegar chamada ou integração | RF-026; interface de plataforma OPEN |
| Falha após ACK com restauração em ponto anterior | Comparar objeto e evidência; se perda for comprovada, comunicá-la e não inferir conformidade apenas por RPO | RNF-009/RNF-012; risco por modalidade OPEN |
| Sobrecarga e tráfego abusivo junto a pedido legítimo | Evidenciar aplicação de prioridade e limites definidos, sem bypass de autorização; observar falhas e possível bloqueio indevido | RF-026; parâmetros e condições de teste OPEN |
| Modo teste ou Guardian com check-in vencido | Modo teste não produz efeito real; atraso Guardian isolado não cria SOS | RF-026, interface RF-028 |
| Superfície de atividade sem evidência de claim | Projetar apenas etapa comprovada, sem anunciar atendimento humano | Interface RNF-016 |

## Relação com eventos e sincronização

O contrato transversal de eventos e sincronização é uma proposta documental v0.1, não wire final ou evidência de implementação (`docs/07-contratos/eventos-sincronizacao.md:1–2`). Esta SPEC usa seus conceitos sem criar mapeamento ratificado: `Command.lifecycle` e `DeliveryAttempt.state` são dimensões diferentes; perda de resposta após envio fica `outcome_unknown`; consulta do recibo antecede novo efeito; tentativa mantém histórico append-only, e somente uma nova tentativa do mesmo efeito pode reutilizar a identidade lógica (`eventos-sincronizacao.md:16–27`). A confirmação humana é separada e exige principal autenticado (`:19,36`). O contrato distingue `completed` de `resolved` e rejeita inferir atendimento humano do ACK de rede (`:34–47`).

Para SOS, aplicar essas regras exige que a chave idempotente sobreviva ao retry/reconciliação e que a fingerprint corresponda ao mesmo conteúdo e escopo. Conteúdo diferente com a mesma chave é conflito; chave desconhecida depois da janela permitida não pode ser executada novamente. Consulta e divulgação do resultado ainda exigem autorização atual. Limites de retenção/dedupe, representação dos identificadores, códigos de erro e efeitos externos dependem de C07/C08/C10 e não estão aprovados. A semântica detalhada de Inbox, Outbox, checkpoint e restore também depende de B06/B12; sua existência documental não prova que o ACK SOS cobre o objeto protegido. Em especial, ACK da Inbox confirma apenas a fronteira que o contrato venha a aprovar, não materialização, replicação regional ou claim humano (`eventos-sincronizacao.md:175–193`).

## Decisões e dependências OPEN

| Questão e responsáveis propostos | Alternativas e consequência | Fechamento documental |
|---|---|---|
| Objeto, fronteira, independência e prova de ACK: Nancy, Kleppmann, Graydon, Michael, Leslie, Brendan e Lea | Recepção local limitada ou confirmação protegida após prova definida; maior exigência pode impedir confirmação sob partição | Crosswalk, objeto/versionamento, autoridade, evidência, apresentação e compatibilidade conciliados; revisão independente |
| Falha regional por modalidade: Nancy, Brendan, Grady e dados/sync | Domínios independentes autorizados, indisponibilidade de confirmação ou risco residual explicitamente escolhido; custos e perda desconhecidos | Comparação com dados e cópias afetados, residência e aprovação específica do usuário; sem afirmar capacidade |
| Retenção e posição: Bruce, Kim, Michael, Paul, Margaret e Nancy | Regras distintas para incidente, evidência, localização, fila, backup e exportação; bloquear posição obsoleta pode reduzir informação ao destinatário | Finalidade, acesso, gatilho, duração, disposição e revisão jurídica aplicável por cópia definidos |
| Prazo, jurisdição e gatilho de descarte da CME-SOS: Nancy, Bruce, Kim e revisão jurídica | Manter o piso sem prazo declarado preserva a prova de acionamento, mas mantém retenção sem base fechada; prazo curto pode destruir prova antes da revisão do incidente | Entrada de política por finalidade, classe e cópia com prazo e gatilho aprovados pelo usuário, substituindo o `OPEN` |
| Duração e apresentação da evidência volátil em memória e código de resposta inválida sem `ack_class`: Nancy, Brendan, C07/C08 | Evidência em memória expira com o processo e pode induzir falsa confiança; nome do erro e forma de representação de resposta inválida pendem dos contratos | Limite documentado de memória e representação de resposta inválida conciliados sem fixar wire antes da aprovação |
| Autorização offline: Kim, Bruce, Alex e Kleppmann | Limite de frescor do perfil e evidência anti-rollback conforme ADR-059; revogação pode não alcançar dispositivo isolado | Ações permitidas, frescor, relógio, revogação e replay conciliados sem afastar negativas existentes |
| Antiabuso e capacidade sob sobrecarga: Nancy, Brendan, segurança e transporte | Rate limiting sem bloqueio indevido de SOS legítimo, reservas, circuit breakers e política de esgotamento; limites ausentes podem permitir abuso ou impedir solicitação legítima | Critérios de admissão, prioridade, observação e casos negativos definidos e aprovados, sem bypass de autorização ou alegação de atendimento |
| Destinatários e escalonamento: Margaret, Nancy e Brendan | Definir contatos, sequência e cobertura; ausência de claim deve ser visível | Autoridades, responsabilidades, política e alternativas aprovadas sem prometer socorro |
| B03/B04, B06/B12, B09/B10 e B01/B11 | Transporte/prioridade, sync/autoridade, operação/restore e identidade são contratos externos a esta peça | Versões e interfaces conciliadas pelos responsáveis; B02 localização, B08 ASR e RF-033/RF-042/RF-043/RNF-016 também dependem de integração |

Aprovação requer revisão independente da versão exata, conciliação das dependências e resolução das lacunas materiais. Ensaios de produto dependem de autorização futura e não são alegados aqui. Jev não foi executado: esta frente não autoriza conexão a serviços; conclusões são documentais.

## Revisão independente da versão anterior

Edsger revisou DRAFT-0.1 SHA-256 `a8a483bb939c7a17991eec3cc75efa23c31de745cba2ea25a9dd7bec3eec1f7b` em `contribuicao-edsger-dijkstra.md:104–115`. Os achados SOS-SPEC-01 a 03 e a referência RF-043 foram incorporados nesta versão; permanecem abertas a conciliação entre frentes, decisões e nova revisão independente. Nenhum cenário foi executado.


## Revisão independente do DRAFT-0.1 e delta

Edsger revisou o DRAFT-0.1 SHA-256 `a8a483bb939c7a17991eec3cc75efa23c31de745cba2ea25a9dd7bec3eec1f7b` em `contribuicao-edsger-dijkstra.md:104–115` e rechecagem posterior; os quatro achados focais foram incorporados. A revisão informou que não identificou conflito material nesses pontos, mas manteve elegibilidade de plataforma, proteção pós-ACK, limites de sobrecarga e critérios determinísticos abertos. O DRAFT-0.2 acrescentou relação explícita com o contrato transversal de eventos/sync. O DRAFT-0.3 aplicou ADR-058 e ADR-059. O DRAFT-0.4 responde ao achado C4 com a seção “Retenção mínima de SOS e comportamento de falha”, acrescentando requisitos de retenção mínima, falha observável, evidência volátil, `ack_class` por etapa com negativas e casos de teste planejados. Nenhum desses deltas foi revisado por terceiro: a revisão do DRAFT-0.4 por Edsger Dijkstra permanece pendente e deve ocorrer por hash desta versão. Nenhuma aprovação ou execução decorre das revisões anteriores ou deste delta.

## Casos por decisão (ADR-056…063)

Casos planejados, sem ID formal: `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED` em todas as linhas. `Tipo`: **P** positivo, **N** negativo, **B** borda, **F** falha/recuperação. Cada decisão aplicável tem ao menos um caso N e um F. Onde o parâmetro é `OPEN`, o esperado é negar ou informar desconhecido — nunca um número, prazo ou código inventado. Estes casos complementam, e não substituem, as tabelas de aceite já existentes.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| C006-056-P | ADR-056 | Incidente criado por binário do perfil N; réplica no perfil N-1; **janela de suporte não declarada** no perfil versionado (`OPEN — decisão de produto`) | Replicar e reler o incidente na réplica N-1 | Esperado reformulado com parâmetro ausente: a operação responde `param_not_configured`, a compatibilidade é exibida como desconhecida e nenhum campo é apresentado como preservado; nenhum aceite por “melhor esforço”. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-056-N | ADR-056 | Peer N-1 sem a feature crítica exigida pelo incidente (campo obrigatório de audiência) | Enviar o incidente ao peer sem a feature | Rejeição antes de qualquer efeito: nenhum `incident_id` persistido no destino, nenhuma fila criada, nenhuma classe de ACK emitida; ausência de downgrade silencioso registrada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-056-B | ADR-056 | Evento gravado por binário N com campo novo; leitor N-1 | Ler e regravar o evento no leitor N-1 | Campo desconhecido preservado na regravação; nenhum número de campo reutilizado; nenhum campo descartado no relatório de leitura | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-056-F | ADR-056 | Perfil do incidente sem janela de suporte declarada (`OPEN — decisão de produto`) | Replicar o incidente entre versões distintas | Rejeição declarando incompatibilidade e estado desconhecido; nenhum aceite por “melhor esforço”; nenhuma exceção sem autoridade registrada | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-057-P | ADR-057 | Timeline de incidentes com eventos em uma partição e `authority_epoch` conhecido | Consultar a timeline com cursor versionado contendo a tupla `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` | Página retornada com os eventos daquela partição/epoch; cursor devolvido com versão declarada e vínculo a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-057-N | ADR-057 | Cursor construído sem `partition` | Apresentar o cursor à leitura de timeline | Cursor rejeitado com erro explícito; nenhuma página, nenhum evento e nenhuma tradução para outra forma | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-057-B | ADR-057 | Duas partições com a mesma `seq` e mesmo tenant/stream | Consultar ambas as partições | Dois eventos distintos devolvidos, sem colapso nem deduplicação indevida; ordenação por partição explícita | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-057-F | ADR-057 | Cursor emitido antes de troca de `authority_epoch` (após failover/restore) | Reapresentar o cursor após a troca | Rejeição fechada por epoch incomparável; nenhum evento servido e nenhum estado antigo apresentado como atual | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-058-P | ADR-058 | CME-SOS gravada com prova de commit no armazenamento protegido | Consolidar a etapa de gravação local | Resposta com `ack_class=persisted_local` e evidência do objeto/versão; UI exibe apenas “salvo localmente” | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-058-N | ADR-058 | Etapa declarada como entrega a dispositivo destinatário | Resposta traz somente `persisted_local` | UI não promove: exibe etapa comprovada (`persisted_local`) e etapa pendente (`delivered_to_device`) separadas; nenhum texto de entrega | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-058-B | ADR-058 | Resposta de API de SOS sem o campo `ack_class` | Consumir a resposta | Resposta tratada como inválida; etapa fica `unknown`; nenhuma confirmação exibida (nome do código de erro permanece `OPEN` em C07/C08) | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-058-F | ADR-058 | Envio cujo recibo se perde após a submissão | Aguardar e repetir a consulta de recibo | Resultado da etapa `unknown`, nunca sucesso; nenhuma classe promovida por tempo, silêncio ou ausência de erro | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-P | ADR-059 / ADR-064 | Grant offline com ação enumerada (abrir incidente), `epoch` comparável e vigente, sem revogação conhecida, contador anti-rollback íntegro, porém **sem fonte de frescor verificável (F1/F2) e sem verificação online (F3)** | Acionar SOS offline | Conforme ADR-064: operação **negada**, com nada aberto e **nada enfileirado**; estado observável na UI e no log informa “modo offline indisponível neste aparelho” com motivo registrado; nenhuma classe de ACK emitida, nenhuma transmissão implícita e nenhuma promessa de persistência. ID e posição originais mantidos; o comportamento passou de positivo a negado pela ADR-064 | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-P2 | ADR-059 / ADR-064 | Mesmo grant enumerado, agora com **F3 disponível**: a verificação online ocorre antes do efeito | Acionar SOS offline com verificação online disponível | Caminho positivo: verificação online executada e registrada antes do efeito; incidente aberto e enfileirado apenas como registro local, com nenhuma classe acima de `persisted_local`, nenhuma transmissão implícita e nenhum prazo exibido como decidido. Caso positivo preservado para o caminho com F3 verificado | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-N | ADR-059 | Grant offline válido apenas para abrir incidente | Pedir divulgação de posição precisa ou mutação federada | Operação negada, sem efeito e com motivo registrado; nenhuma coordenada divulgada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-N2 | ADR-059 | Grant com `epoch` anterior ao último observado, ainda dentro da validade nominal | Acionar SOS com esse grant | Grant recusado antes de qualquer efeito; nenhum incidente criado sob autoridade antiga; motivo registrado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-F | ADR-059 | Restore de snapshot antigo contendo grant nominalmente válido; contador monotônico à frente do snapshot | Tentar usar o grant após o restore | Uso negado por estado anti-rollback / `stale`; incidente não aberto sob o grant restaurado; estado apresentado como desconhecido até nova autoridade | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-059-B | ADR-059 | Limite de frescor do perfil não fixado (`OPEN — a medir`) | Acionar SOS offline com idade do grant indeterminável | Operação negada por frescor desconhecido, com motivo explícito; nenhum prazo ou número apresentado como decidido | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-060-P | ADR-060 | Incidente sem áudio, transcrição ou perna terminadora; perfil de confidencialidade declarado | Consultar o incidente pela API e pela UI | Campo de confidencialidade presente e fiel ao perfil; ausência de alegação de E2EE; ausência de terminação declarada | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-060-N | ADR-060 | Incidente com derivado vindo de bridge/ASR cujo áudio foi terminado | Alegar confidencialidade ponta a ponta sobre esse derivado | Alegação rejeitada pela API; derivado expõe `confidentiality=terminated` de forma visível na UI; nenhum texto prometendo E2EE ou anonimato | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-060-B | ADR-060 | Cópia de incidente sem o metadado de confidencialidade | Ler a cópia | Artefato tratado como `terminated` (falha fechada); estado da ausência informado; nenhuma suposição de E2EE por cache ou versão anterior | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-060-F | ADR-060 | Metadado perdido em restore/replicação de uma cópia | Consultar a cópia restaurada | Cópia apresentada como `terminated` e com metadado ausente declarado; nenhum derivado promovido a protegido | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-061-P | ADR-061 | Serviço de domínio SOS com transação única de banco e `outbox` | Criar incidente | Incidente persistido e evento de outbox na mesma transação; publicação ocorre somente após commit; mesma chave idempotente no objeto e no evento | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-061-N | ADR-061 | Outbox com evento de incidente pendente de publicação | Executar replay do outbox | Projeção reconstruída sem repetir chamada a bridge, rádio, pagamento ou notificação; nenhuma segunda notificação externa emitida | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-061-N2 | ADR-061 | Recepção em Inbox durável separada da projeção (sem transação comum) | Consolidar a recepção do incidente | ACK máximo `persisted_local`; `materialized` não é emitido para essa etapa; UI não exibe projeção aplicada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-061-B | ADR-061 | Dois registros do mesmo `event_id`/efeito no outbox (duplicata por retry do publicador) | Executar replay do outbox | Uma única publicação observada; a duplicata é descartada pela chave idempotente e registrada; nenhum efeito externo repetido e nenhuma segunda entrada de timeline | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-061-F | ADR-061 | Crash entre o commit do incidente e a publicação do evento | Reiniciar o processo e reexecutar o publicador | Incidente permanece persistido; evento publicado uma única vez; `event_id`/deduplicação impede duplicata de tentativa; estado de autoridade `stale`/`unknown` bloqueia mutação forte até reconciliação | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-062-P | ADR-062 | Política da cópia de origem com finalidade e classe declaradas, com prazo e gatilho da ADR-065 (12 meses, `incident_closed + 12m`) e jurisdição BR/CO; revisão jurídica pendente | Acionar SOS | Esperado reformulado com parâmetro ausente: CME-SOS gravada com a política identificada e `persisted_local`; prazo, jurisdição e gatilho reportados como `param_not_configured`; nenhuma expiração automática anunciada; posição precisa, áudio e conteúdo negados | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-062-N | ADR-062 | Nenhuma política aprovada para classe enriquecida (posição precisa, áudio, texto livre) | Tentar gravar a classe enriquecida | Persistência negada com erro explícito; campo ausente do registro e do log; nenhuma gravação improvisada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-062-B | ADR-062 | Política da classe enriquecida ausente, porém a cópia admite a CME-SOS | Acionar SOS | Registro degrada para o piso; degradação informada na UI e no log; nenhuma classe enriquecida persistida | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-062-F | ADR-062 | Escrita negada ou espaço esgotado no armazenamento protegido | Acionar SOS | UI e log com o mesmo veredito “não salvo / não enfileirado”; ação oficial de emergência ainda oferecida; nenhum texto “acionado” | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-062-F2 | ADR-062 / ADR-065 | Cópia específica sem prazo declarado: a tabela da ADR-065 não cobre aquela classe/cópia, que fica sem entrada de política | Consultar histórico de incidente envelhecido naquela cópia | Falha fechada: nenhuma expiração ou descarte automático anunciado ou executado; artefato exibido com a lacuna de política declarada e a cópia bloqueada para uso comum até existir política daquela cópia. Nenhum prazo da ADR-065 é emprestado de outra classe/cópia | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-063-P | ADR-063 | Artefato do cliente (desktop/mobile) com SBOM por release, assinatura válida e `generation` igual ou superior ao piso assinado | Instalar em ambiente limpo e acionar SOS | Instalação aceita; SO assinada, build reproduzível com digest comparado e `generation` registrada; incidente criado com o binário verificado | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-063-N | ADR-063 | Release anterior com vulnerabilidade conhecida assinada e legível | Instalar esse release sobre o piso atual | Rejeição por `generation` inferior ao piso; binário anterior não é substituído; motivo registrado; nenhum downgrade silencioso | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-063-N2 | ADR-063 | Artefato adulterado mantendo o mesmo número de versão | Instalar o artefato | Rejeição por assinatura/digest inválidos antes de qualquer execução; nenhum código do artefato roda | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-063-F | ADR-063 | Chave de assinatura comprometida e lista de revogação publicada | Tentar atualizar com artefato assinado pela chave revogada | Cliente aplica a lista de revogação e rejeita o artefato; atualização só é aceita com chave reemitida; evento registrado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C006-063-B | ADR-063 | Rollback autorizado como exceção limitada no tempo, com piso rebaixado por decisão registrada | Instalar o release rebaixado | Instalação aceita, com autoridade, escopo temporal e auditoria registrados; sem essa decisão o mesmo artefato continua rejeitado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Estes casos complementam as seções de aceite e de casos por decisão, e não as substituem.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-026 | C006-059-P2 (novo; F3 verificado) | C006-059-P (reformulado pela ADR-064: negado, nada enfileirado), C006-059-N, C006-059-N2 | C006-059-B; C006-RF026-B1 (novo) | C006-062-F, C006-062-F2 | Subescopo SOS. Escalonamento, destinatários e cobertura seguem `OPEN`; ACK técnico nunca vira atendimento humano |
| RNF-012 | C006-058-P | C006-058-N | C006-058-B | C006-058-F | Evidência por camada (ADR-058); nome do código de resposta inválida segue `OPEN` em C07/C08 |
| RF-011 | C006-057-P | C006-057-N | C006-057-B | C006-057-F | Apenas a interface de eventos/ACK por etapa; wire, nomes de campo e retenção da chave dependem de C07/C08 |
| RNF-009 | C006-061-P | C006-061-N2 | C006-061-B (novo) | C006-061-F | Interface de sync/outbox; prova de ACK protegido pós-ACK permanece `OPEN` (B06/B12) |
| RF-028, RNF-016 | n/a — pertencem a SPEC-012 e à superfície de experiência | n/a | n/a | n/a | Esta SPEC trata somente a interface SOS de Guardian e de projeção de estado; nenhum caso de superfície é declarado aqui |

Casos novos, descritos em uma linha: **C006-RF026-B1** — acionamento autorizado em canal sem destinatário elegível: registro local com `persisted_local`, ausência de destinatário e de claim exibida em campo próprio, nenhuma escalada criada e nenhuma equipe de plantão presumida. **C006-061-B** — duplicata do mesmo `event_id` no outbox: uma única publicação observada, nenhum efeito externo repetido e nenhuma segunda entrada de timeline.
