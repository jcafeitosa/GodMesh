# Proposta documental — retenção e revogação de dados

> **RR-001 · proposta técnica e de política, não aprovada para produção.** Complementa `seguranca.md`, `privacidade-crowdfind.md`, `wifi-compartilhamento-privacidade.md`, a política de localização, SOS e o Guardian. Não altera ADRs nem define obrigação legal. A jurisdição é Brasil (primária) e Colômbia (secundária), com prazos por classe definidos na ADR-065; nenhum prazo foi validado juridicamente. Nenhuma jurisdição adicional, prazo, retenção de backup, base legal, oferta ou SLO foi escolhida. Dados sem política efetiva não podem ser tratados como autorizados para persistência e replicação.

## 1. Objetivo e separação de decisões

Para cada classe de dado, determinar (1) finalidade e autorização para coletar/processar, (2) prazo de retenção, (3) cada cópia e operador que a mantém, (4) o efeito de revogar acesso ou consentimento, (5) descarte, auditoria e eventual exceção legal. Permissão ativa não é prazo de retenção; expiração não prova apagamento; revogação para de autorizar acesso futuro, mas precisa de uma decisão distinta para conteúdo histórico.

Este documento propõe um contrato comum e alternativas. Valores numéricos são parâmetros pendentes, nunca implícitos. A autoridade que define identidade/grants e a semântica de eventos duráveis pertencem aos contratos propostos de [identidade e autorização](../07-contratos/identidade-autorizacao.md) e [eventos e sincronização](../07-contratos/eventos-sincronizacao.md), mantidos por Kim Cameron e Martin Kleppmann. Sua integração semântica com este documento, incluindo grant, frescor, revogação, ACK por cópia e replay, permanece a conciliar; a existência dos arquivos não aprova seus termos. Este contrato não replica seus estados, envelopes, cursores ou regras de idempotência. [Autoridade de escrita](../01-arquitetura/autoridade-escrita.md) distingue diretrizes de autoridade e mecanismos propostos para mutações fortes, mas não define retenção nem o tratamento de cada cópia.

## 2. Registro de política

Cada regra efetiva DEVE ser versionada e ligada a um escopo de tenant, finalidade, jurisdição e classe. Registro conceitual (não é SQL nem migração):

| Campo | Tipo | Obrigatório / semântica |
|---|---|---|
| `policy_id` | string opaca | Sim; identifica a política sem embutir dados pessoais. |
| `version` | inteiro positivo | Sim; monotônico dentro de `policy_id`; não se altera conteúdo já efetivo. |
| `data_class` | enum | Sim; uma classe da matriz abaixo. |
| `purpose` | enum/string controlada | Sim; finalidade aprovada, sem curingas. |
| `tenant_scope` | UUID ou escopo global explicitamente autorizado | Sim; nunca inferir compartilhamento cross-tenant. |
| `jurisdictions` | lista de códigos de jurisdição | Sim para política efetiva; `[]`/desconhecido significa bloqueada. |
| `retention_trigger` | enum | Sim: `observed_at`, `received_at`, `session_closed_at`, `grant_revoked_at`, `incident_closed_at` ou `created_at`; escolher o evento que dá início ao prazo. |
| `retention_seconds` | inteiro positivo ou nulo | Sim; `null` significa **não configurado**, não infinito, e impede efetivação. Nenhum número é aprovado por RR-001. |
| `copy_class` | enum | Sim: primária, réplica, fila/outbox, dispositivo offline, cache, backup, exportação ou derivado. Cada cópia requer regra própria. |
| `disposition` | enum | Sim: `delete_content`, `crypto_erase_if_supported`, `retain_minimized_audit`, `aggregate_irreversibly` ou `legal_hold`. Depende de aprovação e suporte documentado. |
| `legal_hold_ref` | referência opaca ou nulo | Obrigatório somente em `legal_hold`; aponta para autorização registrada fora do conteúdo retido. |
| `effective_from` / `effective_until` | timestamps UTC ou nulo | Sim / opcional; validade definida pela autoridade. |
| `approvals` | lista de ator, escopo, horário e referência | Sim; autorizações de produto, privacidade/jurisdição e operador de dados conforme a governança aprovada. |
| `policy_status` | enum | `draft`, `blocked`, `approved`, `effective`, `superseded`; transições administrativas e permissões são definidas no contrato transversal, sem tratar `draft` ou `approved` como automaticamente efetivos. |

Uma classe pode ter prazos diferentes por jurisdição ou finalidade somente em versões distintas, sem escolher uma delas silenciosamente. Alterar o prazo afeta novas retenções a partir de `effective_from`; dados existentes recebem uma política explícita de migração/descarte. `legal_hold` suspende somente as classes e cópias listadas; não suspende revogação de acesso nem autoriza ampliar finalidade.

### Classes mínimas da matriz

| Classe | Exemplos / finalidade limitada | Decisão ainda necessária |
|---|---|---|
| `location.guardian.precise` | Observação precisa durante sessão Guardian aceita | Prazo após observação, conclusão, expiração e revogação; cópias offline. |
| `location.sos.precise` | Posição e qualidade no incidente SOS autorizado | Prazo do incidente, auditoria e eventual obrigação de atendimento/registro. |
| `crowdfind.sighting` | Relato cifrado e metadados mínimos do observador/beacon | TTL máximo, janela de resolução, revogação e política de fila. |
| `wifi.observation.private` | Descoberta local de SSID/BSSID | Retenção no dispositivo e sincronização permitida. |
| `wifi.session.operational` | Grant, sessão, IP/MAC transitório e evidência do controlador | Separar prazo de sessão, identificadores efêmeros e agregados. |
| `wifi.ap.location.private` | Coordenada protegida do AP e evidência de origem | Retenção do cadastro, contestação e efeito de revogar publicação. |
| `wifi.map.projection` | Projeção `exact_opt_in`, `coarse` ou `hidden` | Prazo de cache, réplica, índice de busca e remoção após revogação. |
| `identity.authorization.audit` | Emissão, uso e revogação de credenciais/grants | Prazos mínimos/máximos por jurisdição, conteúdo minimizado e acesso. |
| `security.audit` | Acesso administrativo, exportação, descarte e exceções | Prazo e proteção contra alteração/consulta indevida. |
| `derived.aggregate` | Métricas sem identificadores ou localização reconstruível | Critério demonstrável de irreversibilidade e prazo próprio. |
| `backup` / `export` | Cópias que contenham uma das classes acima | Retenção, acesso, ciclo de restauração, expiração e eliminação segura. |

As classes são propostas; a equipe de dados pode refinar nomes e escopos no contrato próprio. Não consolidar classes de localização precisa, avistamento, identidade de visitante e auditoria num único TTL genérico.

## 3. Aplicação e bloqueio de prontidão

- No início da coleta, associar cada conteúdo persistido à classe, finalidade, escopo, versão da política e instante de origem. Registrar também versão da autorização/consentimento referenciada, sem gravar segredo ou prova reutilizável no log.
- Antes de persistir, replicar, exportar ou publicar, verificar política `effective`, autorização ainda válida para essa finalidade e regra aplicável a cada cópia. Repetir a verificação antes de servir leitura sensível e durante replay/snapshot.
- Se política, jurisdição, finalidade, autoridade ou prazo estiverem ausentes/ambíguos, falhar fechado para **persistência, nova divulgação, replicação e exportação**; não converter `null` em retenção indefinida. Uma função transitória local só pode ser usada se tiver autorização própria, minimização e ciclo em memória documentado.
- A prontidão de uma classe permanece bloqueada até que produto escolha finalidades e países-alvo, política aprove duração e gatilho, operador identifique cópias/restore, e contrato de sincronização exponha confirmação ou estado desconhecido. Não há meta operacional nem resultado de descarte neste documento.
- Uma solicitação de exclusão não apaga o único recibo mínimo necessário para demonstrar que houve revogação/descarte. O recibo não contém payload, coordenadas, SSID/BSSID, credencial, token resolvível ou identificador pessoal além do estritamente aprovado.

## 4. Revogação: significado e autoridade

Revogar autorização impede novas ações cobertas pelo grant assim que a autoridade reconhecida registra a mudança. A autorização exata, estado de grants, assinaturas, epoch/cursor, eventos, idempotência e confirmações são de responsabilidade dos contratos comuns de identidade, autoridade de escrita e eventos/sincronização. Este documento define apenas efeitos sobre os dados e cópias.

O pedido/referência de revogação precisa identificar, por referências opacas: `subject_ref`, `grant_ref` ou `consent_ref`, `data_class`, finalidade, escopo, autoridade emissora, hora/ordem autoritativa, política aplicada e instante de recebimento. Não incluir credenciais, coordenadas, conteúdo do evento ou token secreto. O contrato transversal fornecerá o envelope, autenticidade, cursor/epoch, replay e resultados de entrega; tais campos são requisitos de integração, não um segundo esquema de evento C08.

Efeitos que devem ser observáveis:

1. Recusar novas leituras, publicação, emissão de novos grants, resolução de tokens e processamento que exija o consentimento revogado.
2. Invalidar caches controlados pelo serviço no ponto de decisão de acesso; não servir resultado privado enquanto a atualidade da autorização não puder ser demonstrada.
3. Cancelar mensagens em filas/outboxes não entregues sob aquela autorização; operações já em curso devem revalidar antes de publicar ou entregar a próxima cópia.
4. Emitir a ordem comum de sincronização para réplicas e caches controlados, preservando ordem, idempotência e prova de recebimento conforme contrato de eventos. Réplica offline não pode ser declarada revogada até obter confirmação; deve permanecer indisponível para leitores enquanto status for desconhecido.
5. Após a decisão de retenção aplicável, apagar ou tornar inacessível o conteúdo por cópia. Criptografia ou destruição de chave só conta como apagamento se o mecanismo, cópias de chave e recuperação estiverem especificados e comprovados; não afirmar “crypto erase” por intenção.
6. Preservar somente recibo/auditoria minimizados, com prazo próprio e acesso restrito. Se retenção formalmente obrigatória, isolar o conteúdo sob hold de escopo e prazo determinados, negar usos comuns e expor que o apagamento foi adiado.
7. Comunicar que cópias exportadas/baixadas e conteúdo já lido por destinatário não podem ser remotamente recolhidos. Não registrar `delete_confirmed` para dispositivos externos sem confirmação observável.

### Cópias e estados de resultado

Cada cópia é rastreada por classe, controlador/operador, região, tipo e `policy_id/version`. Os estados de entrega da revogação e cursores são definidos pelo contrato de eventos/sincronização; o retorno para esta política usa a disposição do dado, não uma máquina de eventos concorrente:

| Disposição | Significado documental |
|---|---|
| `pending` | Ordem registrada, sem confirmação desta cópia. Não afirmar revogação global. |
| `access_blocked` | Leitura/uso negado nesta cópia, embora conteúdo ainda exista. |
| `delete_confirmed` | Operador confirmou descarte do conteúdo sob critério auditável. |
| `crypto_inaccessible` | Chave de leitura inutilizada, se cópias/recuperação da chave também forem tratadas; equivale a descarte só após verificação definida. |
| `legal_hold` | Conteúdo isolado por retenção formal; finalidade original permanece bloqueada. |
| `unknown` | Cópia ou controlador offline/desconhecido; manter indisponível e não alegar aplicação. |
| `out_of_control` | Cópia foi exportada/recebida fora do controle remoto; usuário foi informado e não se promete remoção remota. |

O contrato de sincronização deve manter evidência por destino, sem reduzir `pending`/`unknown` a `revoked_applied`. O contrato de autoridade resolve disputas de escrita e fencing; nenhum agente deve inventar aqui uma nova fonte de verdade.

## 5. Regras para dados offline, retries e replay

- Fila local contém somente payload cifrado e referências de política/consentimento com expiração; armazenamento seguro, cópias locais e purga precisam ser especificados por plataforma antes de oferecer retenção offline.
- Revogação conhecida no dispositivo interrompe captura/publicação e elimina filas que a política marque para descarte. Se a plataforma não puder provar o descarte, declarar `unknown`, não sucesso.
- Enquanto offline, não inferir que consentimento ainda está válido além de seu escopo e validade expressa. Para localização precisa e resolução CrowdFind, recomendação é não manter concessões offline de leitura; para observações em fila, permitir apenas uso local já consentido se a política efetiva dispuser expressamente, e não enviar quando a autorização atual não puder ser conferida.
- Ao reconectar, validar autorização na origem e no recebimento, data/frescura do dado, política atual e revogações/tombstones antes de replicar. Evento atrasado, duplicado ou fora de ordem não reativa grant, não renova TTL e não substitui revogação mais recente.
- Falha de rede, emissor, nó de sincronização ou controlador produz estado `unknown`/pendente e bloqueia leitura/divulgação sensível; nenhuma tentativa de best effort amplia acesso.
- Backups são cópias, não exceção implícita. Definir janela de expiração e como uma restauração reaplica as revogações posteriores ao snapshot antes de disponibilizar dados. Se isso não puder ser demonstrado, a restauração não pode servir conteúdo sensível.

## 6. Alternativas de descarte e recomendação

Não há decisão de produto/jurisdição. Alternativas para cada classe/cópia:

| Alternativa | Vantagem | Custo/limite |
|---|---|---|
| Apagar payload quando expira/revoga e reter recibo mínimo | Semântica clara e menor conteúdo residual | Requer prazo e propagação de exclusão definidos; indisponível para cópia fora do controle. |
| Destruir chave por objeto/escopo | Pode tornar cópia cifrada inacessível sem varrer armazenamento | Só efetivo se todas as cópias de chave e restore estiverem cobertas; auditoria precisa provar o vínculo. |
| Tombstone e bloqueio de leitura até expiração física | Permite convergência de réplicas e backup | Conteúdo continua armazenado e sensível; precisa prazo máximo e isolamento forte. |
| Retenção longa/indefinida por conveniência operacional | Recuperação/histórico simples | Não recomendada; amplia exposição e não satisfaz minimização por si só. |

**Recomendação documental:** negar acesso imediatamente após revogação; para dados sensíveis, apagar payload segundo prazo curto que o usuário e a revisão jurídica ainda precisam aprovar, conservar recibo minimizado separado e usar tombstones apenas pelo tempo necessário à convergência. Considerar eliminação criptográfica somente como medida adicional com inventário de chaves/cópias e evidência específica. Não converter “curto” em duração até escolha explícita. Holds legais devem ser individualizados, auditados e nunca suspender bloqueio de acesso.

## 7. Erros e exemplos de contrato

Erros de domínio propostos (não são códigos GMP registrados):

| Código | Condição | Resultado exigido |
|---|---|---|
| `RETENTION_POLICY_MISSING` | Classe sem política efetiva ou prazo nulo | Negar persistência/replicação/exportação; apontar classe, sem dados pessoais. |
| `RETENTION_SCOPE_UNRESOLVED` | Jurisdição, finalidade ou cópia sem regra | Bloquear decisão e readiness correspondente. |
| `AUTHORIZATION_STALE` | Não é possível provar grant/consentimento atual | Negar leitura/processamento sensível; devolver estado desconhecido. |
| `REVOCATION_PENDING` | Há destinos sem confirmação | Expor lista de destinos/estado autorizado ao operador; não afirmar global. |
| `DATA_UNDER_LEGAL_HOLD` | Hold formal abrange a cópia | Bloquear usos comuns; permitir apenas processo nomeado e autorizado. |
| `COPY_OUT_OF_CONTROL` | Cópia exportada/recebida fora do domínio | Informar limite; não afirmar remoção remota. |
| `DELETE_UNCONFIRMED` | Operador não confirmou descarte | Manter estado desconhecido/pendente, reter evidência mínima e escalar ao operador. |

Exemplo positivo ilustrativo, **não prazo aprovado**: política de `crowdfind.sighting` está efetiva para país/tenant declarados, contém gatilho e duração aprovados pelo usuário, enumera serviço primário e réplica autorizada; revogação bloqueia resolução imediatamente, ambas as cópias reportam destino/status via contrato comum, conteúdo é apagado no gatilho aprovado e o recibo não revela coordenadas/token.

Exemplo negativo: `retention_seconds=null`, jurisdição desconhecida, réplica offline ou PSK já distribuída. Não marcar a política como efetiva, não continuar anunciando localização exata/mapa como removida, nem tratar tentativa de rotação como revogação aplicada.

## 8. Conformidade documental futura

Critérios propostos para revisão/validação autorizada, não testes executados:

1. Todas as tabelas, eventos, caches, filas, réplicas, backups, exportações e agregados sensíveis têm classe, finalidade, política efetiva, gatilho, prazo e operador; consulta estática encontra nenhum campo sem política.
2. Política `draft`, prazo nulo, escopo desconhecido, grant expirado/revogado ou emissor sem frescor bloqueia gravação durável, leitura sensível e nova replicação.
3. Uma revogação repetida/reordenada não reativa autorização; prova por destino diferencia confirmado, bloqueado, pendente, hold, desconhecido e cópia externa.
4. Cópia offline/restaurada é isolada até processar revogações e tombstones mais recentes; falha de processamento não expõe payload.
5. Depois do prazo aprovado, payload de cada cópia é removido/inacessível e a verificação documenta resultado; backups seguem janela e validação na restauração.
6. Hold legal exige referência/autoridade/escopo e impede leitura fora do processo autorizado, com prazo de revisão definido pela jurisdição aprovada.
7. Auditoria, métricas, erros, notificações e exportações não incluem coordenadas, SSID/BSSID privado, token resolvível, segredo ou conteúdo descartado.
8. Interface e documentos distinguem autorização revogada, acesso bloqueado, conteúdo apagado, chave inacessível, destino pendente e cópia fora do controle.

## 9. Decisões ainda abertas e integração

Para efetivar a matriz, o usuário deve escolher países/serviços e finalidades; responsável jurídico deve avaliar obrigações por jurisdição; produto aprova prazos e retenções permitidas; operador identifica storage, backup, cache, exportação e destino externo; dados/replicação definem cobertura e evidência. SLO/RPO/RTO, carga, hardware, fornecedores e suporte humano não foram escolhidos e não são inferidos.

Pedidos a Alan para integração: (a) fazer `eventos-sincronizacao.md` definir envelope, ordem/cursor, ACK por cópia, replay e restauração; (b) fazer `identidade-autorizacao.md` definir grant/consent revogável, autoridade e frescor; (c) integrar decisões de partição de `autoridade-escrita.md` sem fazê-lo decidir retenção; (d) contratos Guardian, CrowdFind, SOS, identidade, Wi-Fi e sync referenciar a matriz por `data_class/policy_id` e não duplicar estados. A existência dos contratos propostos não fecha esses critérios: as integrações e aprovações aplicáveis e as decisões externas necessárias permanecem pendentes.

## 10. Limites desta proposta

Especificação documental apenas; nenhuma classe está efetiva por este documento. Não foram consultadas fontes jurídicas externas porque nenhuma jurisdição foi escolhida e não se emite aconselhamento jurídico. Não há valores de retenção aprovados, evidência de mecanismo de descarte, confirmação de destino/operador, resultado de teste, infraestrutura, contrato de atendimento humano ou comprovação de implementação. O bloqueio de prontidão deve permanecer explícito até Alan integrar contratos relacionados e o usuário decidir políticas/jurisdições.

## Política inicial de retenção por classe (ADR-065)

> **Origem:** decisão do CTO em 17/11/2026, a pedido do usuário, para desbloquear a ADR-062. **Não é parecer jurídico.** Cada prazo precisa de confirmação por jurisdição antes de operação comercial. Critério: o mínimo que sustenta a função declarada, com folga para disputa, suporte e auditoria. Cada linha declara as seis dimensões da ADR-062 (finalidade, classe, cópia, jurisdição, prazo, gatilho); a política só é efetiva quando finalidade, cópia e jurisdição estiverem confirmadas por jurisdição — até lá vale o fail-closed. Jurisdição padrão: Brasil (primária) e Colômbia (secundária), conforme ADR-066.

| Classe | Finalidade | Cópia | Jurisdição | Prazo | Gatilho de descarte | Justificativa |
|---|---|---|---|---|---|---|
| Mensagem (conteúdo) | histórico e disputa | primária; secundária só meta | BR/CO (declarada) | 24 meses | `expiry` a partir do último evento do canal | permite histórico útil sem arquivo permanente; E2EE não permite busca |
| Metadado de mensagem | auditoria e suporte | primária; secundária só meta | BR/CO | 24 meses | `expiry` | necessário para suporte e disputa |
| Transcrição ASR | função do canal autorizado | primária | BR/CO | 12 meses | `expiry` | subproduto de maior risco; função cumprida em prazo menor |
| Áudio PTT / gravação | não retido por padrão | — | — | 0 (efêmero); opt-in 30 dias | `opt_in_end` | voz é o dado mais sensível e a função não exige arquivo |
| Posição e trajetória | mapa e suporte | primária | BR/CO | 30 dias | `expiry` | reduz vigilância; suficiente para mapa |
| Sessão Guardian | prestação de contas do acompanhamento | primária | BR/CO | 7 dias após o fim | `session_end + 7d` | vínculo temporário por desenho |
| Incidente SOS | dever de cuidado e disputa | primária; cópia de recuperação autorizada | BR/CO | 12 meses | `incident_closed + 12m` | evidência de acionamento e ACK; posição precisa só com autorização |
| Log de auditoria | detecção e investigação de abuso | primária; exportação autorizada | BR/CO | 12 meses (posição precisa: 24 meses) | `expiry` | rastreabilidade proporcional; par com a classe auditada |
| Telemetria e sinais | operação e capacidade | agregado pode ir à secundária | BR/CO | 30 dias brutos; 13 meses agregados | `rollup_complete` | operação sem identificador |
| Wi-Fi: cadastro de AP | titularidade do AP | primária | BR/CO | enquanto ativo + 90 dias | `ap_inactive + 90d` | dado do administrador, não do visitante |
| Wi-Fi: sessões de acesso | auditoria de acesso | primária | BR/CO | 90 dias; sem navegação | `expiry` | auditoria de acesso, não de comportamento |
| CrowdFind | resolução de avistamento | primária | BR/CO | token 15 min; avistamento 24 h | `token_rotate` / `expiry` | reduz correlação e rastreio |
| Backups | recuperação de desastre | primária + cópia secundária declarada | BR/CO | 35 dias de rotação | `backup_expiry` | recuperação sem arquivo morto; restore testado |
| Conta encerrada | reverter engano | primária + backup | BR/CO | 30 dias de carência | `delete_requested + 30d` | permite desfazer erro sem reter indefinidamente |

Regras de execução: prazo é por classe **e por cópia**; tenant pode reduzir, nunca ampliar sem nova decisão registrada; expiração é verificada e registrada, não presumida; classe sem política falha fechada (ADR-062); conteúdo já entregue a destinatário ou bridge não é recolhível; hold suspende descarte com autoridade, escopo e revisão a cada 90 dias.
