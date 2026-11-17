# SPEC-002 — Identidade e autorização

**Versão:** DRAFT-0.6  
**Estado:** DRAFT / OPEN. Não aprovada para implementação.  
**Autora:** Kim Cameron (pod P2 — Identidade)  
**Revisão independente designada:** Bruce Schneier. Revisão da contribuição anterior não equivale a revisão desta versão canônica.  
**Data:** 2026-11-17
**Revisor:** Bruce Schneier (independente; revisão desta versão pendente). Revisão desta DRAFT-0.6 **pendente**; nenhum parecer existente cobre este texto.

## Mudanças desta versão

- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com pelo menos um negativo e um caso de falha/recuperação por decisão aplicável; todos `NOT_EXECUTED` e ID formal `NOT_ASSIGNED`. Nenhum caso foi executado ou revisado.
- Veto C4 sobre ADR-059: nova **§6.3 — Prova anti-rollback e anti-clone do grant offline**. O predicado "estado anti-rollback verificável" deixa de ser autodeclarado pelo cliente: passa a exigir fonte de frescor fora do controle de um cliente restaurado (contador monotônico protegido por hardware quando a plataforma declarar e validar a capacidade, âncora de tempo confiável, ou verificação online obrigatória antes da operação condicional) e alta marca registrada pela autoridade por vínculo. Na ausência de fonte confiável demonstrável, a operação condicional é **negada**.
- Definida detecção de **restauração de snapshot** e de **clonagem de dispositivo** por comparação da evidência de contador/epoch contra a alta marca da autoridade; a evidência local sozinha não distingue as duas ocorrências.
- Distinguidos **"revogação desconhecida"** e **"ausência de revogação"** (§5.1, §6.1, §6.3): apenas a segunda satisfaz o predicado de ADR-059; a primeira equivale a `unknown` e nega.
- §9 ganha os resultados observáveis `rollback_suspected`, `clone_suspected`, `freshness_source_unavailable` e `revocation_state_unknown`, com regra explícita de falha: negar, registrar e orientar ação alternativa; nunca "assumir válido".
- §12 recebe os casos negativos planejados 16–20 (snapshot restaurado, clone com contador divergente, relógio atrasado, offline sem fonte de tempo confiável, revogação desconhecida).
- §11 e §13 registram a capacidade por plataforma e a escolha da fonte de frescor como OPEN. Subseção de ACK (ADR-058) passa de §6.3 para **§6.4**; conteúdo inalterado.
- ADR-059: enumera operações offline locais permitidas, negações invariáveis e predicado condicional de autorização; frescor numérico segue desconhecido até medição.
- ADR-058: exige `ack_class` com uma das seis classes e proíbe inferir entrega ou atendimento.
- Correção G3-02: cabeçalho explicita data, versão, estado e revisor; mantém DRAFT/OPEN.

## 1. Objetivo e autoridade

Esta SPEC define os contratos documentais para identidade tipada, autenticação, autorização por recurso, credenciais, ingresso de nós, rotação, revogação e recuperação de identidade. Ela detalha os requisitos relacionados à identidade sem afirmar que qualquer fluxo está implementado.

A arquitetura de referência e as ADRs prevalecem conforme `docs/README.md`. ADR-034 estabelece GMI imutável UUIDv7 e autenticação OIDC por `iss` + `sub` com WebAuthn/passkeys. ADR-035 estabelece GMC como identificação pública pseudônima padrão e mantém `acct:` como descoberta opt-in; o formato final do callsign ainda precisa de padronização. ADR-036 determina vínculo criptográfico e rejeição de impersonação/replay em mensagens e PTT. Esta SPEC não revoga nem amplia essas direções.

A credencial identifica um principal e pode apresentar escopos, mas o serviço que protege o recurso decide a autorização. Uma conta autenticada, indicativo, domínio ou chave válida isoladamente não concede acesso. O servidor deriva o principal e o contexto de tenant de vínculo validado. Entrada do cliente pode selecionar um tenant entre os vinculados, mas nunca conceder acesso.

## 2. Escopo e rastreabilidade

A matriz contém requisitos aprovados como direção de produto e critérios propostos; seu estado não comprova aprovação desta SPEC nem implementação.

| Requisito | Cobertura documental proposta (não equivale a aceite integral) | Fonte e limite |
|---|---|---|
| RF-001 | Cobertura proposta para identidade tipada e distinção entre usuários, tenants, nós, dispositivos, workloads e gateways; não representa aceite integral do requisito. | `00-produto/requisitos.md:7`; `04-seguranca/seguranca.md`, “Identidades e autorização”; `02-dominio/identidade-global.md`, §§Objetivo e Identidades distintas. Constraints de dados ficam com Michael. |
| RF-015 | Cobertura proposta para separar descoberta, autenticação de peer, aprovação de ingresso, sincronização e readiness; não fecha protocolo de ingresso. | `00-produto/requisitos.md:21`; `01-arquitetura/descoberta-ingresso.md`, §§Estados do nó, Certificado e Falhas. Protocolos e operações concretas do nó ficam com Graydon/Fowler. |
| RF-016 | Parcial, interface de autorização para cada cópia/snapshot/replay. | `00-produto/requisitos.md:22`; `01-arquitetura/nos-regioes.md`; principal documental proposto é Leslie. Esta SPEC não define retenção, residência ou completude de sync. |
| RF-021 | Parcial, aplica principal, tenant, escopo e autorização antes de operações de rádio ou localização. | `00-produto/requisitos.md:27`; `04-seguranca/uso-das-redes.md`; política de operação, licenças, precisão e finalidade é de Bruce/Paul/Radia e decisões do usuário. |
| RF-022 | Parcial, identidade e autorização não alegam E2EE nem autorizam abertura de conteúdo. | `00-produto/requisitos.md:28`; `04-seguranca/seguranca.md`; terminação, gestão de chaves de conteúdo e bridges ficam com Bruce/Radia/Graydon. |
| RNF-002 | Cobertura proposta para identidade estável e rotação/revogação de credenciais, sem aceite integral nem prazo numérico. | `00-produto/requisitos.md:38`; `02-dominio/identidade-global.md`; `07-contratos/identidade-autorizacao.md:108–118`. |
| RNF-003 | Parcial, a decisão de autorização exige tenant, recurso, papel/scope, finalidade, consentimento e ACL. | `00-produto/requisitos.md:39`; `04-seguranca/seguranca.md`, “Isolamento de tenants”; principal documental proposto para o critério global é Bruce. Retenção e auditoria completas não são cobertas aqui. |
| RNF-015 | Interface parcial de principal/tenant para consultas administrativas. | `00-produto/requisitos.md:83`; `06-planejamento/matriz-rastreabilidade.md:144`; Bruce é principal e Jouni cobre Wi-Fi. Esta SPEC não reivindica cobertura do requisito. |
| RNF-017 | Parcial, determina comportamento de autorização sob partição e após reconexão. | `00-produto/requisitos.md:85`; `06-planejamento/matriz-rastreabilidade.md:146`; Martin Kleppmann é principal de sync. Esta SPEC não define garantia de disponibilidade. |

O vínculo de cada requisito a SPEC/teste permanece sob o índice central de Ada. Somente RF-001, RF-015 e RNF-002 têm principal Kim proposto na matriz, ainda sem confirmação individual. Os outros vínculos da tabela são interfaces da família atribuída, não declaração de autoria principal.

## 3. Termos, identidades e dados

### 3.1 Principais tipados

| Tipo lógico | Identificador | Credencial/prova esperada | Limite |
|---|---|---|---|
| Pessoa | `gmi_id` UUIDv7 | Login OIDC validado e vínculo `(iss, sub)`; prova de posse conforme operação | GMC ou nome não substitui GMI nem concede acesso. |
| Organização | `tenant_id` UUIDv7 | Associação/domínio validado e autoridade do tenant | Domínio DNS, nome comercial ou campo do cliente não autoriza membership. |
| Nó | `node_id` UUIDv7 | Chave persistente, certificado/convite autorizado e prova de posse | Descoberta ou handshake não concede ingresso nem leitura privada. |
| Dispositivo | `device_id` UUIDv7 | Chave própria e prova de associação/posse | IMEI, DevEUI, endereço BLE ou fabricante não prova titularidade. |
| Workload/serviço | ID tipado de serviço | Credencial de workload com audience e scopes próprios | Não reutiliza credencial ou scope humano. |
| Gateway | ID tipado do gateway | Credencial de serviço/dispositivo e autorização por bridge/recurso | Não herda autorização do peer ou da rede externa. |

`gmi_id` é imutável e não reciclável. `gmc_callsign` é pseudônimo público estável conforme ADR-035; não contém identidade civil, não prova posse e não é segredo. `acct:` é alias opt-in para descoberta. O `gmi_id` não é identificador público padrão em mensagens/PTT. A forma final do callsign e seus algoritmos permanecem pendentes.

### 3.2 Entidades lógicas da credencial

Esta estrutura é semântica, ainda não wire schema. Campos de assinatura não definem por si política criptográfica.

- **Vínculo externo:** `issuer` (`iss`) e `subject` (`sub`) autenticados, ligados a um único principal GodMesh tipado segundo autoridade validada. E-mail, `preferred_username`, handle e nome não são chave estável.
- **Credencial:** referência, titular tipado, emissor, finalidade, audience, chave/identificador de chave, validade, scopes, estado e epoch/escopo quando necessários.
- **Grant:** autorização limitada a principal, tenant/recurso, operação, finalidade, audience e condições. Não permite inferir operações não incluídas.
- **Prova de posse:** desafio não reutilizável, chave vinculada e finalidade. Provar posse de chave não substitui membership, consentimento ou ACL.
- **Estado de validação:** `valid`, `expired`, `revoked`, `unknown`, `stale` ou `key_unknown`. Estado de validação não é estado de evento.

`authority_epoch`, `dispatch_epoch`, `event_sequence` e cursor são namespaces distintos. Um número sem autoridade, escopo e regra de comparação definidos não é prova de atualidade.

## 4. Atores e autoridades

- **Autoridade de identidade:** valida vínculo `(iss, sub)` a GMI, emite/revoga credenciais de identidade e registra transições autorizadas. Não decide ACL de todo recurso.
- **Autoridade de tenant/recurso:** mantém membership, papel, scope e ACL dentro de seu escopo.
- **Autoridade de federação:** autoriza ingresso, limita e revoga certificados de nós. Não concede escopo que não esteja no certificado.
- **Serviço de recurso:** valida credencial e estado necessários, deriva tenant no servidor e aplica a decisão antes do efeito. Destino federado valida novamente o que recebe.
- **Cliente/principal:** apresenta credencial e prova; pode indicar seletor de tenant vinculado, que o servidor verifica. Não fornece autoridade por campos de body/path/query.
- **Operador/admin:** atua somente com principal, tenant, papel, finalidade e escopo autorizados. MFA para operadores é direção existente em `04-seguranca/seguranca.md`; fatores, provedor e recuperação dependem de decisão/integração.

### 4.1 Derivação do tenant e autorização

O servidor valida o principal e resolve seus vínculos no contexto autenticado. Um seletor do cliente escolhe entre vínculos previamente permitidos; o servidor pode consultar membership protegido para verificar vínculo vigente, recurso e ACL. Ausência de seletor com múltiplos contextos não usa tenant padrão ambíguo. Tenant inexistente, não vinculado, revogado ou de outro principal falha fechado; consultas internas protegidas podem ocorrer, mas resposta e efeito não revelam existência a quem não tem autorização.

GMP v1 `§7.1` declara que `tenant_id` e `principal_ref` são resolvidos/validados pelo servidor e que claims/body/path/query não substituem membership, ACL, propósito ou autorização atual (`07-contratos/gmp-api-v1.md:376`). O caso negativo de `:520–530` rejeita `tenant_id` no body que tente elevar acesso. Estes contratos são propostas, ainda não wire final aprovado.

### 4.2 Decisão por operação

O autorizador avalia, nesta ordem lógica, os itens definidos em `identidade-autorizacao.md:138–148`:

1. Versão, formato, tamanho e audience da requisição.
2. Principal, prova e cadeia do emissor; estado e validade da credencial.
3. Vínculo tipado, tenant e recurso derivados no servidor.
4. Scope mínimo, membership e ACL.
5. Finalidade, consentimento, destinatário e janela temporal quando exigidos.
6. Estado atual, epoch e regra de partição da classe de operação.
7. Autorizações independentes de saída e entrada em bridge/federação.
8. Resultado mínimo e auditoria sem token, chave, coordenada ou conteúdo protegido.

Escopo selecionado pelo cliente nunca substitui autorização. Erros não enumeram identidade, dispositivo, canal, tenant ou localização de terceiro.

## 5. Máquina de estados

### 5.1 Credencial e validação

Estados de credencial: `active`, `suspended`, `revoked`. Resultado de validação: `valid`, `expired`, `revoked`, `unknown`, `stale`, `key_unknown`. São os valores propostos por `identidade-autorizacao.md:238–245`, não enums wire ratificados.

| Resultado | Leitura pública não sensível | Ação privada ou nova emissão/grant | Ingresso/sync privado, localização precisa, migração |
|---|---|---|---|
| `valid` | Aplicar ACL e finalidade | Avaliar operação, vínculo e limites | Exigir autoridade e consentimento da operação. |
| `expired` ou `revoked` | Somente conteúdo realmente público; não revelar dados protegidos | Negar | Negar, bloquear novas sessões/efeitos cobertos e reconciliar revogação. |
| `unknown` ou `stale` | Nega privado; cache público só sem alegar atualidade | Negar novo grant e operação sensível. Grant local só autoriza ação previamente aprovada e enumerada que não contrarie negativas vigentes; não torna o estado atual | Negar ingresso, replicação privada, localização precisa e migração. |
| `key_unknown` | Negar | Negar | Negar. |

`unknown` significa que a autoridade não confirmou o estado. `stale` significa que a prova local superou o limite de idade configurado. **"Revogação desconhecida" não é "ausência de revogação".** Somente há ausência de revogação quando existe evidência positiva cobrindo o escopo e o instante da decisão (lista/estado assinado pela autoridade dentro do limite de frescor aprovado para a classe, ou consulta online no momento da operação, no epoch comparável). Ficha local sem nenhuma revogação registrada, sem evidência que cubra o período, ou com evidência vencida/incomparável, é revogação **desconhecida**: resultado `unknown`, que nega a operação condicional (ver §6.3). Até a política aprovar esse limite por classe, a prontidão de produção fica bloqueada. Assinatura válida comprova integridade/origem segundo a confiança configurada, não ausência de revogação posterior. Grant `valid` não sobrepõe revogação conhecida no destino: não autoriza efeitos futuros no escopo revogado. Sessões e operações em voo dependem de protocolo e autoridade ainda OPEN.

### 5.2 Ingresso de nó

Estados base registrados em `descoberta-ingresso.md`: `initializing`, `discovering`, `awaiting_authorization`, `joining`, `synchronizing`, `ready`, com `degraded`, `isolated` e `revoked`. A sequência e transições específicas são propostas de arquitetura, a conciliar com GMP/C10.

- Descoberta identifica candidato; não concede confiança.
- Handshake autenticado prova controle de chave/peer; não autoriza dados privados.
- Certificado/convite precisa vincular `node_id`, emissor, audience, validade, escopos de tenant/região/capacidade, `key_id`, epoch e assinatura conforme perfil aprovado.
- Nó não amplia os próprios scopes pelo manifesto que apresenta.
- Sem certificado autorizado, nó fica em `awaiting_authorization`/`isolated`; não recebe snapshot privado nem anuncia readiness federada.
- Após aprovação, destino valida a credencial antes de ingresso e novamente antes de sync privado. `ready` depende da readiness real definida pelas SPECs de operação; não prova sincronização integral.
- Revogação conhecida bloqueia novas sessões e cópias sob controle do destino. Nós sem conexão permanecem `unknown/pending`; não há promessa de propagação instantânea.

### 5.3 Migração e recuperação

Migração normal começa com prova da chave atual, pedido ligado ao novo emissor e confirmação conforme política. O início do pedido não transfere autoridade. A troca exige vínculo novo, autoridade de escrita/fencing e epoch crescente, e invalidação do anterior sob contrato de autoridade. Se houver resultado desconhecido, novas mutações são bloqueadas nos dois emissores para o efeito dependente até consulta autoritativa.

Recuperação sem emissor anterior aceita somente provas previamente cadastradas e válidas conforme política. A direção escolhida pelo usuário não aprova via assistida, recuperação de chave de conteúdo, classes/quantidade de provas, contestação, adjudicador ou prazo.

Rótulos de estado candidatos e não aprovados: `requested`, `proof_pending`, `recovery_pending`, `contested`, `approved`, `rejected`, `transferred`, `closed`. `contested` é proposta, não estado ratificado. Conflito, autoridade indisponível ou evidência incompleta bloqueiam transferência; nenhum timeout aprova. Durante estado pendente, preservar vínculo atual e bloquear alteração administrativa sensível. Recuperar identidade não recupera chaves nem conteúdo E2EE.

### 5.4 Transferência e exclusividade

A transferência de identidade/autoridade não é a mesma coisa que corte de sync, fencing no efetor ou confirmação de revogação em destino. Para cada transição, a SPEC exige identificar tenant/partição, emissor antigo e novo, `authority_epoch`/namespace, inventário de efetores, operações em voo e evidência por destino.

A transição só pode ser considerada completa quando a fronteira única for realmente verificável e abranger todos os efeitos relevantes, ou quando o protocolo recuperável demonstrar corte/fence nos efetores aplicáveis e reconciliar resultados desconhecidos. A inexistência de recibo não prova inexistência de efetor. Restore sem histórico completo continua isolado. Essa regra é alinhada à crítica de Kim §19 e à proposta de transferência de Alan, ainda não aprovada.

Ledger de autoridade e recibos autenticados por destino são evidências complementares. O inventário deve declarar alcance e incompletude; sem prova de cobertura, não declarar transferência completa. Se o resultado for desconhecido, os dois emissores permanecem bloqueados para mutações dependentes.

## 6. Operações e grants offline

### 6.1 Matriz de autorização mínima

| Operação | Condição de identidade/autorização | Com `stale`/`unknown` ou partição | Estado |
|---|---|---|---|
| Leitura de recurso público | Sem revelar dados privados; política pública permite | Pode servir cache público sem alegar freshness | Política por recurso ainda precisa contrato. |
| Leitura privada | Principal, tenant, membership, scope, ACL e finalidade vigentes | Negar quando validade/autoridade for desconhecida | Especificado como proposta. |
| Novo grant/alteração de ACL | Autoridade atual e permissão administrativa | Negar | Sem exceção offline aprovada. |
| Ingresso e sync de nó | Certificado/convite válido, scopes e epoch atuais | Negar snapshot/sync privado | Sem limite de frescor aprovado. |
| Migração/recuperação | Prova atual ou política de provas prévias aprovada, autoridade e fence | Negar/pendente | Procedimento excepcional não aprovado. |
| Localização precisa, RF/TX, PTT externo, federação | Grant/finalidade/consentimento e autoridade específica por domínio | Negar | Domínios/autores devem fechar os próprios contratos. |
| Abrir incidente SOS local e enfileirar | Grant local válido e predicado condicional abaixo | Negar se qualquer predicado não puder ser demonstrado | ADR-059; execução/entrega não inferidas. |
| Check-in de sessão Guardian existente | Grant local válido e predicado condicional abaixo | Negar se qualquer predicado não puder ser demonstrado | ADR-059; não cria sessão. |
| Enviar mensagem a canal já autorizado | Grant local válido e predicado condicional abaixo | Negar se qualquer predicado não puder ser demonstrado | ADR-059; apenas canal previamente autorizado. |
| Registrar observação de localização local | Grant local válido e predicado condicional abaixo | Negar se qualquer predicado não puder ser demonstrado | ADR-059; não divulga localização. |
| PTT apenas em rede local com grant de floor válido | Grant local válido e predicado condicional abaixo | Negar se qualquer predicado não puder ser demonstrado | ADR-059; sem RF/transmissão externa. |

**Negações sempre aplicáveis ao grant offline (ADR-059):** divulgar localização precisa; mutação federada; criar ou alterar grant; mudar identidade; transmitir por RF; executar transcrição remota. Essas operações não são habilitadas por frescor, cache, grant ou disponibilidade parcial.

**Predicado condicional (ADR-059):** além da operação estar na lista permitida, exigir `epoch` comparável na autoridade/namespace/escopo corretos, frescor dentro do limite do perfil, ausência de revogação conhecida e estado anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. O valor numérico do limite de frescor é **desconhecido** até medição de uso legítimo; não configurar valor presumido.

Cada cláusula deste predicado tem leitura própria e nenhuma é presumida verdadeira por ausência de evidência contrária: "ausência de revogação conhecida" exige evidência positiva conforme §5.1 e §6.3 (revogação desconhecida nega); "estado anti-rollback verificável" exige fonte de frescor fora do controle do cliente e comparação com a alta marca da autoridade conforme §6.3; "frescor dentro do limite do perfil" exige fonte de tempo confiável, não o relógio arbitrário do dispositivo. Se qualquer cláusula não puder ser demonstrada no instante da decisão, a operação é negada.

### 6.2 Campos e validade do grant

Perfil lógico do grant: emissor/chave confiáveis, titular tipado, tenant/recurso, operação exata, finalidade, audience/efetor, validade, versão de política, limites de uso, vínculo de chave quando aplicável e epoch acompanhado de autoridade/namespace/escopo. Este é o conjunto semântico proposto para evitar ampliação de escopo; formato e campos finais precisam concordância com Graydon/C10/C06.

A validação exige separar assinatura, validade temporal e autorização atual. Prova stale, epoch antigo/incomparável, relógio incerto, política sem limite aprovado ou autoridade desconhecida não autorizam ação sensível. Revogação conhecida no destino prevalece sobre grant/lease; lease não permite nova divulgação depois que o destino conhece a revogação. Offline não cria nova autoridade, ACL, réplica ou efeito federado.

O conjunto de operações locais e as negações acima seguem ADR-059. Duração, valor numérico de frescor, mecanismo de relógio e risco residual permanecem OPEN; nenhum TTL é presumido. Sem esses parâmetros medidos e definidos por perfil, não declarar prontidão de produção.

### 6.3 Prova anti-rollback e anti-clone do grant offline

O predicado "estado anti-rollback verificável" da ADR-059 é indefinido enquanto a única evidência for um contador que viaja junto com o estado que se quer revogar. Contador em armazenamento comum do aplicativo é restaurado pelo backup, pela imagem de fábrica ou pelo snapshot da máquina virtual exatamente na mesma granularidade do grant: o dispositivo restaurado apresenta estado internamente consistente, assinatura válida, epoch coerente com o que gravou e um contador que ele nunca deixou de aumentar do próprio ponto de vista. Isso não é prova de atualidade; é replay com boa aparência. Esta subseção define o que conta como prova.

**Regra de ordem.** A evidência que sustenta o predicado precisa estar fora do controle de quem pode restaurar o estado. Fontes aceitas, das quais a operação condicional exige pelo menos uma demonstrável no instante da decisão:

| Fonte | Descrição | Condição para valer | Estado |
|---|---|---|---|
| F1 Contador monotônico protegido por hardware | Contador que software não decrementa nem reescreve e que sobrevive a restauração de imagem, com evidência autenticada do valor. | A plataforma precisa declarar e validar a capacidade, e a evidência do valor precisa ser verificável pela autoridade. Ausência de suporte não é suporte equivalente. | Capacidade por SO/hardware **OPEN** (§11); nenhum mecanismo foi escolhido. |
| F2 Âncora de tempo confiável | Instante obtido de fonte que avança de forma monotônica e autenticada desde a última observação validada pela autoridade, e não do relógio arbitrário do dispositivo. | A relação entre a âncora e o limite de frescor do perfil precisa estar dentro de política aprovada por classe; o relógio local é comparador, nunca autoridade. | Fonte, precisão e tratamento de desvio **OPEN**; valores numéricos não escolhidos. |
| F3 Verificação online obrigatória | Antes da operação, o cliente obtém da autoridade evidência atual do escopo: epoch, estado de revogação e alta marca. | Só existe havendo conectividade e resposta verificável da autoridade do escopo. Timeout, resposta parcial ou resultado ambíguo **não** contam como confirmação. | Contrato por operação ainda não definido (GMP/autoridade). |

**Alta marca por vínculo.** A autoridade de identidade/grant registra, para cada `(principal_ref, device_id, key_id, escopo do grant)`, o maior valor de contador ou epoch já observado, consumido monotonicamente e só crescente. A emissão e o uso de grant offline carregam e avançam essa marca. A alta marca é a referência contra a qual a evidência local é julgada; ela não vive no dispositivo avaliado.

**Detecção de restauração de snapshot.** Considera-se restauração quando a evidência apresentada para o vínculo é estritamente anterior à alta marca registrada pela autoridade, ou quando o dispositivo, estando isolado, não consegue apresentar valor adiante do capturado na imagem porque não dispõe de F1 nem de F2. No primeiro caso a divergência é provada na reconciliação em F3; no segundo a prova simplesmente não existe e a cláusula anti-rollback falha. Falha em demonstrar é negativa, não espera neutra.

**Detecção de clonagem.** Considera-se clone quando o mesmo material de chave/contador é apresentado para o mesmo escopo por dois dispositivos com valores sobrepostos ou divergentes, sejam em uso concorrente observado online, sejam em históricos que se contradizem na reconciliação. Evidência local não distingue clone de restore: as duas ocorrências produzem o mesmo estado interno. A distinção vem da comparação com a alta marca e do vínculo entre `key_id`, `device_id` e a prova de posse, na autoridade. Enquanto o vínculo estiver sob suspeita de clone, negam-se as operações condicionais cobertas por aquela chave e exige-se revalidação; bloqueio, revogação e reemissão seguem §7, sem ampliar decisão de autoridade de terceiros.

**Ausência de fonte confiável.** Sem F1 verificada, sem F2 dentro de política aprovada e sem F3 disponível, a operação condicional é **negada**. Essa é a decisão padrão, não uma exceção: o custo de negar uma ação local reversível é menor que o de autorizar replay de um grant cuja revogação o dispositivo nunca chegou a conhecer. A negativa se repete a cada tentativa; não existe "autorizar uma vez enquanto não houver fonte".

**Revogação desconhecida versus ausência de revogação.** O predicado de ADR-059 é satisfeito apenas por **ausência de revogação**, isto é, evidência positiva e vigente cobrindo principal, chave, tenant/recurso e operação (F3 no instante da decisão, ou material de estado de revogação assinado pela autoridade, no epoch comparável e dentro do limite de frescor aprovado para a classe). **Revogação desconhecida** — ficha local sem registros, janela não coberta por evidência, material vencido, epoch incomparável, ou autoridade não consultável — é o resultado `unknown` de §5.1 e **nega** a operação condicional. Nunca inferir ausência a partir de desconhecimento; a expressão "nenhuma revogação conhecida" não autoriza nada sozinha.

**Comportamento de falha.** Toda negativa por esta subseção é observável: resultado explícito de §9 (`rollback_suspected`, `clone_suspected`, `freshness_source_unavailable`, `revocation_state_unknown`), decisão mínima registrada conforme §10, e orientação de ação alternativa ao usuário — buscar conectividade e revalidar com a autoridade, sincronizar o estado de revogação, reemitir grant para o dispositivo, ou usar um canal que não dependa de grant offline. É proibido: assumir válido por falta de evidência contrária, liberar a operação após tentativas sucessivas, registrar sucesso quando o resultado foi negativa, ou degradar silenciosamente para "permitir e reconciliar depois". Nenhum efeito externo ou federado é produzido pela tentativa negada.

**Risco residual.** Um dispositivo completamente isolado, sem F1 validada e sem F2 aprovada, não pode provar que não foi restaurado; o que se elimina aqui é a autorização indevida, não a possibilidade do evento. Este texto não escolhe mecanismo, biblioteca, API de plataforma, algoritmo nem qualquer valor numérico de frescor, tolerância de relógio ou janela de reconciliação.

### 6.4 ACK por fronteira (ADR-058)

Toda resposta de API que represente confirmação carrega `ack_class`, obrigatoriamente uma destas classes: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device`, `acknowledged_human`. Cada classe exige emissor, evidência e escopo próprios. É proibido usar “ACK” sem classe ou inferir classe superior por passagem de tempo, sucesso de camada inferior ou ausência de erro. Entrega exige evidência do dispositivo destino; atendimento exige evidência humana. Uma confirmação técnica nunca implica entrega ou atendimento.

## 7. Rotação, suspensão e revogação

1. Manifesto de confiança versionado identifica `kid`, algoritmo, chave pública, finalidade, validade e geração. Chave privada não aparece em token, manifesto público ou log.
2. Rotação publica a nova chave antes do uso. Sobreposição e janela de convivência aguardam aprovação. A chave nova prova vínculo ao titular/autoridade.
3. `suspended` e `revoked` têm efeito definido por tipo de credencial e operação; revogação impede novas emissões, grants e renovações. Sessões em curso e próxima publicação/cópia exigem contrato por serviço.
4. Revogação de credencial, grant e consentimento são eventos distintos. Destino que confirma revogação nega a próxima unidade de efeito coberta. Worker/cliente desconectado não pode ser declarado cortado sem evidência.
5. Cada destino reporta estado separado por cópia e dimensão: recebimento, bloqueio de acesso, disposição/eliminação ou limite fora de controle, com autoridade, instante e evidência; pendente/desconhecido não equivale a confirmado. Um recibo de recebimento não confirma bloqueio ou descarte. Emissão de evento não equivale a aplicação em todos destinos; seguir o contrato de disposição C08/C10 sem criar enums concorrentes.
6. Reconexão consulta autoridade, aplica eventos idempotentemente, atualiza epoch e revalida antes de abrir acesso ou executar fila. Replay não reativa credencial revogada nem amplia prazo original.
7. Se autoridade, histórico, fence ou prova por destino forem desconhecidos, bloquear o efeito dependente sem bloquear fluxo comprovadamente independente.

A RFC 7009 reconhece possível atraso de propagação entre servidores; a documentação GodMesh não presume revogação instantânea. `max_credential_lifetime`, `max_revocation_staleness` e `max_offline_grant_lifetime` são parâmetros por classe sem valores aprovados.

## 8. Recuperação e proteção de provas

- Provas de recuperação são cadastradas antes da perda de acesso, enquanto titularidade e autoridade atuais são verificáveis. Um pedido de recuperação não cadastra o fator que aprovará esse mesmo pedido.
- Prova apresentada fica vinculada a titular, classe, emissor, finalidade, desafio/nonce, pedido e transição. Repetição é rejeitada/consumida segundo política aprovada.
- Prova não cadastrada, expirada, reutilizada, comprometida ou conflitante não transfere identidade. Suspeita de compromisso exige bloquear fator conforme regra a decidir; sem outra prova válida, negar.
- Disputa não transfere autoridade. Rótulos `contested`/`approved` não são decisão aprovada nem substituem adjudicação definida.
- Armazenar somente referência e resultado mínimo, não segredo/prova reutilizável. Recuperação não restaura conteúdo de terceiros nem chaves de conteúdo.

Classes e quantidade de provas, independência, política de compromisso, notificação, contestação, autoridade adjudicadora, auditoria, retenção e tratamento de pessoas sem fator alternativo continuam OPEN. Alternativas: negar toda recuperação excepcional até provas válidas ou aprovar processo formal de contestação com autoridade, janela e controles definidos. Não há autorização para via assistida.

## 9. Erros, falhas e recuperação

| Resultado conceitual | Causa | Comportamento exigido |
|---|---|---|
| `invalid_token` | Assinatura, emissor, audience ou validade inválida | Negar antes de qualquer efeito; não enumerar outro sujeito. |
| `invalid_proof` | PoP, desafio, vínculo ou assinatura inválidos | Negar; não revelar se outro principal existe. |
| `insufficient_scope` / `permission_denied` | Scope, membership, ACL, finalidade ou consentimento insuficientes | Negar com detalhe mínimo e nenhum side effect. |
| `tenant_context_invalid` | Seletor ausente/ambíguo ou não vinculado | Negar sem revelar se tenant/recurso existe. Mesmo com múltiplos tenants ou membership revogado, consultas internas protegidas podem resolver vínculo; resposta externa e efeito devem ser indistinguíveis segundo GMP. Nome wire/status final depende de GMP. |
| `authority_unavailable` | Confiança, revogação, membership, epoch ou chave não podem ser validados | Falhar fechado nas operações privadas/sensíveis; não fazer downgrade. |
| `stale_epoch` | Epoch não aceito para autoridade/escopo | Negar; não comparar com sequence/cursor de outro domínio. |
| `recovery_pending` / `contested` | Prova ou decisão incompleta/conflitante | Rótulos candidatos; manter titular/vínculo anterior e bloquear transferência. |
| `authority_transfer_unknown` | Queda ou resultado ambíguo entre fence/commit | Bloquear mutações de ambos emissores para o efeito afetado, consultar ledger/autoridade e retomar idempotentemente; nunca inferir sucesso/rollback pelo timeout. |
| `restore_history_incomplete` | Snapshot sem revogações/epochs/cortes completos | Isolar cópia e negar operação protegida até reconciliação. |
| `rollback_suspected` | Evidência de contador/epoch do dispositivo anterior à alta marca da autoridade para o mesmo vínculo, ou impossível de avançar após restauração (§6.3) | Negar a operação condicional sem nenhum efeito; orientar reconexão e revalidação. Jamais assumir válido. |
| `clone_suspected` | Mesmo material de chave/contador usado por mais de um dispositivo para o mesmo escopo, com valores sobrepostos ou divergentes (§6.3) | Negar sob aquela chave, bloquear efeitos dependentes do vínculo e encaminhar revalidação/revogação conforme §7; não punir outros vínculos do principal. |
| `freshness_source_unavailable` | Nenhuma fonte de frescor confiável disponível no instante da decisão (sem F1 verificada, sem F2 aprovada, sem F3) | Negar a operação condicional (§6.3) e informar a ação alternativa. Indisponibilidade da fonte nunca é permissão. |
| `revocation_state_unknown` | Estado de revogação do escopo não confirmado pela autoridade | Tratar como `unknown` de §5.1 e negar; revogação desconhecida não equivale a ausência de revogação. |

Os nomes acima são rótulos lógicos propostos, não códigos wire aprovados. Graydon define envelope, status HTTP e mapeamento de erro; C10 define persistência/replay; C06 autoridade/fence; C08 disposição de dados.

## 10. Privacidade e auditoria

- GMI é interno; comunicação apresenta GMC/alias autorizado. WebFinger e resolução pública expõem apenas aliases publicados opt-in e metadados mínimos.
- Não usar e-mail, telefone, IP, callsign, DevEUI ou handle como chave primária de pessoa ou prova de titularidade.
- Não registrar token, chave privada, prova reutilizável, coordenada, conteúdo ou claims de tenant sem finalidade.
- Registrar mudança de vínculo, emissão/revogação, decisão e consulta administrativa com referências opacas, escopo, versão de política e motivo sanitizado, sujeito a acesso e retenção aprovados.
- Exigir papel, tenant e finalidade para consultar auditoria. Negar escopo indevido sem confirmar a existência de pessoa, visitante ou prova.
- Esquema/campos mínimos, correlação, papéis, acesso, retenção e confirmação por cópia permanecem OPEN; ausência de evento não prova cobertura integral.
- Autorização de identidade não torna o conteúdo E2EE nem autoriza ASR, gravação, localização ou transmissão RF.

Papéis, dados de auditoria, jurisdição, retenção por cópia, acesso excepcional e descarte continuam OPEN com Bruce, Lawrence, Jouni quando há dados Wi-Fi e decisão do usuário.

## 11. Plataformas e modalidades

A semântica de identidade e autorização é comum. Cada plataforma precisa demonstrar capacidades de armazenamento de chave, ciclo de vida, relógio, autenticação, proteção de credenciais e revalidação. Nenhuma linha abaixo implica suporte de API/hardware verificado.

| Plataforma/produto | Aplicabilidade | Critério específico / pendência |
|---|---|---|
| Servidor em contêiner Linux / GodMesh Node | Aplicável a autoridades, validadores, serviços e nós federados | Validar credencial, tenant e autorização no servidor/destino; isolamento de chave, restore e fencing dependem de SPECs de operação/dados. |
| macOS arm64/x86_64 | Aplicável como cliente desktop e operador autorizado | Mesmo contrato; armazenamento seguro e interação OIDC/passkey dependem de APIs/versões ainda não homologadas. |
| iOS | Aplicável como cliente | Revalidar após suspensão/resume e mudanças de rede; capacidade de chave/entitlement/background é OPEN com Donny. |
| Windows | Aplicável como cliente | Contrato idêntico; interface e proteção de credencial são OPEN com Lea/Fowler. |
| Linux desktop | Aplicável como cliente | Contrato idêntico; armazenamento de segredo e ciclo de sessão por ambiente é OPEN. |
| Android | Aplicável como cliente | Revalidar em mudança de processo/rede; Keystore/biometria/lifecycle precisam análise oficial por Chet. |
| Web | Aplicável à autenticação/cliente compatível, se oferecido | Não recebe papel de roteador/autoridade permanente; WebAuthn/OIDC e proteção de token precisam fluxo/browser compatíveis definidos. |
| Personal, Teams, Platform/SDK | Aplicável conforme principal, tenant, escopo e produto | Perfil e políticas de produto não mudam verificação criptográfica nem ACL. |
| Bridges externas | Aplicável apenas quando explicitamente autorizadas | Autorização distinta na saída e entrada; identidade/proveniência preservadas pela UDNL; sem herdar permissão de rede externa. |

A capacidade de sustentar F1 ou F2 da §6.3 — contador monotônico protegido por hardware e âncora de tempo confiável — é declarada por plataforma como **OPEN**: nenhum SO, API, versão ou hardware foi verificado, e a ausência de suporte comprovado resulta em negativa da operação condicional, não em concessão.

Sem APIs e versões mínimas escolhidas, compatibilidade real permanece não verificada. Não se declara N/A global para nenhuma plataforma do escopo do produto; operações exclusivas de servidor ficam sem superfície direta no cliente, mas seus clientes dependentes continuam aplicáveis.

## 12. Critérios de aceite e casos planejados

Os cenários abaixo são planejados e não executados. São condições para futura validação; não constituem plano executável completo nem casos com IDs atribuídos.

1. **Vínculo OIDC positivo.** Dado `(iss, sub)` autenticado e vinculado, o recurso resolve o GMI correspondente e permite somente escopo/tenant/ACL/finalidade vigentes.
2. **Identificador textual falso.** Apresentar GMC, handle, e-mail ou GMI sem prova válida não cria vínculo nem autoriza acesso.
3. **Tenant cruzado.** Principal do tenant A pede recurso de B via body/path/query. Servidor pode consultar vínculo/membership em estado interno protegido, mas nega o efeito e não revela externamente se B ou o recurso existe. Seleção de B por principal com vínculo a A e B só funciona após validação atual de membership/ACL.
4. **Principal de tipo errado.** Workload com scope humano ou dispositivo usando credencial de usuário é negado antes de qualquer efeito.
5. **Credencial e chave.** `expired`, `revoked`, `key_unknown`, assinatura inválida, audience incompatível ou algoritmo fora da allowlist são negados. Algoritmos/allowlist ainda OPEN, então esse caso não define escolha criptográfica.
6. **Rotação.** Nova chave passa somente após manifesto e vínculo válidos; chave antiga não ganha validade adicional por rotação. Falhas de publicação deixam estado explícito e não emitem grant novo.
7. **Revogação em sessão longa.** Após confirmação no destino, próxima unidade de efeito coberta é negada; operação em curso fica `interrupted`, `completed_before_revocation` ou `outcome_unknown` conforme evidência definida. Destino desconectado segue `unknown/pending`.
8. **Ingresso.** Descoberta e handshake sem convite/certificado autorizado mantêm nó isolado; não publicam readiness nem recebem snapshot privado. Credencial válida libera somente scopes aprovados.
9. **Partição/offline.** `unknown/stale` nega operações privadas/sensíveis. Grant só permite operação local reversível quando operação, frescor e limites estiverem aprovados; revogação conhecida no destino prevalece sobre lease.
10. **Migração concorrente.** Falha entre novo vínculo, fence e revogação antiga bloqueia mutações de ambos emissores; retry idempotente não transfere duas vezes; timeout não declara sucesso/rollback.
11. **Provas de recuperação.** Prova ausente, não cadastrada, expirada, reutilizada, comprometida ou de titular errado não transfere identidade. Conflito bloqueia; `contested` permanece proposta até decisão.
12. **Restore.** Snapshot anterior a revogação inicia isolado. Sem histórico completo e estado atual por autoridade/escopo, não serve dado privado nem reabre mutação.
13. **Anti-enumeração e auditoria.** Consulta cross-tenant ou sem papel/finalidade é negada sem revelar identidade; registro não contém segredo, prova reutilizável, conteúdo ou coordenada.
14. **Bridge/federação.** Autorização de saída sem autorização de entrada não publica; atributos não suportados não criam identidade nem scope e perda é reportada pela UDNL.
15. **Plataformas.** Repetir fluxos aplicáveis em servidor Linux, macOS arm64/x86_64, Windows, Linux desktop, iOS, Android e web onde a função existir. Cada resultado registra versão/SO/fluxo; suporte e ambiente não estão validados.
16. **Snapshot restaurado com grant na validade (negativo).** Restaurar o dispositivo para uma imagem anterior cujo grant, assinatura e epoch são válidos e aparentemente frescos do ponto de vista local. A evidência de contador é anterior à alta marca da autoridade para o mesmo vínculo; a operação condicional é negada com `rollback_suspected`, sem nenhum efeito local ou externo, e a reapresentação repetida não muda o resultado. Se o dispositivo estiver isolado e sem F1/F2, o resultado é `freshness_source_unavailable` pela mesma razão: prova inexistente.
17. **Clone com contador divergente (negativo).** Dois dispositivos usam o mesmo `key_id`/material de grant para o mesmo escopo, com valores sobrepostos ou divergentes. A reconciliação na autoridade detecta o conflito, nega sob aquela chave com `clone_suspected`, bloqueia os efeitos dependentes do vínculo e não propaga a negativa a outros vínculos válidos do principal.
18. **Relógio atrasado (negativo).** Ajustar o relógio local para trás, mantendo grant dentro da validade nominal. O relógio local não é fonte de frescor: sem âncora confiável (F2) ou verificação online (F3) que corrobore, a operação é negada; não basta o valor interno do relógio estar coerente com o `not_before`/`not_after` do grant.
19. **Offline sem fonte de tempo confiável (negativo).** Operação condicional sem F1 validada na plataforma, sem F2 aprovada e sem conectividade para F3. Todas as cinco operações enumeradas em §6.1 são negadas com `freshness_source_unavailable`, de forma observável e com orientação de ação alternativa; nenhuma é liberada por "não haver evidência de revogação".
20. **Revogação desconhecida (negativo).** Dispositivo cuja ficha local não contém nenhum registro de revogação, mas também nenhuma evidência positiva vigente cobrindo o escopo e o instante (material vencido, epoch incomparável ou autoridade não consultável). O resultado é `revocation_state_unknown`, tratado como `unknown`, e a operação é negada; a ausência de registros locais nunca é lida como ausência de revogação.

## 13. Dependências, decisões OPEN e alternativas

| Decisão/dependência | Dono | Alternativas e consequência | Critério de fechamento |
|---|---|---|---|
| Token API: JWT RFC 9068 ou token opaco/introspecção; claims, audience e cache | Graydon + Kim | Verificação local melhora disponibilidade, mas depende de chave/cache e janela de revogação; introspecção central reduz estado local, mas exige autoridade disponível. | Contrato GMP, identidade e erro concordam; casos de revogação/partição definidos. |
| Perfil de credencial: JWS ou COSE; algoritmo/chaves | Kim + Bruce + Graydon | JWS proposto por coerência JSON/OIDC; COSE pode atender perfis CBOR. ES256/EdDSA são candidatos na fonte, não aprovados. | Revisão de segurança e compatibilidade por plataforma; allowlist, rotação, validação e interop especificadas. |
| Autoridade de escrita, epoch, transferência/fencing e inventário de efetores | Leslie + Kim + Martin + Michael | Fronteira única verificável quando todos os efeitos cabem nela; ou protocolo recuperável multi-etapas. Ledger e recibos por destino são complementares. Sem prova, bloquear só efeitos dependentes. | Universo, completude, ponto de efeito, commits em voo, restore e prova por destino definidos e revisados. |
| Ações offline, validade, frescor, relógio e janela residual | Usuário + Kim + Leslie; Chet/Donny/Alex por plataforma | Consulta atual a cada ação reduz estado obsoleto e reduz disponibilidade; grant limitado permite somente ações locais autorizadas e mantém exposição residual. | Lista de ações, classe, validade/frescor, relógio e falhas aprovados; sem números presumidos. |
| Fonte de frescor anti-rollback aceita por classe de operação (F1 hardware, F2 âncora de tempo, F3 verificação online) e alta marca por vínculo | Kim + Leslie + Chet/Donny/Alex por plataforma; C06 para autoridade | F1 depende de capacidade real por SO; F2 depende de política de frescor e da própria definição de tempo confiável; F3 elimina o problema offline ao custo de indisponibilidade. Combiná-las amplia cobertura e amplia superfícies de falha. | Capacidade verificada por plataforma, contrato da alta marca e da evidência de contador aprovados por autoridade, e regra de negativa escrita. Sem fonte aprovada, a operação condicional fica negada. |
| Recuperação e provas previamente cadastradas | Usuário + Kim + Bruce | Negar recuperação sem provas válidas; ou adotar processo formal de contestação com autoridade e controles. Via assistida não aprovada. | Classes/quantidade, inscrição, compromisso, notificação, disputa, autoridade, evidência e critérios de negativa aprovados. |
| Retenção de vínculos, provas, auditoria e cópias | Bruce + Lawrence + Kim + Michael | Minimização por finalidade/cópia; prazos e exceções variam por classe/jurisdição. Nenhum prazo escolhido. | Política por jurisdição/classe/cópia, acesso e prova de disposição definidos; revisar legislação escolhida. |
| Tenant seletor, claims, membership e schemas | Kim + Graydon + Fowler | Derivar tenant exclusivamente no servidor; cliente pode selecionar apenas vínculo já autorizado. | Schemas e sessões refletem a mesma autoridade; caso cross-tenant negativo e anti-enumeração revisados. |
| Compatibilidade e armazenamento de chaves | Chet + Donny + Lea + Alex | APIs por plataforma ou abstração comum com capacidades declaradas. Não assumir hardware seguro ou API uniforme. Sem capacidade comprovada, §6.3 nega a operação condicional. | SO/versões mínimas e capabilities por plataforma documentadas por fonte oficial e validadas depois. |
| RF/location/E2EE/ASR | Bruce + Radia + Paul + Daniel + Jouni + Kim | Autorização de identidade é pré-condição, não decisão suficiente de finalidade/consentimento ou gestão de conteúdo. | Cada domínio define scope, consentimento, destino, frescor, retenção e ponto de terminação. |

### 13.1 Alternativas de recuperação

O contrato deve preservar pelo menos duas classes de resultado: migração com emissor atual e recuperação excepcional sem emissor atual. Na migração normal, exigir prova da chave atual, confirmação do novo emissor, authority/epoch/fence e revogação do vínculo anterior. Na recuperação excepcional, ausência de prova previamente cadastrada válida mantém vínculo e direitos atuais sem transferência. Decisão de número/independência das provas, notificação e contestação continua sob decisão do usuário. Nem acesso ao e-mail, nome semelhante, GMC ou novo domínio substitui prova.

### 13.2 Alternativas de revogação offline

- Consulta online em cada operação crítica: menor dependência de cache; indisponibilidade da autoridade nega a operação.
- Grant limitado e assinado: permite somente operação local explicitamente autorizada e dentro de limites aprovados; traz janela residual até revogação conhecida ou expiração.
- Grant amplo/longo: não recomendado e nunca supera negativas `stale/unknown`, mas não existe valor numérico decidido.

A recomendação documental favorece grants limitados somente para ações locais reversíveis aprovadas. Não estender a federação, ACL, localização precisa, migração ou PTT externo. Sem decisão de ação e limite, não declarar prontidão.

## 14. Contribuições e revisões

Esta SPEC consolida a contribuição de Kim (SHA-256 `2fb19617fb3a90735621d9912a950baae3b9ff8d112ab693b737300757741479`) e as fontes independentes disponíveis em 16/11/2026. Bruce Schneier revisou a contribuição de Kim e identificou seis pontos parciais em `contribuicao-bruce-schneier.md` (SHA-256 `8a18dac0c45c4ad365112619399cfdf1f14c4c28dfb2932e98b00ec971923488`), §14 (sessão/revogação, epoch/frescor, transferência/fence, provas, restore e auditoria). Kim respondeu em contribuição §§12–19, mas os critérios permanecem OPEN até integração e decisões. Erich Gamma revisou a conciliação F-1/G-2 em contribuição §17 de Kim e confirmou consistência textual dos recortes, sem conflito confirmado; schemas/sessões e múltiplos tenants permanecem OPEN. Pareceres são de versões/hash específicos, não aprovação desta SPEC canônica.

Bruce Schneier revisou a DRAFT-0.1 (SHA-256 `3876cb5e41b34ee1de731e6d46c2c88e334fc69528333bc02d219d5c3979f8ab`), §21 da contribuição dele, e a DRAFT-0.2 (SHA-256 `ef612bbfcba1d3318bb43e1c06fe1770350a2c7611e16bf995e29dbd7e214b56`), §23. A DRAFT-0.3 esclarece que relatórios de destino discriminam dimensão por cópia e que lookup interno protegido de membership pode ocorrer sem revelar externamente tenant/recurso. Transferência, provas, auditoria e integrações seguem parciais/OPEN; nenhum parâmetro, política, schema ou prova operacional foi fechado.

O veto de C4 à DRAFT-0.4 incidiu sobre ADR-059: o predicado anti-rollback dependia de um contador que pode ser restaurado junto com o snapshot, o que tornava o replay indistinguível de uso legítimo. A DRAFT-0.5 responde com a §6.3 (fonte de frescor fora do controle do cliente, alta marca por vínculo, negativa na ausência de fonte confiável) e com os erros e casos negativos correspondentes em §9 e §12. A correção é textual e não foi revisada; permanece sujeita ao parecer de Bruce e ao confronto com ADR-059 e com as SPECs que a referenciam (SPEC-004, SPEC-014, SPEC-015, SPEC-016, SPEC-019, SPEC-020).

A proposta de transferência de Alan, SHA-256 `fe4d9b8820fcaf724e487f96cbb0f53c7916985862ba6f39999c273d9bfea4e2`, recomenda fronteira única apenas se verificável e protocolo recuperável entre efetores caso contrário. Kim §19 identificou contraexemplos e critérios, sem escolher tecnologia. Integração de Leslie/Martin/Michael/Bruce e crítica recíproca Ada↔Alan continuam pendentes.

## 15. Estado e limitações

- **Especificado nesta versão:** semântica proposta de principal tipado, derivação do tenant, decisão por operação, estados de validação, negativos, distinção entre grant e autorização atual, direção de provas pré-cadastradas, comportamento de partição e dependências. Nesta DRAFT-0.5: o que conta como prova anti-rollback e anti-clone do grant offline (§6.3), a exigência de fonte de frescor fora do controle do cliente restaurado, a negativa na ausência de fonte confiável, a distinção entre revogação desconhecida e ausência de revogação (§5.1, §6.1, §6.3) e o comportamento observável de falha (§9).
- **Aprovado em fonte normativa:** GMI imutável UUIDv7, OIDC `(iss,sub)`, GMC público pseudônimo, `acct:` opt-in, autenticação federada e autorização explícita. Formato final de GMC e detalhamento de políticas seguem pendentes conforme ADRs.
- **OPEN:** parâmetros de validade/frescor/TTL; qual fonte de frescor (F1/F2/F3) é aceita por classe de operação e por plataforma, com que capacidade comprovada; formato, prova e autoridade da alta marca por vínculo; mecanismos/algoritmos; política de provas/contestação; fronteira/fence/inventário; retenção/jurisdição; session lifecycle; schemas/wire; suporte por SO; papéis/auditoria; autorizações RF, localização, E2EE e ASR.
- **Não demonstrado:** implementação, schema de banco, endpoints operacionais, compatibilidade de SDK, suporte de hardware, fencing, integração de provedores, qualidade criptográfica ou conformidade legal.
- **Não executado:** testes de produto, builds, ataques, banco, serviços ou infraestrutura. Casos desta SPEC são planejados e NOT_EXECUTED.
- **Revisão:** versão canônica DRAFT-0.5. Pareceres sobre versões anteriores não aprovam esta versão; novo confronto pelo hash atual permanece necessário. A correção do veto C4 sobre ADR-059 foi feita por Kim Cameron (pod P2) e ainda **não** foi revisada independentemente; nada neste arquivo deve ser lido como aprovação.

Não liberar implementação até o planejamento estar encerrado e todas as SPECs aprovadas, conforme `06-planejamento/pendencias.md` e o protocolo de consolidação da rodada.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC, propostos nesta versão. `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED`; nada foi executado. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN significa esperado de negar ou informar desconhecido, nunca valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S002-056-P1 | ADR-056 | Perfil de credencial publicado declara N e N-1; credencial N-1 com claims exigidas e assinatura válida | Autenticar e autorizar operação | Credencial aceita, scopes atuais aplicados; versão usada registrada na decisão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-056-N1 | ADR-056 | Cliente apresenta credencial em formato fora da janela declarada (N-2) | Chamar operação autenticada | Negado antes de qualquer efeito; nenhum parser tolerante, nenhum downgrade para formato antigo; motivo sanitizado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-056-N2 | ADR-056 | Grant assinado com `versão de política` desconhecida pelo validador | Validar o grant | Negado; nenhuma cláusula ignorada como opcional e nenhum campo desconhecido tratado como equivalente | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-056-B1 | ADR-056 | Credencial com claim opcional desconhecida e claim crítica desconhecida | Validar | Claim opcional ignorada sem alterar semântica crítica; presença de claim crítica desconhecida nega a operação | B | NOT_EXECUTED / NOT_ASSIGNED |
| S002-056-F1 | ADR-056 | Cliente atualizado para versão que só aceita grant em formato novo; existem grants antigos em circulação | Rollback do cliente para versão anterior | Emissão e uso de grant no formato antigo bloqueados; estado de incompatibilidade explícito; nenhuma conversão silenciosa de grant | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-057-P1 | ADR-057 | Grant com `authority_epoch` comparável, no escopo e na autoridade corretos, e fonte de frescor demonstrável | Executar operação condicional | Permitida; epoch registrado na decisão e nenhum outro tipo de epoch usado como prova | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-057-N1 | ADR-057 | Grant com `epoch` de outro escopo/autoridade, ou `dispatch_epoch`/`event_sequence` apresentado como epoch | Validar o grant | `stale_epoch`, negado sem efeito; nenhuma comparação entre tipos distintos | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-057-N2 | ADR-057 | Cursor de sync apresentado como prova de autorização de leitura privada | Autorizar a leitura | Negado; cursor não concede acesso e não substitui grant ou ACL | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-057-F1 | ADR-057 | Transferência de autoridade concluída com novo `authority_epoch`; cliente mantém grant do epoch anterior | Reapresentar o grant | Rejeitado fail-closed em todas as tentativas; revalidação com a autoridade exigida; nenhuma tradução para o epoch novo | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-058-P1 | ADR-058 | Grant emitido e persistido; nenhuma prova de entrega ao dispositivo | Ler a resposta da emissão | `ack_class=persisted_local`; UI mostra "não entregue ao dispositivo" | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-058-N1 | ADR-058 | Resposta com `ack_class=persisted_local` | Fluxo tenta exibir "grant entregue" ou "ativo no dispositivo" | Exibição recusada; estado de entrega permanece desconhecido; nenhuma classe superior inferida | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-058-N2 | ADR-058 | Resposta de revogação chega sem `ack_class` | Cliente processa a confirmação | Resposta tratada como inválida; nenhuma marca de revogação confirmada; registro `unknown` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-058-F1 | ADR-058 | Revogação enviada ao destino; recibo perdido | Nova unidade de efeito coberta pela revogação | Negada até confirmação; resultado `outcome_unknown` com consulta, nunca sucesso por omissão | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-058-B1 | ADR-058 | Incidente SOS com entrega confirmada pelo dispositivo e sem pessoa autenticada | Tentar exibir atendimento | `delivered_to_device` sem `acknowledged_human`; atendimento só aparece com `principal_ref` e auditoria de pessoa autorizada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-P1 | ADR-059 | Grant offline válido para operação enumerada, com F1 verificada ou F3 disponível | Check-in de sessão Guardian existente | Aceito localmente, sem criar sessão nova e sem efeito federado; ACK `persisted_local` | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-N1 | ADR-059 | Grant offline válido | Criar nova sessão Guardian, divulgar localização precisa, alterar ACL, mudar identidade, transmitir RF ou pedir transcrição remota | Negado em todas; nenhum efeito local ou federado e nenhum grant derivado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-N2 | ADR-059 | Revogação conhecida aplicada no destino; grant ainda dentro da validade nominal | Próxima unidade de efeito coberta | Negada; lease não sobrevive à revogação conhecida e não há nova divulgação | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-B1 | ADR-059 | Dispositivo sem qualquer registro local de revogação e sem evidência positiva vigente cobrindo o escopo | Operação condicional | `revocation_state_unknown`, tratado como `unknown` e negado; ausência de registros nunca lida como ausência de revogação | B | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-F1 | ADR-059 | Mesmo `key_id` e material de contador apresentados por dois dispositivos no mesmo escopo | Reconciliação na autoridade | `clone_suspected`, negativa sob aquela chave, bloqueio dos efeitos dependentes do vínculo; outros vínculos do principal preservados | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-059-F2 | ADR-059 | Relógio local ajustado para trás; grant dentro do `not_before`/`not_after` | Operação condicional sem F2 nem F3 | Negada; relógio local não é fonte de frescor e coerência interna do grant não basta | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-060-P1 | ADR-060 | Autorização de identidade válida para o canal; nenhum consentimento de ASR registrado | Pedido de transcrição remota do canal | Negado; identidade não habilita ASR, gravação, localização ou RF; canal permanece com `confidentiality` do perfil | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-060-N1 | ADR-060 | Principal com papel administrativo e grant válidos | Pedido de decifrar conteúdo E2EE ou recuperar conteúdo sem chave | Negado; autorização de identidade não é chave e não produz texto claro | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-060-N2 | ADR-060 | Perna terminada por bridge, ASR ou gravação | API/UI ou contrato declara E2EE para essa perna | Alegação rejeitada; `confidentiality=terminated` exibido; TLS/DTLS/SRTP não sustentam a alegação | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-060-F1 | ADR-060 | Consentimento de ASR por canal revogado durante processamento | Novos segmentos chegam ao worker | Novos segmentos bloqueados; derivados já produzidos permanecem `terminated`; resultado de segmento em voo reportado como desconhecido ou interrompido, nunca concluído por omissão | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-061-P1 | ADR-061 | Banco de domínio disponível para emissão de grant | Emitir grant que também registra evento | Transação única com outbox; commit único; publicação posterior uma vez | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-061-N1 | ADR-061 | Estado de autoridade `stale` ou `unknown` para o escopo | Mutação de ACL, grant ou vínculo | Bloqueada sem commit e sem ACK de sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-061-N2 | ADR-061 | Emissão com Inbox em ledger separado da projeção | Tentar emitir `ack_class=materialized` | Proibido; máximo `persisted_local`, projeção em `pending` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-061-B1 | ADR-061 | Migração de vínculo com fence emitido e commit interrompido | Retry idempotente do mesmo pedido | `authority_transfer_unknown`; mutações de ambos os emissores bloqueadas; nenhuma transferência dupla e nenhum sucesso inferido do timeout | B | NOT_EXECUTED / NOT_ASSIGNED |
| S002-061-F1 | ADR-061 | Restore de snapshot sem revogações e epochs posteriores completos | Tentar servir dado privado ou reabrir mutação | `restore_history_incomplete`; cópia isolada e negativa até reconciliação; replay não reativa credencial revogada nem amplia prazo | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-062-P1 | ADR-062 | Classe de auditoria com política aprovada e `policy_id/version` vigente | Registrar emissão, revogação e decisão administrativa | Registro gravado com referências opacas, escopo, versão de política e motivo sanitizado | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-062-N1 | ADR-062 | Classe de auditoria sem política aprovada | Tentar gravar o registro | Falha fechada com erro explícito; nenhum registro persistido além do mínimo do efeito imediato e degradação declarada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-062-N2 | ADR-062 | Prazo de auditoria não decidido (OPEN) | Consultar retenção da classe | Informa prazo não definido; nenhum valor presumido e nenhuma retenção indefinida apresentada como política | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-062-F1 | ADR-062 | Hold de auditoria com autoridade, escopo e revisão identificados | Consulta comum durante o hold | Descarte suspenso apenas no escopo do hold; consulta comum negada e nenhum grant reativado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-062-F2 | ADR-062 | Revogação aplicada; uma cópia está offline | Coletar estado por destino | Cópia reportada `pending`/`unknown`, nunca `confirmed`; nenhuma revogação declarada global | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-063-P1 | ADR-063 | Cliente com piso de versão assinado; atualização assinada com `generation` superior ao piso | Instalar atualização | Assinatura verificada, `generation` registrada e instalação aceita | P | NOT_EXECUTED / NOT_ASSIGNED |
| S002-063-N1 | ADR-063 | Release anterior que reintroduz falha de anti-rollback do grant, com `generation` inferior ao piso | Tentar instalar esse release | Rejeitado pelo piso; cliente permanece na versão atual e registra o motivo | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-063-N2 | ADR-063 | Dependência nova de validação de token sem versão resolvida, licença, digest ou análise de vulnerabilidade | Aceitar a dependência no build | Entrada bloqueada; build sem artefato distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S002-063-F1 | ADR-063 | Chave de assinatura comprometida; lista de revogação publicada | Validar token ou grant assinado pela chave revogada | `key_unknown`, negado; artefatos da chave reemitida aceitos; revalidação de grants emitidos sob a chave revogada exigida | F | NOT_EXECUTED / NOT_ASSIGNED |
| S002-063-F2 | ADR-063 | Rotação de `kid` publicada sem janela de sobreposição aprovada (OPEN) | Usar credencial assinada pelo `kid` anterior | Negado e informado como janela não aprovada; nenhum período de convivência presumido | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) valores de `max_credential_lifetime`, `max_revocation_staleness` e `max_offline_grant_lifetime` — sem medição, só há negativa; (2) qual fonte de frescor (F1/F2/F3) vale por classe e por plataforma; (3) formato de prova e autoridade da alta marca por vínculo; (4) algoritmo/allowlist e formato JWS/COSE; (5) classes e quantidade de provas de recuperação e autoridade adjudicadora; (6) enum de `ack_status` de check-in Guardian (D3) e interface de `replicated_to_authority` (D8); (7) retenção/jurisdição de vínculos e provas.
