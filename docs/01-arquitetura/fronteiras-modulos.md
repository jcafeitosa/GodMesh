# Fronteiras de módulos e interfaces de integração

> **Estado:** proposta documental C05 para esclarecer responsabilidades e interfaces existentes. Não é uma ADR aprovada, um esquema wire GMP, uma decisão de processo/deploy nem evidência de implementação. A interpretação do Universal Gateway e a semântica de frescor de capacidades aguardam revisão e integração por Alan Turing.

## 1. Pergunta e decisões existentes

Este documento explicita quem responde por descoberta, tradução semântica, autorização e roteamento quando um protocolo/dispositivo externo integra com o GodMesh. O objetivo é reduzir o que cada chamador precisa inferir sobre as interfaces; não introduzir serviços adicionais.

Decisões que permanecem prevalentes:

- ADR-019 mantém monólito modular como forma padrão do domínio; processos dedicados têm justificativa restrita, como mídia ou segurança.
- ADR-022 e ADR-032 mantêm Mumble, FRN e demais integrações externas como bridges/adaptadores opcionais e isolados.
- ADR-037 define a UDNL como camada bidirecional de normalização, identidade escopada, proveniência e projeção com perdas declaradas; áudio PTT permanece separado do log genérico de eventos.
- A arquitetura de referência separa planos e determina que autorização preceda encaminhamento de dados privados ou TX externo.

Os nomes e módulos abaixo são os que já aparecem no glossário, na UDNL, na estratégia tecnológica e no Universal Gateway. A afirmação de que **Universal Gateway é um subsistema/papel composto, não um módulo implantável único**, é uma proposta de interpretação para resolver a ambiguidade documental. Alternativa: tratá-lo como módulo runtime único; isso não combina tão bem com a lista de módulos especializados e os planos separados já documentados. Não se propõe mudar a topologia ou criar uma nova crate/processo.

## 2. Vocabulário e propriedade das responsabilidades

| Nome | Tipo e responsabilidade | O que não possui |
|---|---|---|
| Universal Gateway | Papel/subsistema de integração: coordena os módulos de descoberta/perfil, capacidades, política e roteador para compor uma rota elegível. A localização exata desses módulos dentro do monólito modular ou em processo dedicado continua conforme ADR-019 e requisitos demonstrados. | Não é sinônimo de bridge, UDNL ou de um daemon obrigatório; não decodifica todos os protocolos nem decide identidade/ACL por conta própria. |
| Dispositivo gateway | Instância externa de hardware/software que conecta redes ou sensores. A identidade da instância e as capacidades disponíveis são escopadas ao cadastro e evidências associados. | Não recebe autoridade GodMesh só por ser descoberto ou conectado. |
| Adapter / adaptador | Implementação de borda para um protocolo ou perfil; interpreta entradas/saídas externas e expõe as obrigações versionadas do SDK. Atua como produtor de observações de origem e consumidor de projeções. | Não define os tipos canônicos, não inventa identidade, não concede ACL e não suprime perda de projeção. |
| Bridge | Adapter que oferece uma integração autorizada entre GodMesh e um domínio de protocolo externo. Pode operar em dados, mídia, ou ambos, com superfícies e permissões separadas. | Não é necessária para tráfego GodMesh nativo; conectividade da bridge não equivale a autorização de TX nem a E2EE ponta a ponta. |
| GodMesh Bridge SDK | Interface versionada que descreve os contratos exigidos a implementações de bridge/adaptador, inclusive capacidades, identificação, proveniência, saúde, permissões e relatório de perda, conforme a UDNL. | Não é um processo, plugin arbitrário ou fonte de verdade de decisões de política. A seleção de versão e o wire GMP pertencem aos contratos de protocolo. |
| UDNL | Camada semântica central: valida contexto externo, normaliza identidades/observações/eventos e prepara projeção explícita; usa modelos canônicos e regras de proveniência já definidos em `normalizacao-dados.md`. | Não é o conector, o motor de política ou o serviço que seleciona/transporta uma rota; não transporta quadros de áudio no barramento de eventos. |
| Motor de políticas/identidade | Avalia identidade, principal, finalidade, escopo, permissões e regras externas e devolve decisão de acesso pelo contrato de identidade/autorização aplicável. | Não transforma `supported` ou `operational` em permissão implícita. |
| Roteador de serviços | Compara rota solicitada com capacidades atuais, direção, destino, restrições e decisão de acesso; retorna apenas rota elegível. | Não normaliza payloads, não sobrepõe recusa de política e não autoriza perda de campo obrigatório. |
| GodMesh Media / media-edge | Controla sessões e encaminha áudio no plano de mídia. Uma bridge de voz é uma fronteira separada, potencialmente terminando/transcodificando a mídia. | Não serializa cada frame como evento durável nem infere identidade civil do falante. |

### Fluxo de responsabilidade

```text
Externo → Adapter de origem → UDNL (validar, normalizar, preservar proveniência)
        → política/identidade (decisão explícita) → roteador (rota elegível)
        → UDNL (projeção e relatório de perdas) → Adapter de destino → Externo

Mídia em tempo real → GodMesh Media ↔ fronteira de mídia da bridge autorizada
```

`Universal Gateway` nomeia a coordenação desse fluxo e seus módulos de suporte. É uma convenção arquitetural proposta, não uma ordem de chamadas wire ou desenho de processo. A ordenação semântica de validação, autorização, rota e projeção continua subordinada à UDNL, GMP e política aplicável.

## 3. Contrato documental do adapter

As estruturas abaixo definem metadados mínimos que o limite precisa tornar identificáveis. São um modelo arquitetural para revisão, **não** uma representação JSON/Protobuf, ABI Rust ou API pública. As estruturas canônicas de eventos, identidade, proveniência e projeção continuam pertencendo à UDNL; mensagens e erro de transporte/wire GMP continuam ao documento de GMP/API de Graydon.

### 3.1 Descritor de adapter

| Campo | Tipo lógico | Presença | Autoridade e invariantes |
|---|---|---|---|
| `adapter_id` | `ScopedId` opaco | obrigatório | Identifica a instância do connector dentro do escopo GodMesh; não é credencial nem identificador global do operador externo. |
| `network_kind` | `Namespace` | obrigatório | Declara o protocolo/domínio, por exemplo `mumble` ou `frn`; não prova conexão nem identidade. |
| `network_instance_id` | `ScopedId` opaco | obrigatório | Delimita a instância externa (servidor/rede/gateway) usada para escopar referências; origem/adaptador informa o valor e o serviço retém proveniência. |
| `contract_version` | `ContractVersion` | obrigatório | Versão do contrato SDK efetivamente atendida; negociação, formato wire e política de compatibilidade são definidos nos documentos de protocolo, não aqui. |
| `directions` | `Set<Direction>` (`inbound`, `outbound`) | obrigatório | Só anuncia direções implementadas para este adapter; descoberta não habilita direção de transmissão. |
| `capability_refs` | `List<CapabilityAssessmentRef>` | obrigatório; pode ser vazia | Referências para avaliações contextualizadas; ausência não equivale a suporte universal. O motor de capacidades mantém estado/evidência, não o descriptor. |
| `required_permission_refs` | `Set<PermissionRef>` | obrigatório; pode ser vazia | Enumera permissões que uma operação pode exigir; listar uma permissão não significa concedê-la. O motor de política é autoridade de `AccessDecision`. |

O descritor não contém senha, token de autenticação, chave privada, coordenada pessoal, PII de visitante, payloads ou configuração secreta. Credenciais ficam no cofre/escopo apropriado, nunca em descriptor, proveniência ou erro.

### 3.2 Avaliação contextualizada de capacidade

Para impedir que os estados existentes sejam confundidos, este documento propõe interpretá-los como **níveis de evidência por capability e contexto**, não como autorização nem como saúde geral do servidor.

| Campo | Tipo lógico | Presença | Uso |
|---|---|---|---|
| `subject_ref` | `ScopedId` | obrigatório | Instância precisa de dispositivo, serviço ou adapter avaliada; avaliação de um subject não se transfere a outro. |
| `capability_id` | `CapabilityId` | obrigatório | Função e direção que estão sendo avaliadas, como `voice.tx` ou `message.rx`; a semântica do catálogo é compartilhada com o motor de capacidades. |
| `context_ref` | `ContextRef` | obrigatório | Perfil/versão/configuração e rota ou destino pertinentes; mudanças de contexto invalidam a aplicação automática de evidência anterior. |
| `state` | enum de avaliação (`supported`, `verified`, `operational`) | obrigatório na avaliação interna proposta | `supported`: declarada pelo perfil/adapter; `verified`: evidência atribuível confirma capability e contexto; `operational`: a capability e dependências estão disponíveis naquele contexto observado. São níveis de evidência propostos para uma avaliação, distintos de autorização e de readiness do serviço. |
| `evidence_ref` | `EvidenceRef` (nullable) | condicional | `supported` pode referir apenas a declaração que o afirma; `verified` e `operational` exigem referência a evidência observável. A alegação externa nunca vira prova de identidade GodMesh sem vínculo/autorização próprios. |
| `observed_at` | `Timestamp` (nullable) | condicional | Nulo para declaração sem observação operacional; obrigatório para `verified` e `operational`. Relógio/origem e qualidade devem vir na evidência referenciada. |

Para a representação de API, esta proposta mapeia as avaliações cumulativamente: todo item em `operational[]` também aparece em `verified[]` e `supported[]`; todo item em `verified[]` também aparece em `supported[]`. Assim, `supported[]` contém declarações conhecidas, `verified[]` contém as declarações com evidência atribuível e `operational[]` contém as verificadas cuja disponibilidade foi observada no contexto. Isso corresponde ao contrato GMP v1, seção 7.3, que declara explicitamente as três coleções cumulativas e define seu mapeamento. O contrato marca a semântica como proposta documental; esta fronteira a espelha como proposta técnica, sem declarar aprovação normativa ou validação de implementação. Uma avaliação singular `state` representa o nível máximo atualmente sustentado, não uma autorização.

Uma mudança material de sujeito, perfil, contexto, dependência ou evidência reduz o nível ou torna a avaliação inaplicável. Sem política de frescor configurada para capability/origem/contexto, a observação operacional não habilita nova rota; **nenhum prazo/TTL foi escolhido**. `AccessDecision` continua condição separada, e `HealthReady`/readiness descreve se serviço pode atender sua função operacional, não a permissão ou capacidade de um recurso particular. Logo, `operational` não implica `READY`, e `READY` não concede autorização.

### 3.3 Saúde do adapter

O estado de saúde é distinto do estado de capability. Como proposta de leitura do método `health` já listado na UDNL:

| Estado | Significado documental | Consequência de roteamento |
|---|---|---|
| `ready` | Observação recente indica que o adapter e as dependências que ele reporta respondem. | Não prova nenhuma capability nem concede autorização; cada capability precisa de sua própria evidência/contexto. |
| `degraded` | Adapter responde, mas relata falha limitada por operação/dependência. | Só capacidades sem dependência afetada podem ser avaliadas; não usar um health global para promover uma rota. |
| `not_ready` | Adapter não pode executar a operação relevante ou não há observação atual confiável. | Nenhuma rota que dependa dele é elegível enquanto a disponibilidade não for estabelecida. |

O relatório lógico contém `state`, `scope_ref`, `observed_at` e `reason_code` não sensível. Os rótulos reutilizam a forma de prontidão já proposta para `HealthReady` em `gmp-api-v1.md`; o dono da API deve confirmar se esse perfil também se aplica ao health local do adapter. A atualidade mínima, cadência, timeout, limite de taxa e SLO não estão escolhidos; não atribuir valores numéricos até validação por operação/hardware.

## 4. Obrigações da interface e autoridade

O Bridge SDK e a UDNL já enumeram as operações `discover_capabilities`, `parse_inbound`, `normalize`, `project_outbound`, `validate_identity_evidence`, `health`, `permissions` e `report_projection_loss`. Este documento fixa as obrigações entre módulos, não cria assinaturas concorrentes:

1. **Descoberta/perfil** pode enumerar um recurso e suas afirmações declaradas. Não trata descoberta como posse, autenticação, capability verificada ou autorização.
2. **Adapter de origem** interpreta o formato específico e associa a observação ao `network_instance_id`, preservando o identificador nativo e a proveniência disponíveis. Payload e rótulos externos são não confiáveis até validação.
3. **UDNL** valida escopo/forma, mantém identidade externa distinta da identidade GodMesh, normaliza com proveniência, e devolve erro explícito para representação inválida ou semântica não suportada. Não inventa campos ausentes.
4. **Política/identidade** retorna decisão explícita para principal, finalidade, destino/direção e operação. Ausência/indeterminação de decisão não é `allow`.
5. **Motor de capacidades** conserva a avaliação por sujeito/capability/contexto/evidência. O router não promove `supported` a `verified` ou `operational`.
6. **Roteador** seleciona só uma rota cuja direção e contexto coincidam, todas as capabilities obrigatórias estejam operacionais e frescas, e haja decisão de acesso explícita. Uma capability operacional sozinha não habilita TX.
7. **UDNL de saída** produz `ProjectionReport`; a interface já exige `delivered_fields`, `dropped_fields`, `identity_display_mode`, `upstream_permission` e `loss_reason`. Se a política exigir atributo que o destino não suporta, a rota é recusada; não se descarta silenciosamente.
8. **Adapter de destino** envia apenas a projeção e operação autorizadas e relata o resultado. `enqueued`, `accepted`, `delivered` e ACK humano mantêm seus significados por etapa; não declarar sucesso além da evidência observada.
9. **GodMesh Media** usa fluxo de mídia separado. Uma bridge de voz deve reportar sua própria capacidade/identidade de speaker e limites de confiança; falha do Adapter de dados não deve ser traduzida em áudio, nem vice-versa.

### Elegibilidade de rota

Uma rota documentalmente elegível exige, cumulativamente:

`direção compatível` + `contexto exato` + `capability obrigatória operacional e fresca` + `AccessDecision explícita permitindo a finalidade/destino` + `permissões externas necessárias comprovadas` + `nenhuma perda proibida` + `dependências da rota disponíveis`.

Se qualquer condição estiver ausente, negada, indeterminada ou desatualizada, a resposta é “sem rota elegível” com causa identificável; nenhum fallback amplia escopo ou autorização. Esta regra não cria licença RF, jurisdição, política de consentimento, SLA, atendimento ou condição de implantação; esses valores seguem pendentes nos documentos donos.

## 5. Erros, falhas e isolamento

As fronteiras precisam devolver a falha do contrato que possui a operação, preservando referência/correlação e uma causa sanitizada. Este documento **não cria um catálogo nem envelope concorrente de erros**. Erros de identidade/projeção/cadeia de bridge usam os perfis e códigos propostos pela UDNL; versões, erros HTTP/GMP e compatibilidade pertencem ao contrato de Graydon; IDs/estados/idempotência pertencem ao contrato de eventos/sync; credenciais/decisões pertencem ao contrato de identidade/autorização.

| Condição de falha | Comportamento requerido | Dono do erro/estado canônico |
|---|---|---|
| Entrada externa inválida ou sem proveniência/evidência suficiente | Rejeitar antes de normalizar/rotear; preservar a referência externa quando permitido; não inferir identidade/campos ausentes. | UDNL (`normalizacao-dados.md`), identidade/autorização para vínculo. |
| Capability não verificada, não operacional ou sem observação fresca | Não anunciar/selecionar a rota; não promover estado por health global ou declaração. | Motor de capacidades / C15; contrato GMP para erro exposto ao cliente. |
| Decisão de autorização negada, ausente ou indeterminada | Fail closed; não encaminhar conteúdo privado nem executar TX. | Contrato de identidade/autorização (Kim) e API/GMP (Graydon). |
| Versão/feature sem compatibilidade | Recusar antes de aceitar operação mutável; manter semântica nativa. | GMP/API e negociação de protocolo (Graydon). |
| Projeção incapaz de manter atributo requerido | Recusar envio antes do efeito, registrar `ProjectionReport` e perda por campo segundo UDNL. | UDNL; códigos de projeção nela propostos ainda precisam integração GMP. |
| Timeout ou falha do adapter/dependência | Isolar a fronteira, reportar health `not_ready` quando não houver observação confiável; não afirmar resultado remoto sem recibo. | Bridge SDK/adapter (Gregor); resultado de comando/evento segundo Martin. |
| Retry de efeito externo ou resultado ambíguo | Repetir apenas segundo ID/idempotência/estado canônicos; sem confirmação, resultado permanece desconhecido. | Eventos/sync (Martin) e API/GMP (Graydon). |
| Limite de taxa/quota atingido | Recusar ou adiar conforme perfil configurado; nenhum caminho alternativo contorna o limite. | Perfil/contrato proprietário do adapter/API; valores não escolhidos. |

O retry é responsabilidade do chamador e contrato proprietário. Falha transitória não autoriza repetição ilimitada; repetição de efeito não idempotente sem estado conclusivo fica bloqueada. Nenhum diagnóstico contém segredo, payload privado, localização precisa ou identidade civil. As categorias e códigos específicos apresentados pela UDNL/API não estão homologados como wire aqui; C15 e C07 devem conciliá-los em suas fontes.

## 6. Invariantes

1. Sistemas externos permanecem opcionais; falha ou desativação de bridge não interrompe comunicação nativa GodMesh↔GodMesh.
2. `network_kind + network_instance_id + native_id/session` mantém identidades externas escopadas; apelido/callsign aparente não funde sujeitos.
3. Evidência de autenticação upstream não é prova de titularidade GMI/GMC. Vínculo GodMesh exige prova e autorização segundo a fonte de identidade.
4. Capability e saúde são evidência de suporte/disponibilidade, nunca substitutos de `AccessDecision`, licença ou consentimento.
5. Dados privados só atravessam a fronteira depois da política de autorização; TX externo exige autorização específica de operação e destino.
6. Se a política exigir atributo indisponível na projeção, recusar em vez de eliminar o atributo sem sinalização.
7. Áudio usa plano de mídia, buffers e isolamento próprios; não inserir cada frame no log genérico de eventos.
8. Credenciais não entram em `AdapterDescriptor`, proveniência, `ProjectionReport`, erros ou logs.
9. Repetição após timeout só é segura quando idempotência/recibo está definido pelo contrato dono; sem isso, resultado pode ser desconhecido e não se repete automaticamente.
10. Estados/decisões são relativos ao sujeito, capability, contexto, evidência e instante; não propagar validade entre tenant, instância, perfil, direção ou destino.

## 7. Exemplos de conformidade documental

Estes exemplos demonstram a aplicação das regras e não representam teste executado, implementação ou dados reais.

### Caso positivo — entrada externa com identidade limitada e projeção permitida

- O adapter reporta `network_kind=mumble`, uma instância de servidor escopada, sua própria identidade de adapter e uma sessão externa; a evidência indica apenas autenticação da sessão naquele servidor.
- A UDNL conserva identificador/proveniência externa e normaliza evento sem atribuir GMI/GMC.
- O motor de política avalia destino, finalidade e identidade; decisão explícita permite uma rota de dados compatível.
- A rota usa somente capabilities avaliadas `operational` e ainda frescas no perfil/instância aplicável.
- A projeção registra campos enviados/omitidos e modo de identidade; o destino recebe o nome/ID externo com a limitação declarada.
- Resultado: integração permitida no escopo aprovado, sem converter sessão externa em identidade civil/GodMesh.

### Caso negativo — mesmo apelido em Mumble e FRN

- Dois adapters reportam o mesmo display label, cada um sob `network_kind` e `network_instance_id` diferentes.
- O Universal Gateway não os colapsa por semelhança de nome e a UDNL preserva ambas as referências externas.
- Resultado exigido: sujeitos separados; sem prova de vínculo, nenhum recebe GMC/GMI.

### Caso negativo — `supported` usado como autorização

- Perfil anuncia `voice.tx` como `supported`, mas não há evidência de operação atual ou `AccessDecision` para o destino.
- O router não promove o estado e não seleciona a rota.
- Resultado exigido: sem rota elegível; não se faz TX e não se altera licença, preferência ou consentimento.

### Caso negativo — perda de atributo requerido

- Destino externo não transporta identidade individual exigida pela política da operação.
- UDNL registra os campos representáveis e ausentes; router recusa quando a perda é proibida.
- Resultado exigido: sem envio parcial silencioso; eventual rota alternativa precisa ser reavaliada com autorização e projeção próprias.

### Caso negativo — timeout de bridge

- A conexão de bridge vence prazo sem recibo conclusivo.
- O adapter é isolado, a health/capability operacional corrente é invalidada para novas rotas e o resultado remoto é reportado como desconhecido, não entregue.
- Resultado exigido: serviços nativos continuam; não repetir automaticamente sem idempotência ou recibo definidos.

## 8. Checklist de conformidade documental (não executado)

1. Cada termo distingue papel/subsistema, módulo, adapter concreto e bridge opcional.
2. Cada capability/health report identifica sujeito, escopo/contexto e observação/evidência conforme seu estado.
3. Nenhum caminho transforma descoberta, `supported` ou `operational` em permissão.
4. Nenhum fluxo de dados privados ou TX ignora decisão explícita.
5. Toda projeção declara perdas e a rota falha quando uma perda requerida é proibida.
6. Falha de adapter preserva a separação entre serviço externo e caminho nativo.
7. Mídia e eventos não compartilham um contrato de frame persistente.
8. Erros não contêm credenciais nem afirmam entrega sem evidência.

Nenhum item foi validado contra implementação. Validação futura requer fonte/versão de adapter, decisão de frescor, mapeamento de erros GMP, autorização externa/país quando aplicável, ambiente e critérios de operação ainda não escolhidos.

## 9. Alternativas, compatibilidade e decisões para integração

| Questão | Proposta documental | Alternativa | Efeito/limite |
|---|---|---|---|
| Universal Gateway | Papel/subsistema que coordena módulos especializados; não um processo ou crate único. | Tratar como um módulo/serviço que contém todas as funções. | Mantém coerência com lista de módulos atual e ADR-019; muda apenas vocabulário neste lote, sem decidir deploy ou criar crate. Requer integração de Alan na arquitetura comum. |
| Estados e projeção de capacidade | Proposta de níveis de evidência por avaliação e listas cumulativas: `operational[] ⊆ verified[] ⊆ supported[]`. `operational` expressa disponibilidade contextual, não autorização nem readiness global. | Tratar listas como disjuntas, como eixos independentes, ou usar outra regra de projeção. | Faz os nomes da API corresponderem a uma avaliação de nível máximo e mantém a elegibilidade distinta de AccessDecision/HealthReady. GMP v1 §7.3 declara explicitamente a cumulatividade como proposta documental; C26 já conciliou `capacidades.md`, separando evidência de capacidade, autorização, elegibilidade de rota e readiness. A decisão normativa e a política de frescor permanecem abertas. |
| Frescor operacional | Exigir política por capability/origem/contexto; estado sem validade corrente não roteia. | Janela global única, ou nenhuma expiração. | Não foi escolhida janela nem há medições; qualquer rota dependente permanece não pronta até que a política seja definida e validada. |
| Erros internos e wire | Não criar vocabulário paralelo; usar o contrato proprietário da UDNL, identidade, eventos/sync ou API/GMP e pedir conciliação de nomes por C07/C15. | Consolidar um catálogo comum posteriormente em GMP. | Nenhum código ou envelope de erro é definido por C05; não há mudança wire. C07/C15 devem mapear os nomes propostos em suas fontes. |

### Pedidos de integração a Alan Turing

- Incluir `01-arquitetura/fronteiras-modulos.md` no índice do README e linkar a definição de fronteiras no ponto canônico da arquitetura, sem editar esses arquivos neste lote de um escritor por arquivo.
- Obter revisão de Martin Fowler para a interpretação de “Universal Gateway” como subsistema composto e para o lugar correto da fronteira entre módulos internos e processos dedicados.
- Alinhar com Gregor Hohpe/C15 os nomes/campos autoritativos do motor de capabilities e o relatório de perda da UDNL; preservar a UDNL como fonte dos tipos canônicos.
- Registrar que Graydon/C07 documentou em GMP v1 §7.3 a cumulatividade e o mapeamento como proposta; registrar a conciliação já realizada por Jack/C26 em `02-dominio/capacidades.md`, que separa disponibilidade contextual, autorização, elegibilidade de rota e readiness; manter pendentes a política de frescor e a decisão normativa.
- Pedir a Graydon Hoare/C07 e Gregor Hohpe/C15 a conciliação dos códigos já propostos em suas fontes com as categorias de erro API/GMP; este documento não cria catálogo local nem declara wire compatível.
- Manter aberta a escolha de freshness window, thresholds de saúde, quotas, timeout/limite de taxa e política por país/hardware até existir fonte operacional/homologação. Não preencher por estimativa.

## Referências internas

- [Glossário](../00-produto/glossario.md) — relação proposta entre Universal Gateway, UDNL, adapter, bridge e capability.
- [Arquitetura de referência](arquitetura-referencia.md), em especial “Cinco planos” e “Normalização universal de dados e identidade”.
- [Estratégia tecnológica](estrategia-tecnologica.md), em especial “Interface de extensibilidade”.
- [UDNL](normalizacao-dados.md), em especial “Contratos canônicos”, “Compatibilidade e lossiness”, “Segurança e operação” e “Organização Rust”. Fonte dos tipos de normalização/projeção.
- [Universal Gateway](../03-integracoes/gateway-universal.md), em especial “Serviços e contratos”. Fonte atual dos nomes de módulo e estados de capacidade.
- `06-planejamento/decisoes.md`: ADR-019, 021, 022, 029, 031, 032 e 037. Estas decisões não foram alteradas.
