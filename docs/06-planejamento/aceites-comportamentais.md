# Plano de aceites comportamentais — GodMesh integral

Propostas documentais revisadas, não SPEC nem implementação/validação. Integra 60 requisitos existentes; origem e contratos não alterados. Futura SPEC e caso: NOT_ASSIGNED; observado: NOT_EXECUTED. Campos autorais suplementares permanecem documentais. Normalização de negativo/negative e observado/observed/observed_execution não muda semântica; bloco Fowler negativo/degradação/recuperação permanece combinado. Nenhuma atribuição documental designa plantão ou aprova política.

RNF-001 continua ambíguo: alcance de rede versus interface. RNF-013 preserva “provação” na origem. RNF-014 exige crosswalk rótulo/evento/estado sem criar enum e internet_verified permanece sem método/fonte/frescor definidos.

O resultado final de CONCILIACAO-RESPONSABILIDADES.md prevalece sobre declarações históricas de conflito abaixo. Os seis adendos integrais ao fim deste plano integram os aceites referenciados, inclusive subescopos e deltas comportamentais. Os 21 pares originais ficam inalterados. Fonte → requisito → esperado não é prova de execução; gate integral continua aberto.

## RF-001

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-kim.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Existem principals tipados e recursos/tenants conhecidos; issuer/audience e credenciais de cenário estão configurados; as regras de vínculo e autorização são aplicáveis, ainda que schema/constraints finais estejam pendentes.

### Estímulo

Executar autenticação OIDC de issuer permitido; repetir com subject igual em issuer distinto, atributo semelhante sem vínculo, device/node/gateway sem proof-of-possession, membership cross-tenant e GMC como texto em vez de prova.

### Esperado

Somente (iss,sub) validado mapeia ao GMI vinculado; principals não humanos usam suas próprias identidades/credenciais; o recurso deriva tenant no servidor e permite apenas a ação coberta por vínculo, scope, ACL e finalidade. Identificadores públicos não revelam nem autenticam o GMI.

### Invariante

ID/alias/IP isolado não é segredo nem prova; tipos de sujeito não se confundem; nenhuma operação cross-tenant é concedida por valor fornecido pelo chamador.

### Negativo

Issuer/audience, assinatura, vínculo, prova, scope, membership ou consentimento inválidos resultam em deny/indeterminate, sem mutação nem dados privados; e-mail/GMC/IP copiados não vinculam usuário.

### Degradação e recuperação

Autoridade de identidade ou trust indisponível/stale não inventa vínculo nem marca conta verificada; recuperação/migração disputada segue procedimento aprovado, com auditoria, e permanece bloqueada enquanto política de prova/contestação não for definida.

### principal documental

**nome**: Kim Cameron

**status**: proposto_na_matriz_confirmado_como_adequado

**justificativa**: Especialidade cobre semântica GMI/GMC e credenciais tipadas. Colaboração necessária com modelo de dados, API/GMP, gateway e autoridade federada sem transferir o principal.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos integrais; **trecho**: Identidade para usuários, tenants, nós, dispositivos e gateways; **linhas**: 7

- **arquivo**: docs/04-seguranca/seguranca.md; **secao**: Identidades e autorização; **trecho**: Usuários são identificados pelo GMI (UUIDv7 / URN), não por e-mail, número de telefone, IP ou indicativo.

- **arquivo**: docs/02-dominio/identidade-global.md; **secao**: Identidades distintas por domínio; **trecho**: Pessoa/usuário | `gmi_id` UUIDv7 | GMC público; `acct:` opcional

- **arquivo**: docs/07-contratos/identidade-autorizacao.md; **secao**: Conta humana e OIDC; **trecho**: O serviço GodMesh mapeia o par sensível a maiúsculas `(iss, sub)` a exatamente um `gmi_id` sob autoridade local.

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux Rust

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

- web no escopo integral

**nota**: Todos consumem a semântica de identidade; UI, armazenamento seguro e detalhes por plataforma são dependentes de arquitetura/SPEC futura. Sem exclusão implícita.

### decisoes pendentes

- **tema**: Tipos e autoridades de emissão; **alternativas**: - uma autoridade por classe; - hierarquia/federação por escopo; **recomendacao**: Mapear emissor, sujeito, audience, finalidade e autoridade por classe, sem pressupor emissor global único.; **consequencia**: Necessário para vínculo e aceitação federada; altera confiança, complexidade e disponibilidade.

- **tema**: Relação pessoa/tenant/device/node/gateway; **alternativas**: - modelo de registros com relações tipadas; - principals unificados por abstração; **recomendacao**: Manter tipos e credenciais distintos conforme identidade-autorizacao.md.; **consequencia**: Evita misturar conta humana com ativo/serviço; constraints e delegações exigem reconciliação do modelo de dados.

### dependencias

- Graydon Hoare: contrato GMP/audience/erros

- Michael Stonebraker: modelo e relações tipadas

- Erich Gamma: gateway/principals de integração

- Leslie Lamport: autoridade federada

- Bruce Schneier: revisão de política/ameaça

- Alan Turing: consolidação

### criterio fechamento documental

Matriz de principals, emissores, vínculos, proof, tenant, ACL e decisão por operação reconciliada com contratos; casos positivos/negativos e recovery definidos; revisão independente; sem converter aprovação documental em validação.

### ids futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite_documental_proposto_pendente_revisao_e_parametros

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-002

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-michael.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Principal autenticado e autorizado no tenant T; ativo A e dispositivos D1/D2 ainda não existem; contrato aprovado define IDs estáveis, cardinalidade, autorização e operações de endpoint. Limite numérico de ativos/endpoints não definido.

### Estímulo

Criar A em T; vincular dois devices distintos D1 e D2 ao mesmo A; registrar em cada um ao menos um endpoint de protocolo distinto (exemplos BLE e GPS/LTE são apenas ilustrativos); consultar A e suas relações.

### Esperado

Consulta representa A uma vez, lista D1 e D2 como identidades distintas associadas a A e retorna cada endpoint sob o device correto com protocol e external_id. Relações persistem sob os mesmos IDs e tenant autorizados. Registro de endpoint não afirma conectividade ou disponibilidade sem evidência independente.

### Invariante

Cada device vinculado referencia ativo e tenant coerentes conforme a cardinalidade/política aprovada; cada endpoint pertence ao device correto; autorização atual controla leitura e mutação; múltiplos devices/endpoints não se colapsam nem geram relações cross-tenant.

### Negativo

Tentativa de vincular device de T2 a ativo de T1 ou endpoint de outro tenant/device é recusada, não cria vínculo e não revela dados do tenant alheio. O contrato cita 403 not_owner para POST device; resposta de endpoint e política anti-enumeração ainda requerem decisão. Semântica de duplicatas segue contrato comum e não é definida por RF-002 sozinho.

### Degradação e recuperação

Falha de um transporte não apaga nem reatribui device/endpoint registrado e não declara endpoint saudável. Após recuperação/restore, IDs e vínculos permanecem íntegros; filho órfão ou sem escopo coerente é isolado/reconciliado, nunca associado por inferência. Health/retry/dedupe dependem de contratos ainda a definir.

### principal documental

**name**: Michael Stonebraker

**status**: proposed_adequate_for_data_model

**basis**: A origem remete ao modelo-dados.md; o escopo é relações assets/devices/endpoints e integridade. Contratos de API de devices/endpoints dependem de Graydon, sem transferir a responsabilidade documental do modelo.

**conflict**: None

### evidencias

- **file**: docs/00-produto/requisitos.md; **lines**: 8; **section**: Requisitos integrais — rastreabilidade; **quote**: | RF-002 | Ativos com múltiplos dispositivos e endpoints por transporte | APROVADO | `02-dominio/modelo-dados.md` |

- **file**: docs/02-dominio/modelo-dados.md; **lines**: 6; **section**: Conceitos; **quote**: **Tenant** (pessoal/empresa) possui **ativos**; ativo possui **dispositivos**; dispositivo apresenta **endpoints** de rede; endpoints geram **observações**, comandos e eventos. O mesmo ativo pode ter BLE + GPS/LTE. Localização sempre contém origem e instante da observação.

- **file**: docs/02-dominio/modelo-dados.md; **lines**: 36-38; **section**: Tabelas centrais; **quote**: | `assets` | id, tenant_id, name, category | Ativo lógico |
| `devices` | id, tenant_id, asset_id, manufacturer, model, status | `id` interno imutável |
| `device_endpoints` | id, device_id, protocol, external_id, gateway_id | Suporta múltiplos transportes |

- **file**: docs/02-dominio/modelo-dados.md; **lines**: 63-75; **section**: SQL inicial (ilustrativo, incompleto); **quote**: CREATE TABLE assets (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id),
  name TEXT NOT NULL,
  category TEXT NOT NULL
);
CREATE TABLE devices (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id),
  asset_id UUID REFERENCES assets(id),
  model_id TEXT,
  status TEXT NOT NULL DEFAULT 'registered'
);

- **file**: docs/07-contratos/gmp-api-v1.md; **lines**: 433-436; **section**: Operações asset/device; **quote**: | `GET /v1/assets` | `cursor?`, `limit?`, `filter?` allow-listed | `200 Page<Asset>`: `asset_id`, `kind`, `display_label?`, `version`, `updated_at` | `asset.read`; filtro não amplia tenant; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`. |
| `POST /v1/assets` | `Idempotency-Key`; body `kind:string`, `display_label?`, `attributes:object` allow-listed | `201 Asset` com `asset_id`, version e timestamps | `asset.write`; validar tipos/campos permitidos; `400 invalid_request`, `401 invalid_token`, `403 insufficient_scope/not_owner`, `409 IDEMPOTENCY_CONFLICT`, `413 MESSAGE_TOO_LARGE`, `422`. |
| `GET /v1/devices` | `cursor?`, `limit?`, `asset_id?` | `200 Page<Device>`: `device_id`, `asset_id?`, `state`, `capability_summary[]`, version | `device.read`; owner/ACL; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`. |
| `POST /v1/devices` | `Idempotency-Key`; body `device_kind`, `asset_id?`, `claimed_capabilities[]`, `proof_ref?` | `201 Device` com `device_id`, `binding_state`, version; declarado ≠ verificado/operacional | `device.enroll`; prova de posse e delegação conforme C09; `400 invalid_request`, `401 invalid_proof`, `403 not_owner`, `409 IDEMPOTENCY_CONFLICT`, `422`. |

### aplicabilidade

**produtos**: **Personal**: Aplicável a ativos próprios e seus vários dispositivos; transporte específico depende de endpoint suportado e autorizado.

**Teams**: Aplicável diretamente a ativos/veículos/equipamentos e vários dispositivos; ACL/owner organizacional precisa estar definido.

**Node**: Aplicável quando armazena/replica registros autorizados; node não se torna dono do ativo por hospedar dados.

**Platform SDK**: Aplicável às operações de domínio que exponham ativos/devices; compartilhar o contrato, sem assumir API de endpoint inexistente.

**Web**: Permanece no escopo integral da matriz; UI de gestão depende de decisão/projeto, sem exclusão presumida.

**plataformas**: **Linux server Rust**: Aplicável à persistência e integridade do serviço.

**macOS universal SwiftUI**: Semântica aplicável ao gerenciar ativos/devices; interface não altera relações lógicas.

**iOS SwiftUI**: Semântica aplicável ao gerenciar ativos/devices; políticas locais e background fora deste requisito.

**Windows desktop**: Preservado no escopo futuro; não há justificativa documental para excluir comportamento de domínio compartilhado.

**Linux desktop**: Preservado no escopo futuro; não há justificativa documental para excluir comportamento de domínio compartilhado.

**Android**: Preservado no escopo futuro; não há justificativa documental para excluir comportamento de domínio compartilhado.

**N A**: Não proponho N/A para nenhum produto/plataforma; UI de gestão pode ser não aplicável somente após decisão de produto, não o contrato compartilhado.

### decisoes pendentes

- **decision**: Cardinalidade de device→asset e obrigatoriedade de asset_id.; **alternatives**: - asset_id obrigatório para devices do domínio de ativos; - asset_id opcional somente quando device sem ativo tiver finalidade definida; - permitir relação many-to-many apenas se houver caso de domínio; **recommendation**: Definir cardinalidade e razão para device sem asset antes da SPEC; não inferir pela coluna opcional do SQL ilustrativo.; **consequence**: Altera modelo, API, autorização e integridade referencial; escopos de domínio, identidade e produto dependem dela.

- **decision**: Identidade/colisão/lifecycle de endpoint por protocol, external_id e gateway.; **alternatives**: - ID interno mais escopo externo explicitamente definido; - chave externa composta com unicidade aprovada por tenant/gateway/protocolo; - outro esquema que cubra atualização e substituição; **recommendation**: Especificar escopo de identidade e colisões sem inventar unicidade ou limite.; **consequence**: Afeta dedupe, ingestão, adaptadores, gateway e migração.

- **decision**: Operações/payloads/autorização para endpoints.; **alternatives**: - adicionar operações explicitamente ao contrato API comum; - definir endpoint como derivado internamente com fontes e atualização explicitadas; **recommendation**: Concluir contrato documental coordenado por Graydon (API) e Michael (modelo); não iniciar SPEC nesta entrega.; **consequence**: Sem isso, estímulo ponta a ponta e erros/retry não estão completamente definidos.

- **decision**: Integridade tenant-scoped entre device e asset e entre endpoint e device.; **alternatives**: - constraints/FKs compostas e autorização de serviço, com RLS complementar; - mecanismo equivalente com critérios negativos verificáveis; **recommendation**: Não depender de IDs opacos como autorização; explicitar FK/tenant e papel/RLS conforme modelo de dados.; **consequence**: SQL ilustrativo atual não demonstra integridade cross-tenant; validação fica para autorização futura.

- **decision**: Semântica de endpoint cadastrado versus observado/operacional.; **alternatives**: - cadastro separado de health operacional; - perfil integrado com timestamp/fonte/estado explicitamente tipados; **recommendation**: Separar associação de transporte de evidência de disponibilidade.; **consequence**: Evita status enganoso; depende de contratos de capacidade/readiness.

### dependencias

- Graydon Hoare: contrato de operações de asset/device/endpoint e wire

- Erich Gamma: distinção capacidade declarada/verificada/operacional

- Kim Cameron: principal/tenant e grants

- Alan Turing: integração; Edsger Dijkstra: revisão independente

- decisões de produto sobre cardinalidade e eventual UI por plataforma

### criterio fechamento documental

Atribuição de Michael é adequada para semântica de modelo, com interface de API explicitamente dependente de Graydon; critério especifica pré-condição, estímulo, esperado, invariantes, falhas e recuperação; cada produto/plataforma tem aplicabilidade justificada; cardinalidade, identidade, API, tenant integrity e status ficam como decisões com alternativas/recomendação/consequências; Alan integra e Edsger revisa. Não implica implementação ou validação.

### estado

acceptance_proposed_open_decisions

### future spec

NOT_ASSIGNED

### test case

NOT_ASSIGNED

### observed

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-003

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-erich.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Sujeito escopado, perfil/versionamento selecionado, capability/direção e contexto nomeados; classificação operacional exige fonte/método de evidência e política de frescor.

### Estímulo

Consumidor consulta a avaliação de uma capability para uso em contexto/destino concreto.

### Esperado

Resultado liga sujeito, capability, contexto e nível máximo sustentado. Declaração sem prova não aparece como verified/operational; operational exige evidência corrente e dependências específicas. Na projeção GMP proposta, operational[] ⊆ verified[] ⊆ supported[]; ausência não prova nível positivo. Unknown/stale/inválido não satisfaz rota dependente.

### Invariante

Evidência não se transfere entre sujeito, versão/perfil, direção, contexto ou origem materialmente diferentes; capability não concede autorização, rota nem readiness.

### Negativo

Perfil anuncia voice.tx sem prova atribuível: no máximo supported e nenhuma rota TX. Lista operacional que não esteja contida nas listas verified e supported é violação da projeção proposta.

### Degradação e recuperação

Mudança material, expiração ou validade incerta deixa de sustentar operational para novas rotas e expõe unknown/stale. Reavalia após nova evidência contextual; não restaura estado antigo nem altera AccessDecision automaticamente.

### principal documental

**name**: Erich Gamma

**status**: adequado_confirmado_para_fronteiras_e_interface_de_capabilities; atribuicao_original_proposta; integracao_pendente

**rationale**: A contribuição cobre contrato de interface e distinções de domínio. O proprietário de perfil/API conserva a forma de dados; não há alegação de confirmação por terceiros.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: tabela de requisitos, linha 9; **trecho**: Perfis declarados, verificados e disponibilidade operacional; **estado origem**: APROVADO como direção

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual, linha 89; **trecho**: Erich Gamma — proposto por especialidade, confirmação por requisito pendente; **estado**: atribuição proposta

- **arquivo**: docs/02-dominio/capacidades.md; **secao**: Perguntas separadas, linha 4; **trecho**: Uma resposta positiva em uma dimensão não implica resposta positiva nas demais.

- **arquivo**: docs/02-dominio/capacidades.md; **secao**: Níveis, linhas 7–9; **trecho**: evidência atual conforme uma política de frescor configurada

- **arquivo**: docs/02-dominio/capacidades.md; **secao**: Projeção cumulativa, linha 20; **trecho**: `operational[] ⊆ verified[] ⊆ supported[]`

- **arquivo**: docs/03-integracoes/gateway-universal.md; **secao**: Serviços e contratos, linhas 33–38; **trecho**: autorização e readiness são gates separados

- **arquivo**: docs/01-arquitetura/normalizacao-dados.md; **secao**: Contratos canônicos, linha 18; **trecho**: Preservar evidência contextual de capability, seleção de rota e decisão de autorização como resultados distintos

- **arquivo**: docs/01-arquitetura/normalizacao-dados.md; **secao**: linha 24; **trecho**: Ela não promove uma declaração `supported` a `verified` ou `operational`, e não transforma disponibilidade em autorização.

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK quando avalia sujeitos/capabilities

**plataformas**: - servidor Linux Rust

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

**justificativa**: Semântica é comum; fontes, permissões e hardware que produzem evidência variam por plataforma. Capability irrelevante para certo sujeito/dispositivo é não aplicável por perfil (ex.: voz TX em tag BLE sem áudio) e não deve ser presumida; o requisito global continua aplicável.

**fora de caso**: Operações sem o sujeito/capability avaliado não exercitam esse critério, sem remover a exigência para usos aplicáveis.

### decisoes pendentes

- **questao**: Autoridade, método e versão dos perfis/evidências; ratificação da projeção cumulativa.; **alternativas**: - Manter níveis/coleções cumulativos como proposta; - Tornar coleções independentes; - Usar enum único; **recomendacao**: Manter a projeção existente como proposta coerente com GMP v1 §7.3 até decisão do usuário.; **consequencia**: Alternativas mudam a compatibilidade de payloads e exigem revisão do contrato proprietário; nenhuma está ratificada por esta contribuição.

- **questao**: Política de frescor, por origem/capability/contexto, relógio incerto e evidência conflitante.; **alternativas**: - Validade configurada por classe/contexto; - Limite comum; **recomendacao**: Definir validade por classe/contexto depois dos cenários e fontes de evidência.; **consequencia**: Limite comum pode ser conservador ou permissivo demais; nenhum número/TTL aprovado.

### dependencias

- Dono de perfil/API confirma forma de dados e compatibilidade

- Kim/identidade mantém autorização separada

- Graydon reconcilia projeção GMP

- Produto decide criticidade e cenários

- Evidência/frescor precisam de política antes de operacionalidade

### criterio fechamento documental

Fonte de perfil, contexto, níveis e projeção reconciliados entre capacidades.md, GMP §7.3 e contratos de integração; desconhecido/stale, negativa e recuperação explicitados; decisões e dono por interface registrados, sem alegar implementação.

### estado

aberto; aceite comportamental proposto, decisões e confirmação de integração pendentes

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-004

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-erich.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Pedido identifica serviço/operação, direção, origem, destino/contexto e restrições; candidatos cadastrados com capability/validade avaliáveis; `AccessDecision` disponível; política/ordem de custo aprovada se houver comparação.

### Estímulo

Consumidor solicita rota ou condição material da rota muda e exige reavaliação.

### Esperado

Retorna rota elegível com serviço/destino/caminho e evidências/restrições, ou resultado sem rota com causa identificável. Toda capability obrigatória está sustentada/atual; autorização, gates de rota e readiness exigidos passam. Só compara custo com dados comparáveis e política aprovada; sem isso não declara rota ótima.

### Invariante

Capacidade, descoberta ou custo favorável não sobrepõe AccessDecision negativa/unknown nem restrições. Rota inelegível não executa efeito. O roteador consome decisão de policy; não a concede.

### Negativo

(a) autorização negada, sem TX/seleção; (b) capability stale ou rota/recurso indisponível, sem rota elegível; (c) bridge descoberta sem grant específico, sem TX; (d) custos incomparáveis, sem declarar vencedor otimizado.

### Degradação e recuperação

Falha distingue unknown, negativa de policy, ausência de capability e ausência de caminho. Reavalia alternativas com evidência e autorização atuais; não faz fallback que viole restrições. Após recuperação exige nova avaliação; não presume autorização restaurada nem mantém decisão vencida.

### principal documental

**name**: Erich Gamma

**status**: adequado_confirmado_para_interface_de_roteador; principal_proposto_permanece; ownership_de_autorizacao_e_custo_e_transversal

**rationale**: Erich pode especificar a composição/interface de rota; identidade/policy é dona da decisão de acesso, e produto define objetivo de custo. Não transfiro nem amplio a atribuição.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: tabela de requisitos, linha 10; **trecho**: Roteador que resolve serviço, destino, caminho, autorização e custo; **estado origem**: APROVADO como direção

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual, linha 90; **trecho**: Erich Gamma — proposto por especialidade, confirmação por requisito pendente; **estado**: atribuição proposta

- **arquivo**: docs/03-integracoes/gateway-universal.md; **secao**: Etapas de habilitação, linha 30; **trecho**: Busca de rota em grafo com função, direção, codec, latência, custo e validade.

- **arquivo**: docs/02-dominio/capacidades.md; **secao**: Dimensões independentes, linhas 16–22; **trecho**: Se autorização, rota, readiness ou evidência necessária for negativa, desconhecida ou não atual, a operação não é elegível

- **arquivo**: docs/03-integracoes/gateway-universal.md; **secao**: Avaliação, linhas 46–52; **trecho**: A rota só fica elegível quando a capability obrigatória está operacional e fresca, a autorização permite a ação e os gates de rota e recurso também passam.

- **arquivo**: docs/01-arquitetura/fronteiras-modulos.md; **secao**: Vocabulário, linhas 28–30; **trecho**: Roteador de serviços | Compara rota solicitada com capacidades atuais, direção, destino, restrições e decisão de acesso; retorna apenas rota elegível.

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux Rust

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

**justificativa**: Roteamento é transversal; contexto muda por tenant, destino, região, serviço e bridge. Clientes e servidores são consumidores/produtores em papéis distintos; não implica implementação presente.

**nao aplicavel quando**: Operação puramente local sem endpoint, serviço remoto ou escolha de caminho não exercita o roteador; justificar caso a caso, sem dispensar RF-004 globalmente.

### decisoes pendentes

- **questao**: Dimensões de custo e precedência/pesos/limites.; **alternativas**: - Filtros obrigatórios primeiro, depois política de produto lexicográfica/regras; - Função ponderada única; **recomendacao**: Filtros obrigatórios primeiro e custo como otimização segundo regra de produto explícita.; **consequencia**: Função ponderada exige métricas comparáveis e pesos aprovados; sem isso não declarar ótimo. Nenhum preço/peso/SLO inventado.

- **questao**: Restrições obrigatórias, dado de custo ausente e política de fallback/impacto em sessão.; **alternativas**: - Falhar fechado para restrição obrigatória desconhecida; - Fallback somente entre rotas reavaliadas e autorizadas; **recomendacao**: Não tratar desconhecido como permitido; reavaliar cada alternativa sob as mesmas condições.; **consequencia**: Continuidade pode ser reduzida quando não houver rota verificável; produto define prioridade de serviço e comportamento de sessão.

### dependencias

- Kim/C09 para AccessDecision e validade/revogação

- Graydon/C07 para interface/erros GMP

- capabilities.md para avaliações

- Vint/transportes para dados de rota

- produto/usuário para semântica de custo, prioridades e restrições

- readiness/serviço define gates próprios

### criterio fechamento documental

Interface separa pedido, rota elegível, decisão de acesso e custo; falhas/unknown/fallback e dono de cada dimensão documentados; significado e política de custo decididos; contratos proprietários coerentes, sem afirmar performance/implementação.

### estado

aberto; aceite comportamental proposto, semântica de custo/fallback e integração pendentes

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-005

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-radia.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Serviço elegível; participantes autenticados e com membership/ACL válida no canal; transport e capability compatíveis; política do canal definida; bridge desativada para provar caminho nativo. Ambiente futuro registra versões, grant e configuração.

### Estímulo

Dois participantes entram em canal/grupo; A solicita floor e transmite; B tenta TX simultaneamente; A libera e B solicita; repetir com usuário não autorizado e após falha de mídia/controlador.

### Esperado

Somente titular do grant vigente/epoch aceito transmite e o media edge valida; ouvintes autorizados recebem fluxo identificado; release revoga o direito segundo política; após resume novo stream e autorização atual antes de TX; cliente web e nativo comunicam pelo GodMesh com Mumble/FRN desligados.

### Invariante

No máximo um TX grant autorizado por domínio half-duplex; áudio recebido não concede transmissão; bridge opcional não é dependência do caminho nativo; grant expirado/obsoleto não é ressuscitado.

### Negativo

ACL/membership insuficiente, grant expirado/revogado, epoch antigo, stream sem vínculo, segundo transmissor concorrente ou capability de voz incompatível não produz áudio aceito. Desativar bridge não deve bloquear voz nativa.

### Degradação e recuperação

Distinguir falha de controle, mídia e sessão. Sem grant/epoch demonstrável, parar TX; failover pode encerrar sessão e não promete continuidade inaudível. Resume revalida ACL, emite stream/grant atuais e não retransmite áudio antigo; resultado não afirma recepção universal ou ACK humano.

### parametros abertos

- ordem/preempção de prioridade

- ACL e políticas por canal

- TTL/renovação/frescor de grant

- budgets de qualidade e fallback por plataforma

- condições de partição e critérios de admissão

### principal documental

**nome**: Radia Perlman

**status**: confirmado_por_mim_como_principal_documental

**justificativa**: Voz/PTT nativo está dentro do papel atribuído; owner operacional não designado.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Tabela de requisitos funcionais; **linhas**: 11; **trecho**: Voz nativa com PTT, grupos, prioridade e canais; **estado origem**: APROVADO como direção, não implementação.

- **arquivo**: docs/01-arquitetura/midia-nativa.md; **secao**: Invariantes; Aceitação; **linhas**: 34-39,54-55; **trecho**: Dois clientes nativos estabelecem voz PTT via GodMesh sem Mumble/FRN; prioridades, expiração e failover são testados; bridges são desligáveis e não causam impacto no núcleo.; **status**: especificação de engenharia não validada

- **arquivo**: docs/01-arquitetura/protocolo-nativo.md; **secao**: Máquina de estados PTT; **linhas**: 88,103-108; **trecho**: No modo half-duplex existe no máximo um transmissor efetivamente autorizado por canal/domínio de coordenação por vez.

- **arquivo**: docs/05-operacao/testes.md; **secao**: Convergência; **linhas**: 35; **trecho**: GodMesh Native PTT funciona com grupos/ACL, cliente web e cliente nativo sem servidor Mumble.

### aplicabilidade

**produtos**: **Personal**: aplicável a canais pessoais e grupos autorizados

**Teams**: aplicável a canais organizacionais/ACL; prioridades dependem de política aprovada

**Node**: aplicável ao serviço nativo que hospeda/roteia voz

**Platform SDK**: aplicável quando contrato/versão de voz for aprovado

**plataformas**: **Linux server Rust**: aplicável ao serviço de mídia planejado

**web**: aplicável; teste de referência exige cliente web e nativo

**macOS universal SwiftUI**: aplicável; suporte/versionamento ainda por validar

**iOS SwiftUI**: aplicável; suporte/versionamento ainda por validar

**Windows desktop**: aplicável no escopo integral; implementação posterior

**Linux desktop**: aplicável no escopo integral; implementação posterior

**Android**: aplicável no escopo integral; implementação posterior

**nao aplicavel**: - **alvo**: endpoint LoRaWAN/BLE data-only; **justificativa**: Visão do produto descreve LoRaWAN como dado curto sem áudio PTT de baixa latência; roteamento deve negar capability incompatível, não inventar voz.

**limite**: Fase futura inicial Linux/macOS/iOS não reduz cobertura planejada das demais plataformas; bridge interoperability é RF-007/008, não pré-condição de RF-005.

### decisoes pendentes

- **assunto**: prioridade do floor; **alternativas**: - ordem explícita por canal sem preempção implícita; - preempção em canais nativos configurados/autorizados; - ordem universal; **recomendacao**: política explícita por canal; preempção somente configurada/autorizada; não usar ordem universal como default; **consequencia**: preempção pode interromper fala nativa; fila sem preempção pode atrasar solicitação; **escopos**: - Personal; - Teams; - incident_channel; **responsavel da decisao**: usuário

- **assunto**: retomada e grants; **alternativas**: - retomar só após nova autorização; - reutilizar lease vigente apenas com prova atual no edge; **recomendacao**: revalidar membership/ACL e emitir grant atual ao retomar, conforme PI-radia-001; **consequencia**: menor disponibilidade durante isolamento em troca de evitar grant obsoleto; **escopos**: - todos os canais e plataformas; **responsavel da decisao**: usuário; contratos de autoridade e segurança dependentes

- **assunto**: transporte e limites; **alternativas**: - WebRTC browser e transports nativos avaliados por plataforma; - um transporte comum se suportado; **recomendacao**: usar profile comparativo e não presumir interop; conforme PI-radia-002; **consequencia**: versões/relay/budgets afetam suporte e custo; **escopos**: - web; - mobile; - desktop

### dependencias

- ACL/membership e contrato de grant/fencing

- autoridade em partição e comportamento de resume

- política de canal/prioridade

- C13-SLI-005: targets e método ainda abertos

- decisão de transportes e plataformas

### criterio fechamento documental

Política de grupos/ACL/prioridade/concessão/retomada aprovada; estados positivo/concorrente/negativo/degradado/recuperação ligados a contrato; parâmetros TBD identificados; cenário por produto/plataforma confirmado; SPEC/caso NOT_ASSIGNED e revisão independente. Não é SPEC nem validação.

### estado

planejado_parcial_com_decisoes_abertas

### futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### referencias comprovadas

- ADR-014

- ADR-021

- ADR-022

- ADR-028

- ADR-036

- PI-radia-001

- PI-radia-002

### natureza

direção de produto aprovada; aceite comportamental proposto; implementação não verificada

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-006

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-graydon.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Dois peers autenticados declaram major, intervalo minor, features e perfil de transporte; nenhum efeito de domínio foi aceito.

### Estímulo

Negociar: major/minor compatíveis; nenhum major comum; feature crítica requerida desconhecida; feature opcional desconhecida; schema major incompatível; transporte alternativo permitido ou não permitido.

### Esperado

Com interseção compatível, selecionar somente versão, features oferecidas/conhecidas/autorizadas e perfil publicado antes de aceitar operação. Sem major comum, feature crítica ou schema compatível, rejeitar explicitamente antes de decodificar/produzir efeito. Feature desconhecida só pode ser ignorada quando não muda autorização, semântica ou interpretação. Transporte permitido conserva semântica GMP e não causa downgrade silencioso.

### Invariante

Versão GMP, versão do schema, feature negotiation e transporte são dimensões distintas; não se introduz cifra/codec/transporte próprio; nenhum dado de aplicação precede handshake e autorização aplicáveis.

### Negativo

Bytes malformados/truncados, frame acima de limite publicado, enum/campo crítico desconhecido, autenticação inválida, incompatibilidade ou operação pré-handshake são rejeitados sem efeito persistido, downgrade ou uso ilimitado de recursos.

### Degradação e recuperação

Usar IPv4/HTTPS/WebSocket somente se o perfil permitir e preservar autenticação/semântica; caso contrário reportar indisponibilidade. Reconexão renegocia versão e identidade atuais. Após timeout, consultar a mesma identidade idempotente; não inferir sucesso. Tempos e limites numéricos permanecem abertos.

### principal documental

**nome**: Graydon Hoare

**estado**: confirmado como adequado para contrato GMP/wire; confirmação de política e perfis permanece pendente

### evidencias

- **fonte**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linha**: 92; **trecho**: RF-006 | 00-produto/requisitos.md:12 | APROVADO | 01-arquitetura/protocolo-nativo.md | Graydon Hoare — proposto por especialidade, confirmação por requisito pendente

- **fonte**: docs/00-produto/requisitos.md; **linha**: 12; **trecho**: RF-006 — Protocolo GMP próprio, versionado, sem criptografia própria — APROVADO

- **fonte**: docs/01-arquitetura/protocolo-nativo.md; **linhas**: 7-14; **trecho**: GMP sobre transportes estabelecidos; sem cifra, codec ou transporte próprio

- **fonte**: docs/07-contratos/gmp-api-v1.md; **secoes**: - 1; - 2; - 3; - 10; - 11; - 12; **trecho**: negociação de versão/features, framing, erros, casos de conformidade e limites propostos

- **fonte**: docs/06-planejamento/decisoes.md; **adrs**: - ADR-021; - ADR-026; - ADR-029; - ADR-031; - ADR-033; **trecho**: direções para GMP, ausência de criptografia própria, versionamento e compatibilidade

- **fonte**: entregas C07 anteriores; **id**: GMD-A03-001; **trecho**: proposta de handshake/wire; aprovação, artefatos formais e validação permanecem pendentes

### aplicabilidade

**escopo**: Todos os peers e plataformas participantes, inclusive Windows/Linux/Android futuros; primeira sequência prevista: servidor Rust Linux, macOS universal SwiftUI e iOS SwiftUI.

**na**: False

**justificativa na**: Não aplicável: conformidade é definida pelo wire compartilhado, embora perfis/capacidades variem por peer.

### decisoes pendentes

- **tema**: Perfil formal de handshake, Protobuf/packages/tags, features e ALPN; **alternativas**: - Ratificar perfil comum completo; - Restringir explicitamente o primeiro perfil interoperável; **recomendacao**: Preservar direção versionada e fail-closed; formalizar compatibilidade antes de liberar SPEC; **consequencia**: Aumenta previsibilidade; perfil restrito pode recusar peers antigos.

- **tema**: Transportes por plataforma, 0-RTT e limites de framing; **alternativas**: - Aprovar matriz de perfis e limites; - Deixar perfil sem transporte elegível e falhar fechado; **recomendacao**: Escolher por evidência de plataforma e ameaça, sem inventar números; **consequencia**: Determina interoperabilidade e disponibilidade; limites continuam bloqueio de prontidão.

### dependencias

- C09 identidade/autorização

- C10 replay, idempotência e IDs

- C06 autoridade

- C08 retenção

- ADR-033 compatibilidade/rollout

### criterio fechamento documental

Contrato versionado especifica handshake, compatibilidade, framing e limites por perfil, rejeições sem efeito e casos interoperáveis; revisão independente e aprovações correspondentes registradas. SPEC/testes seguem não atribuídos.

### estado

**planejamento aceite**: proposto

**SPEC**: NOT_ASSIGNED

**teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

**requisito origem**: APROVADO; direção não demonstra wire final nem implementação

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-007

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-gregor.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Instância Mumble cadastrada; autorização de usuário e operador externo vigentes; política de canal e versão/perfil do conector compatíveis; credencial de escopo disponível ao Bridge Host isolado.

### Estímulo

Solicitar recepção de canal ou transmissão via bridge Mumble.

### Esperado

Estabelecer somente a sessão/canal permitido, informar estado e capacidade; TX inicia somente após concessão PTT/autorização vigente; ao desativar a bridge, encerrar sessão externa e manter comunicação GodMesh nativa.

### Invariante

Mumble não é autoridade de identidade/canal nem dependência do núcleo; resultado externo permanece distinto do estado nativo.

### Negativo

Sem autorização em qualquer ambiente, perfil compatível, credencial ou permissão do canal, negar conexão/TX sem revelar segredo nem inferir identidade GodMesh por nome externo.

### Degradação e recuperação

Queda, timeout ou crash isola a integração e informa indisponibilidade; retry não repete efeito externo incerto automaticamente. Nova conexão revalida autorização e perfil.

### spec

NOT_ASSIGNED

### teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### principal documental

**nome**: Gregor Hohpe

**estado**: confirmado pelo autor para integração/bridge

**interfaces**: - Radia Perlman: voz e mídia

- Graydon Hoare: autorização e contrato GMP

### evidencias

- **arquivo**: Projects/GodMesh/docs/00-produto/requisitos.md; **linha**: 13; **trecho**: Mumble como bridge externa opcional, isolada; **estado fonte**: APROVADO como direção

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 7-10; **trecho**: Toda interoperabilidade atravessa um Bridge Host isolado e com política explícita.

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 59-66; **trecho**: Link desabilitado por padrão; aprovação explícita de usuário habilitado e operador do sistema externo.

- **arquivo**: Projects/GodMesh/docs/06-planejamento/decisoes.md; **linha**: 28; **trecho**: Mumble/FRN somente por Bridge SDK opcional e isolado; **estado fonte**: ADR-022 aprovada como direção

### aplicabilidade

**produtos**: Personal, Teams, Node e Platform quando a integração for configurada/autorizada; funções nativas permanecem disponíveis sem bridge.

**plataformas**: - servidor Linux

- macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

**racional**: A integração pode ser hospedada em Node/servidor; clientes exibem controles/estados conforme recurso. O princípio de independência é multiplataforma; não há evidência para excluir uma plataforma.

**nao aplicavel**: Nenhum produto/plataforma declarado não aplicável; ausência de conector validado não equivale a não aplicabilidade.

### decisoes pendentes

- **pergunta**: Quais instâncias, canais, sentidos RX/TX, direitos e regras de retenção podem ser habilitados?; **alternativas**: - aprovação por instância/canal/direção; - política de conta com escopos explicitamente delimitados; **recomendacao**: Bridge desligada por padrão e aprovação de instância, canal e direção separadas.; **consequencia**: Sem evidência, a operação permanece inelegível; dependem operador, política, privacidade e compatibilidade.

### dependencias

- ADR-022 e ADR-032

- perfil versionado/negociação UDNL-Bridge SDK

- política de autorização e credenciais

- C07 para envelope/erros quando houver efeito

- C08 para retenção de recibos quando aplicável

### criterio fechamento documental

Perfil descreve fluxo RX/TX, pré-requisitos de autorização, versão, capacidade, isolamento e resultados negativos/degradados; referências são integradas ao Bridge SDK e contratos UDNL/GMP. Conector, protocolo e operação reais validados em etapa futura autorizada.

### estado

aceite_proposto; dono confirmado; implementação e validação pendentes

### referencias existentes

- ADR-022

### estado conhecimento

**especificado**: direção e critérios em documentação

**aprovado**: opcionalidade e isolamento como direção

**implementado**: não verificado

**validado**: nenhum conector confirmado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-008

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-gregor.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Instância e operador FRN cadastrados; usuário e operador externo autorizaram escopo/direção; credencial e perfil aplicável válidos; licença e política do meio satisfeitas.

### Estímulo

Solicitar conexão/recepção ou transmissão FRN.

### Esperado

Estabelecer somente sessão permitida; reportar capacidade/estado; RX não implica TX; TX só após concessão de canal, autorização e validação atual de lease/fencing imediatamente antes do efeito. Desconexão preserva comunicação nativa.

### Invariante

Descoberta, conexão e recepção não autorizam TX; FRN não é núcleo GodMesh nem autoridade global de identidade.

### Negativo

Se credencial, operador, autorização, licença, fencing, protocolo ou compatibilidade faltar ou estiver desconhecido, negar TX e não promover a operação a autorizada.

### Degradação e recuperação

Perda de rede, lease ou autoridade impede novo TX. Envio sem confirmação mantém resultado desconhecido, sem retry com nova identidade. Reconexão revalida autorização e deduplicação e não retoma TX automaticamente.

### spec

NOT_ASSIGNED

### teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### principal documental

**nome**: Gregor Hohpe

**estado**: confirmado pelo autor para integração/bridge

**interfaces**: - Radia Perlman: voz e mídia

- Bruce Schneier/Kim Cameron: política, identidade e autorização, quando aplicável

### evidencias

- **arquivo**: Projects/GodMesh/docs/00-produto/requisitos.md; **linha**: 14; **trecho**: FRN como bridge externa opcional, isolada e autorizada; **estado fonte**: APROVADO como direção

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 48-53; **trecho**: exigir autorização do operador para qualquer bridge externa

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 59-66; **trecho**: No máximo um processo bridge TX ativo por endpoint externo segundo lease com fencing token

- **arquivo**: Projects/GodMesh/docs/06-planejamento/decisoes.md; **linha**: 34; **trecho**: Pontes RF/FRN usam autoridade exclusiva com fencing e TX opt-in; **estado fonte**: ADR-028 aprovada como direção

### aplicabilidade

**produtos**: Personal, Teams, Node e Platform quando configuração, operador e regras externas permitirem; política pode variar por ambiente externo.

**plataformas**: - servidor Linux

- macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

**racional**: Node/servidor pode hospedar a integração; clientes exibem estado/controles quando oferecem o recurso. Sem fonte para excluir plataforma.

**nao aplicavel**: Nenhum produto/plataforma declarado não aplicável.

### decisoes pendentes

- **pergunta**: Quais servidores, operadores, jurisdições, modos RX/TX, regras de auditoria e provas de fencing/cancelamento são aceitos?; **alternativas**: - aprovar somente RX até validação separada de TX; - aprovar RX/TX por servidor após evidência; - manter FRN desabilitado; **recomendacao**: Avaliar servidor e direção separadamente; manter TX bloqueado até prova e política aprovadas.; **consequencia**: Sem evidência legal/operacional e de destino, a operação permanece inelegível.

### dependencias

- ADR-022 e ADR-028

- C07/C09 para autorização e erros

- C10/C15 para comandos, replay e resultado desconhecido

- C08 para retenção

- operador externo, licença/jurisdição e validação autorizada

### criterio fechamento documental

Contrato por servidor/direção define evidência de operador, política, licença, lease/fencing, cancelamento e recuperação incerta; casos negativos e degradados não iniciam TX. Compatibilidade e homologação do destino permanecem validação futura.

### estado

aceite_proposto; dono confirmado; implementação e validação pendentes

### referencias existentes

- ADR-022

- ADR-028

### estado conhecimento

**especificado**: direção, controles e casos documentados

**aprovado**: opcionalidade e TX opt-in/fencing como direção

**implementado**: não verificado

**validado**: servidor e protocolo não verificados

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-009

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-paul.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Observação tem fonte, geometria, incerteza disponível, tempos observado/recebido, tenant e autorização. Localização pessoal exige consentimento vigente. Geofence tem geometria e escopo definidos.

### Estímulo

Registrar observações sucessivas e consultar proximidade, histórico, mapa e estado de geofence como perfis público, autorizado e Guardian.

### Esperado

Consultas retornam apenas dados autorizados e mostram fonte, qualidade e idade. Ausência de posição confiável não cria ponto. Geofence aceita inside, outside ou indeterminate; cruzamento da incerteza com limite não gera transição categórica. Posição atrasada permanece marcada stale.

### Invariante

Precisão pessoal não vaza para chat, PTT ou bridge. Região consentida em chat respeita raio mínimo de 50 km. Avistamento BLE é posição do observador, não do beacon. Geofence ou overdue não inicia SOS.

### Negativo

Sem consentimento, autorização, tenant correto ou grant vigente, não há coordenada/histórico. Pedido de precisão em chat não retorna o ponto. Indeterminate nunca vira entrada, saída, abandono ou emergência.

### Degradação e recuperação

GNSS ausente, PDR, fonte indireta ou rede offline mostram última observação confirmada, idade e incerteza disponível. Ao reconectar, eventos atrasados/fora de ordem não substituem silenciosamente estado mais novo nem duplicam alertas. Limiares ainda não aprovados.

### decisions open

- Classificação e efeito de indeterminate

- Frescor, tolerância/histerese e cadência por finalidade

- Retenção de histórico e tratamento de eventos fora de ordem

### principal documental

**owner**: Paul Ramsey

**status**: proposto_por_especialidade_e_confirmado_pelo_contribuidor

**basis**: A atribuição da matriz corresponde à especialidade de geoespacial, localização, geofencing e trajetória.

### evidencias

- **source**: docs/00-produto/requisitos.md:15; **literal**: Localização, proximidade, histórico, mapa e geofencing; **state**: APROVADO como direção, não implementação

- **source**: docs/02-dominio/modelo-dados.md:38-40,79-85; **literal**: observations: source, geometry, accuracy, occurred_at, received_at

- **source**: docs/01-arquitetura/guardian-localizacao-trajetoria.md:22-29; **literal**: Cada observação possui observed_at, received_at, source, accuracy_m ou distribuição de incerteza; rotas e geofences usam tolerâncias e avisos.

- **source**: docs/04-seguranca/politica-localizacao.md:7-23; **literal**: Chat usa região coarse de raio mínimo 50 km; precisão de fonte não se confunde com generalização intencional.

### aplicabilidade

**products**: - Personal

- Teams

- Node

- Platform/SDK

**platforms**: - Rust/Linux para serviço futuro

- macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

**not applicable**: Um produto sem mapa/geofence ativo pode declarar a interface específica indisponível; o requisito integral permanece aplicável ao serviço e às interfaces que ofereçam esses recursos.

**future platform rule**: Não reduz o escopo das plataformas preservadas.

### decisoes pendentes

**alternativas**: - Estado indeterminate sem alerta categórico

- Classificação binária com margem escolhida

- Adiar transições até fechar parâmetros

**recomendacao**: Adotar estado indeterminate quando evidência espacial não sustentar inside/outside.

**consequencia**: Sem limiar/janela aprovados, não se pode fechar o limite de idade, distância ou tolerância aceitável.

### dependencias

- Guardian e UX: efeito de alertas

- Michael Stonebraker: persistência de observações

- Política de localização e RF-029

- Decisão do usuário sobre parâmetros

### criterio fechamento documental

Aprovar semântica de estados, frescor, tolerância e ordenação; documentar casos de interior, exterior, fronteira ambígua, incerteza ausente, posição velha e observação atrasada; conciliar modelo, Guardian, política de privacidade, contrato e RF-009/RF-029. A proposta PI-paul-001 é complemento, não fechamento.

### estado

criterio_proposto; parametros e efeitos pendentes; implementação não verificada

### spec id

NOT_ASSIGNED

### test id

NOT_ASSIGNED

### observed execution

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-010

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jack.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Adaptador com perfil/versionamento, operações e limites declarados; dispositivo, rede, tenant e operador associados por evidência permitida; autorização, consentimento, licença RF e contexto avaliados antes da rota/efeito; proveniência e limites de identidade, horário e precisão preservados.

### Estímulo

Receber uplink LoRaWAN via ChirpStack/MQTT; evento por LoRa mesh via gateway compatível; observação BLE consentida de tag/beacon; posição/telemetria GPS/LTE de rastreador com protocolo declarado. Separadamente, se downlink for suportado, enfileirar comando e acompanhar apenas as confirmações que a origem realmente fornece.

### Esperado

Cada adaptador valida identidade/escopo, normaliza evento com origem, proveniência e incerteza; GPS/LTE usa `location.observed` sem mapear IMEI para gmi_id. LoRaWAN A respeita janela após uplink; C só se hardware/rede suportarem; LoRaWAN clássico não habilita PTT convencional. LoRa mesh só afirma rota/alcance sustentados pela topologia observada. BLE gera `proximity_sighting`, nunca posição GNSS do beacon, e só para detecção autorizada. Estados de fila, transmissão/entrega e confirmação funcional não se confundem. Capacidades não suportadas permanecem desabilitadas e explicadas.

### Invariante

Identificadores externos não viram identidade GodMesh sem prova; localização e proveniência mantêm origem/qualidade; autorização, rota e readiness continuam independentes; descoberta não habilita operação; dados privados não são projetados sem política; não há garantia de exactly-once ou entrega sem suporte observável do protocolo.

### Negativo

Identificador inválido/desconhecido, associação cross-tenant, falta de autorização/licença/consentimento, versão não suportada, payload inválido ou proveniência necessária ausente não habilitam rota ou efeito; não há busca BLE arbitrária, identidade inventada, precisão inventada ou sucesso inferido de fila aceita.

### Degradação e recuperação

Sem internet externa, o caminho LoRaWAN local só permanece disponível se ChirpStack/MQTT e IP local existirem; Classe A não recebe push fora da janela. Perda de dependência, permissão BLE, frescor do GPS ou evidência de origem torna a disponibilidade afetada indisponível/desconhecida. Após recuperação, nova evidência atribuível restaura avaliação; deduplicação segue a chave existente e não promete resultado entre reconexões quando a origem não tem chave estável. Não inferir eventos ausentes. Retry, spool, retenção e limiares continuam por decidir.

### principal documental

**nome**: Jack Ganssle

**estado**: confirmado_pelo_autor_para_integracao_de_adaptadores_de_dispositivos

**escopo e limite**: Principal para a integração documental do requisito e seus adaptadores de dispositivo/LoRa/BLE. GPS/LTE conserva interfaces e revisões cruzadas com localização, rede e plataformas móveis; não há atribuição separada desse subescopo na matriz atual.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linhas**: 16; **trecho**: | RF-010 | ChirpStack/LoRaWAN, LoRa mesh, BLE e GPS/LTE por adaptadores | APROVADO | `03-integracoes/lora-lorawan-ble.md` |; **sha256**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linhas**: 81-85, 96; **trecho**: | RF-010 | [origem L16](../00-produto/requisitos.md) | Direção APROVADA na origem; vínculo individual a ADR ainda a conciliar | [03-integracoes/lora-lorawan-ble.md](../03-integracoes/lora-lorawan-ble.md) | Jack Ganssle — proposto por especialidade, confirmação por requisito pendente | Observar: ChirpStack/LoRaWAN, LoRa mesh, BLE e GPS/LTE por adaptadores. Negativa/degradação e limiares: completar no registro comportamental. | Aberto: conciliar decisão, aceite e cenário com o principal; não liberado | NOT_ASSIGNED | NOT_ASSIGNED | NOT_EXECUTED |; **sha256**: c00e82090170872591cc5318f89b83d0a7af37294daae9e76e17795ec314e4e9

- **arquivo**: docs/03-integracoes/lora-lorawan-ble.md; **secao**: ChirpStack, Classes LoRaWAN, BLE, Firmware e modos offline; **linhas**: 12-32; **trecho**: - Confirmar que fila aceita o comando não prova entrega pelo dispositivo.; **sha256**: 70fa7c9663816962c155b419013c5f6d479ce06ea0aa8bee5702923479f697ab

- **arquivo**: docs/03-integracoes/gateway-universal.md; **secao**: Missão, normalização, habilitação, capability/rota/readiness e pipelines; **linhas**: 4-15, 18-38, 40-48; **trecho**: Toda bridge/adaptador cruza a Universal Data Normalization Layer: valida dados e identidade no contexto da rede de origem, preserva proveniência, transforma em eventos GMP e projeta para redes de destino de modo explícito e consciente de perdas.; **sha256**: 248b31e4a2807287af885544a8ad9cd2ece39d10305a8a50155f444a44a1b350

- **arquivo**: docs/03-integracoes/outros-adaptadores.md; **secao**: GPS e rastreadores celulares; perfil de origem/autorização; **linhas**: 1-4, 20-45; **trecho**: A integração GodMesh importa telemetria para o domínio (`location.observed`) e não confunde o IMEI ou outro identificador externo com a identidade principal do dispositivo.; **sha256**: d47ca32f104df6c1095b96b80301ea572d583f5af0b832ca8b603373ba81305f

- **arquivo**: docs/03-integracoes/crowdfind-beacon.md; **secao**: Papel do smartphone e Interoperabilidade e estado; **linhas**: 8-16, 41-48; **trecho**: Um avistamento por BLE não é uma coordenada do telefone anunciado.; **sha256**: f226bfad6a01ac6fae42c115a71569b3d58dfd09bc91b4378c1a63dbc6a082cb

- **arquivo**: docs/02-dominio/capacidades.md; **secao**: Perguntas separadas, níveis e dimensões independentes; **linhas**: 3-22, 27-36; **trecho**: Uma resposta positiva em uma dimensão não implica resposta positiva nas demais.; **sha256**: 7eead7494bdc8220988b952771161225f7dad50b942149843933785fa477480e

- **arquivo**: docs/01-arquitetura/normalizacao-dados.md; **secao**: Compatibilidade/lossiness, segurança, proveniência e ciclos; **linhas**: 253-274, 276-298; **trecho**: Localização GNSS ≠ localização estimada por gateway BLE ≠ posição declarada pelo usuário.; **sha256**: d77ecbb31031f9a70079bd05a322a871485040c08c467c4af7272e3ea0b9ad14

- **arquivo**: docs/06-planejamento/decisoes.md; **secao**: ADR-017, ADR-018, ADR-020, ADR-043 e ADR-044; **linhas**: 23-26, 56-57; **trecho**: | ADR-017 | ChirpStack/LoRaWAN como adaptador para dados | Aprovada |; **sha256**: 11321182f7b87fbfdd2c2f12dd3273216e64fc623727cf03413728954b2f8903

### aplicabilidade

**produtos**: - Personal quando usa BLE/GPS ou terminal compatível

- Teams quando usa rastreadores, gateways ou rádio autorizado

- Node como host de adaptadores ChirpStack/MQTT e gateways

- Platform/SDK na interface versionada de adaptadores

**plataformas**: Servidor Rust/Linux; macOS universal e iOS SwiftUI na primeira implementação futura, somente quando produto/plataforma atuar como adaptador ou consumidor e houver API/hardware permitido; Android, Windows e Linux desktop continuam no escopo integral futuro.

**nao aplicavel**: PTT convencional de baixa latência sobre LoRaWAN clássico é expressamente excluído pelo contrato/ADR-018; critério exige que a rota de voz seja recusada. Nenhuma exclusão geral de produto/plataforma. Ausência de API/hardware é estado/limite do perfil, não isenção do requisito.

### decisoes pendentes

- **decisao**: Modelos, gateways, protocolos e operações habilitados por classe.; **alternativas**: - Allowlist de modelos homologados por instalação; - Perfis versionados para terceiros, aceitos somente após cumprir gates definidos; **recomendacao**: Permitir apenas perfis versionados e autorizados, com allowlist por instalação antes de ativar operação.; **consequencias**: Define cobertura de produto e trabalho de integração/homologação; não reduz o escopo integral.

- **decisao**: Jurisdição, permissão de RF, potência, duty cycle e autoridade de TX.; **alternativas**: - Bloquear TX até validar licença e regras aplicáveis; - Habilitar somente classes/regiões explicitamente homologadas; **recomendacao**: Manter TX inelegível sem evidência de autorização e aplicabilidade legal.; **consequencias**: Restringe operação RF por localidade; dados RX só podem prosseguir se houver base autorizada.

- **decisao**: Garantias de downlink e de confirmação por operação/dispositivo.; **alternativas**: - Expor somente o último estágio confirmado pelo protocolo; - Exigir confirmação funcional nos comandos que dependem de efeito terminal; **recomendacao**: Nunca elevar o estado além da última evidência; decidir por operação quais confirmações são exigidas.; **consequencias**: Afeta UX, retry e roteamento; protocolos sem confirmação não poderão prometer entrega/executado.

- **decisao**: Identidade/proveniência, consentimento, retenção, localização e freshness por operação.; **alternativas**: - Manter avaliações independentes e classes de dados por política; - Definir condições específicas por produto/tenant e fonte; **recomendacao**: Preservar as dimensões independentes e fontes/qualidade atuais; não escolher TTL sem decisão.; **consequencias**: Determina elegibilidade da rota e tratamento de dados; validade/frescor segue sem valor aprovado.

- **decisao**: APIs, versões de SO, permissões e modelos suportados para BLE e GPS/LTE.; **alternativas**: - Homologar modelos/plataformas após seleção de hardware; - Manter suporte condicional a evidência de API/permissão por contexto; **recomendacao**: Decidir após escolha dos modelos e plataformas-alvo; não assumir disponibilidade contínua em background.; **consequencias**: Delimita interfaces de app/adaptador e casos de aceitação por plataforma.

### dependencias

- RF-003 e RF-004

- UDNL e SourceProvenance

- Identidade/tenant e política de autorização

- Política RF/licença e jurisdição

- Contratos de location.observed e retenção

- Perfis e versões de hardware/firmware

- APIs/permissões de plataformas móveis

- Dependências gateway/rede

- Revisão dos contratos de localização e rede para GPS/LTE

### criterio fechamento documental

Origem e contratos revisados pelo principal e interfaces; cada classe/operação tem critérios e limites explícitos de identidade, proveniência, autorização, confirmação e degradação; decisões pendentes registradas; matriz aponta SPEC e casos futuros com IDs reais quando existirem. Nenhum aceite documental declara produto validado.

### estado

aceite_comportamental_proposto; dependencias_e_decisoes_pendentes; nao_liberado

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-011

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Martin Kleppmann. Subescopos e deltas vinculados: [Conciliação kleppmann](#conciliacao-kleppmann). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Ator e destino autorizados no escopo/tenant; evento/comando identificado; política de retenção/expiração aplicável; inicialização em estado criado/local, sem alegar confirmação que não existe.

### Estímulo

Exercer fluxos de mensagem, presença, notificação e SOS, registrando marcos de persistência, submission/aceite, recibo de dispositivo e ação humana quando cada contrato os suporta.

### Esperado

Cada marco é ligado a evidência e exibido separadamente. Estados não observáveis ficam unknown/confirmation_unavailable ou equivalente; delivered só aparece após predicado de domínio aprovado e comprovado.

### Invariante

ACK de transporte não é reconhecimento humano; presença não é autorização; claimed SOS exige ação autenticada autorizada; nenhum estado ultrapassa evidência disponível.

### Negativo

MQTT/gateway aceita sem recibo final: não marcar delivered/acknowledged_by_person; timeout desconhecido não cria segunda operação; operação sem suporte a ACK explicita confirmação indisponível.

### Degradação e recuperação

Offline mantém pendência e idempotência; após reconexão a redelivery é deduplicada e não repete efeitos; estado só avança por nova evidência persistida.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: False

**estado**: conflito_de_escopo

**principal recomendado**: Martin Kleppmann

**interfaces recomendadas**: - Nancy Leveson para ACK/estados SOS

- Graydon Hoare para contrato de serviço

- Lea Verou para apresentação ao usuário

**fonte**: CONTEXTO-E-PAPEIS.md: Kleppmann é principal transversal de eventos/sync; Nancy é principal de SOS.

**justificativa**: RF-011 abrange mensagens, presença, notificações, SOS e ACKs; excede segurança operacional SOS.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linhas**: 17; **trecho**: Mensagens, presença, notificações, SOS, ACKs por etapa; **estado origem**: APROVADO como direção

- **arquivo**: docs/02-dominio/eventos.md; **secao**: Estados de um comando; SOS; **linhas**: 39-50, 58-59; **trecho**: O estado do comando, o resultado de cada tentativa de transporte e o reconhecimento humano são dimensões separadas.; **estado**: especificação; implementação não verificada

- **arquivo**: docs/07-contratos/eventos-sincronizacao.md; **secao**: Estados de comando e evidências; **linhas**: 13-47; **trecho**: HumanAcknowledgment ... Nunca inferido do ACK de rede.; **estado**: proposta v0.1 não congelada

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 97; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - Web

- macOS universal

- Windows

- Linux

- iOS

- Android

- servidor Linux

**regra**: Aplica aos fluxos suportados de emissão/consumo; UI e SDK preservam estados do serviço. Node não fabrica reconhecimento humano.

**nao aplicavel**: Não há exclusão geral. Quando um transporte/dispositivo não oferece confirmação fim a fim, a etapa não pode ser testada como confirmada; o sistema declara essa limitação.

### decisoes pendentes

- **decisao**: Definir predicado de delivered por classe de mensagem/notificação e quais ações requerem ACK humano.; **alternativas**: - Um predicado geral; - Predicados separados por operação; - Não projetar delivered até definir por operação; **recomendacao**: Predicados separados por operação; omitir delivered enquanto faltar prova contratual.; **consequencia**: Sem escolha, valida-se somente a distinção de camadas; aceitação de entrega fica aberta.

- **decisao**: Definir perfil de destinos, TTL, cobertura e comportamento offline.; **alternativas**: - Política global; - Política por tenant/canal/operação; **recomendacao**: Política por escopo e capacidade, sem números presumidos.; **consequencia**: Determina validade e elegibilidade do resultado.

### dependencias

- Contrato de eventos e sync revisado por Kleppmann/Graydon

- Política de retenção e autorização

- Critérios de UI por Lea

- Parâmetros de TTL/cobertura decididos

### criterio fechamento documental

Dono principal e interfaces aprovados; cada classe coberta tem marcos, significado, evidência, estado desconhecido, predicado de conclusão, offline/replay e critérios negativo/degradado registrados; versões e campos wire definidos sem inferir implementação.

### estado

proposta comportamental documental; atribuição conflitante; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-012

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-leslie.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Em configuração futura identificada, existem pelo menos dois full-nodes regionais e a solicitação tem identidade/tenant/serviço autorizados. Cada candidato tem versão/protocolo, dependências, readiness, capacidade reservável, acesso a dados e estado de autoridade/epoch com evidência/frescor definidos. As quantidades e metas continuam TBD; este cenário não supõe disponibilidade ou carga medida.

### Estímulo

Encaminhar operação elegível; em cenários isolados tornar um nó unready/unknown, remover dependência requerida, exceder limite de capacidade aprovado, ou perder acesso ao quórum; depois restaurar nó/região e solicitar reentrada no conjunto elegível.

### Esperado

- Escolher apenas candidato que satisfaz compatibilidade, autorização, credencial, readiness, política de dados e reserva de capacidade; RTT/custo/afinidade só ranqueiam os elegíveis.

- Não declarar mutação committed antes do commit na autoridade vigente; registrar resposta/rejeição e seu marco sem revelar dados de outro tenant.

- Após falha local, outra instância pode receber nova operação somente se elegível; não alegar migração contínua de sessão de mídia quando a especificação admite interrupção.

- Ao retorno, revalidar versão, serviço/dependências, escopo/dados, autoridade/fencing e readiness antes de readmitir.

- Em perda de região, overflow só se autorização, residência, rota, readiness e capacidade do destino forem provadas.

### Invariante

- No máximo uma autoridade vigente por partição sensível; o destino efetor rejeita epoch/fencing obsoleto.

- Health, ping, RTT ou timeout não substituem autoridade, autorização, residência ou readiness.

- Estado unknown em requisito obrigatório não é saudável nem capacidade livre.

- Elegibilidade é por serviço, rota, tenant e classe de dado, não por status global do nó.

### Negativo

Recusar nova operação se tenant/rota não autorizados, credencial inválida, versão incompatível, dependência requerida sem evidência, readiness stale, capacidade ausente ou escopo/residência proibidos. Nó saudável sem autoridade não aplica escrita forte; timeout não elege escritor. Falha de bridge não altera readiness do serviço nativo por inferência.

### Degradação e recuperação

Sem quórum, suspender escritas fortes; permitir apenas eventos/operações locais expressamente autorizados, mantendo-os pendentes. Em falha de nó/região, não fazer overflow a destino desconhecido. Na recuperação, fencing do escritor antigo e replay autorizado/idempotente por snapshot/cursor contíguo; reabrir admissão só após evidência atual. Falha de mídia pode interromper PTT; não prometer seamless.

### limiares

População, pico, margem, latência, capacidade, freshness, histerese, cooldown, SLO/RPO/RTO e custo são decisões/benchmarks pendentes; sem números inventados.

### principal documental

**name**: Leslie Lamport

**status**: confirmed_for_distributed_topology_and_consistency

**reason**: The requirement concerns regional full nodes, authority/failover semantics and balancing constraints, matching the assigned distributed-systems remit.

**interfaces**: - Alan Turing: matrix integration

- Martin Kleppmann: durable event synchronization

- Graydon Hoare: GMP/API errors

- Brendan Gregg and service owners: readiness evidence and operations

**operational owner**: not assigned by this confirmation

### evidencias

- **file**: docs/00-produto/requisitos.md; **section**: Requisitos integrais; **lines**: 18; **quote**: RF-012 | Vários servidores full-stack regionais com balanceamento | APROVADO; **sha256**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664

- **file**: docs/01-arquitetura/nos-regioes.md; **section**: Região; Governaça sem SPOF; Autoridade de escrita por tenant/partição; **lines**: 12-13,33-40; **quote**: Cada região comporta vários `full-node` [...] com balanceamento de entrada e peer mesh regional.

- **file**: docs/01-arquitetura/balanceamento.md; **section**: Escolha de nós; Readiness e efeito de admissão; Cenários de seleção e capacidade; **lines**: 16-19,30-36,50-58; **quote**: Critérios obrigatórios antes da pontuação: versão e protocolo compatíveis; tenant autorizado; credenciais válidas; serviço healthy; capacidade reservada disponível; acesso a dados ou rota para obtê-los.

- **file**: docs/01-arquitetura/autoridade-escrita.md; **section**: 2-9; Decisões ainda abertas e prontidão; **lines**: 18-28,67-106,141-143; **quote**: Bloqueio de prontidão: não anunciar autoridade/failover de produção até ADR decidir essas opções [...]

- **file**: docs/06-planejamento/decisoes.md; **section**: ADR-005, ADR-011, ADR-012; **lines**: 11,17-18; **quote**: Full nodes com serviços completos e regiões Active-Active | Aprovada

### aplicabilidade

**provedores**: - servidor Rust em contêiner Linux

- full-node regional

- edge-node para serviços locais dentro do seu escopo autorizado

**produtos**: **Personal**: consome serviços/roteamento; tenant e classe de dado dependem da política aprovada

**Teams**: consome serviços/roteamento com isolamento organizacional e escopos definidos pelo produto

**Node**: provedor central, regional ou edge conforme papel instalado; não se presume que todo Node tenha topologia idêntica

**Platform/SDK**: consome endpoints e versões, sem autoridade para contornar o roteador

**plataformas**: **iOS SwiftUI, macOS universal SwiftUI, Windows/Linux desktop, Android, Web**: clientes observam estado/retry e conectam-se aos endpoints; não são, por serem clientes, servidores full-stack regionais nem relays persistentes

**nao aplicavel como provedor**: Cliente UI não provê o cluster regional; isso é limite da superfície do requisito e não exclusão do aceite de sistema. Comportamento da interface pertence aos owners de cliente.

### decisoes pendentes

- **question**: Particionamento/propriedade, home region/residência, votantes/placement, mecanismo de quorum e handoff/fencing?; **alternatives**: - home_region estável + líder único por partição/epoch sob quorum, recomendação documental ainda não aprovada; - líder por partição sem home_region normativa; - serviço dedicado de autoridade; **recommendation**: Manter proposta C06 de home_region + único escritor como candidata; usuário/arquitetura decide e registra ADR.; **consequence**: Sem autoridade atual demonstrável, fail-closed para mutações fortes; não se pode anunciar failover produtivo.

- **question**: Quais cargas, falhas toleradas, objetivos por rota e orçamento de capacidade?; **alternatives**: - Definir após perfil aprovado e baseline medido (recomendado); - Definir metas de produto antes do benchmark sem tratá-las como medidas; **recommendation**: Registrar targets apenas com owner, população, janela e evidência; manter todos os números em aberto até decisão/baseline.; **consequence**: Não há claims de escala, latência, disponibilidade ou custo; gates de capacidade/failover continuam bloqueados.

### dependencias

- Contrato por rota/readiness e observabilidade com fonte/frescor/owner.

- C06 autoridade; C10 cursors/Inbox/replay; C07 GMP/erros; C08 retenção/revogação.

- Produto define workload/topologia e SLO/RPO/RTO; medições futuras autorizadas.

- Alan integra referências mantendo distinção entre proposta, aprovado e validado.

### criterio fechamento documental

Matriz contém precondição, estímulo, resultados, invariantes, negativos, degradação/recuperação e aplicabilidade por provedor/consumidor; decisões têm alternativas, recomendação, consequência e responsável; contratos/ADRs conciliados e revisão independente concluída. Futuras SPECs/testes só são ligados quando efetivamente atribuídos; planejamento não declara produto validado.

### estado

proposed_pending_decisions_and_review

### future spec

NOT_ASSIGNED

### test case

NOT_ASSIGNED

### observed

NOT_EXECUTED

### implementation

NOT_VERIFIED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-013

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-kleppmann.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Nós participantes estão autorizados; tenant/stream/escopo e política eficaz estão identificados; evento versionado e cursor/checkpoint persistidos; para mutação sensível, autoridade, epoch e prova estão vigentes. Sem autorização ou política de replay, serving/replay protegido fica bloqueado.

### Estímulo

Origem grava alteração e Outbox na mesma transação local; receptor solicita a partir do cursor autorizado, valida escopo, schema, integridade, autorização e ordem, persiste Inbox/checkpoint e materializa ou registra projeção recuperável.

### Esperado

Só o escopo concedido é entregue. ACK confirma apenas o prefixo contíguo durável de Inbox; não afirma materialização ou confirmação humana. Lacuna mantém cursor parado; depois de reconexão, replay continua do checkpoint recuperável sem origem única permanente. Snapshot só inicia replay depois de validação de escopo e integridade.

### Invariante

Não entregar dados fora do escopo; revalidar autorização; streams independentes não compartilham sequência; nenhum skip silencioso; checkpoint é durável e contíguo; ACK de Inbox não significa projeção concluída; sem promessa exactly-once.

### Negativo

Identidade repetida com digest igual deduplica; mesma identidade com digest diferente falha fechado sem sobrescrever. Evento futuro antes da lacuna fica pendente. Autorização revogada, schema não suportado, prova inválida, epoch obsoleto, política ausente ou snapshot parcial não avançam checkpoint nem servem dados protegidos.

### Degradação e recuperação

Indisponibilidade mantém retry/backpressure sob política aprovada. Cursor expirado demanda snapshot/rebootstrap autorizado, nunca salto. Após retorno, continuar desde checkpoint. Restore que não prova último checkpoint/Outbox reconcilia com fonte mais recente ou bloqueia serving/escrita afetados. RPO/RTO e disponibilidade não têm valores decididos.

### principal documental

**name**: Martin Kleppmann

**status**: confirmado_por_autor_como_principal_documental_de_eventos_sync

**conflito**: None

**limite**: Decisões de produto e responsabilidade operacional permanecem com seus donos.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao linha**: linha 19; **trecho**: Federação global e sync escopado, sem origem única permanente; **estado**: APROVADO na origem

- **arquivo**: docs/01-arquitetura/replicacao.md; **secao linha**: §Fluxo linhas 23-33; §Snapshot + replay linhas 35-40; §Escopo de replicação e §Autoridade de escrita linhas 51-55; **trecho**: ACK ao remetente declara apenas o maior prefixo contíguo persistido no Inbox; **estado**: critério arquitetural especificado; implementação não verificada

- **arquivo**: docs/07-contratos/eventos-sincronizacao.md; **secao linha**: §§1-9, especialmente §§4-6 e §9; **trecho**: contrato transversal proposto v0.1; wire e políticas ainda pendentes; **estado**: proposta documental, não norma aprovada

### aplicabilidade

**produtos**: - Node e serviços federados: direta

- Teams multi-região: direta

- Personal e Platform/SDK: quando sincronizam dados sob escopo

**plataformas**: - Servidor Linux: direta nos serviços de replicação

- macOS universal e iOS SwiftUI: consumo/retomada apenas se o cliente implementar cursor/outbox local

- Windows/Linux desktop, Android e web: permanecem no escopo integral e aplicam-se quando implementarem fluxo de sync

**nao aplicavel**: Nenhum produto/plataforma declarado globalmente não aplicável. Um componente só poderá ser isento se seu dono demonstrar que não persiste nem consome sync durável; isso não exclui o sistema daquela plataforma.

### decisoes pendentes

- **questao**: Namespace de sequence/cursor; **alternativas**: - global por origem; - independente por origem/tenant/stream/authority_epoch; **recomendacao**: independente por namespace para não bloquear streams distintos, após alinhamento C11/C06/C07; **consequencia**: afeta unicidade, migração, envelope, snapshot, cursor e compatibilidade

- **questao**: Fila/log durável; **alternativas**: - NATS JetStream como transporte; - log próprio; **recomendacao**: decidir após comparar operação/replay/retention e manter Outbox/Inbox como registros autoritativos; **consequencia**: altera topologia e recuperação; cache Redis não substitui fonte autoritativa

- **questao**: Política de retenção/revogação; **alternativas**: - política aprovada por classe/cópia/jurisdição; - bloquear replay sem política eficaz; **recomendacao**: fail-closed até configuração eficaz; **consequencia**: impacta serving, replay, custo, residência e rebootstrap offline

- **questao**: Autoridade e recuperação; **alternativas**: - somente operações permitidas localmente durante partição; - exigir autoridade/quórum para mutações fortes; **recomendacao**: preservar distinção por tipo de operação conforme arquitetura até ratificação C06; **consequencia**: impacta escrita offline, handoff e convergência

### dependencias

- Leslie/C06: autoridade, epoch e handoff

- Graydon/C07: cursor, wire, erros e compatibilidade

- Bruce/C08: retenção/revogação por classe/cópia

- Kim/C09: identidade e reautorização

- Michael/C11: persistência e unicidade

- usuário: políticas, carga, SLO/RPO/RTO

- Ada: integrar cobertura à matriz

- Edsger: revisão independente

### criterio fechamento documental

A regra aprovada de escopo/ordem/cursor aparece coerentemente no envelope, modelo, contrato, GMP, snapshot/replay e compatibilidade; aceites cobrem duplicata, conflito, lacuna, snapshot, epoch, revogação, partição e recuperação por produtos/plataformas aplicáveis; dependências e decisões têm donos e evidência futura identificada. Não implica SPEC, implementação ou validação.

### estado

parcial

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-014

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-vint.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Peer and service are authenticated and authorized; endpoints, supported protocols and network policy are available; compatible GMP version and minimum capabilities are known; the applicable route profile is identified. Missing or stale endpoint or authorization evidence makes the path ineligible.

### Estímulo

Start an IP connection in dual-stack, IPv6-only and IPv4-only conditions; make the active address family unavailable during a session and later restore it. Include a future protocol candidate only if it is declared implemented and allowed.

### Esperado

In dual-stack, try IPv6 first when the current profile considers it reachable and eligible, then use IPv4 if IPv6 fails or is ineligible. In IPv4-only and IPv6-only conditions, connect through the available compatible family. Report the selected route and recovery rule; keep transport negotiation independent from GMP version negotiation. Select a future protocol only when implemented, declared and negotiated under policy.

### Invariante

Peer cryptographic identity does not change with its address. A family change cannot expand authorization or scope. IPv6 failure cannot downgrade authentication. Expired endpoints, incompatible protocols, unknown authorization and destinations without policy cannot be admitted. Reachability is current evidence for the route and service, not an assumed ping or public IP.

### Negativo

When endpoints are missing or expired, protocol versions conflict, policy denies the route or authorization is unknown, do not mark a private service connection successful. Return a categorized reason and unavailable or unknown route state. Do not use an insecure path to complete fallback.

### Degradação e recuperação

Use IPv4 only when eligible and compatible after IPv6 failure. If neither family works, report no eligible route and keep operation state separate from delivery status. Relay remains conditional on authentication, policy and quota. After a route returns, apply the approved hysteresis and residence rule; no values are approved yet. Do not promise uninterrupted session migration.

### future spec and test ids

NOT_ASSIGNED

### observed

NOT_EXECUTED

### principal documental

**name**: Vint Cerf

**status**: confirmed_for_documentary_transport_acceptance

**basis**: The matrix assigns Vint to RF-014; transport and dual-stack selection are within the assigned specialty. Platform/provider owners still need to confirm applicability.

### evidencias

- **file**: docs/00-produto/requisitos.md; **section**: Requisitos integrais — rastreabilidade; **line**: 20; **sha256**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664; **quote**: | RF-014 | IPv6 preferencial e IPv4 fallback; extensibilidade futura | APROVADO | `01-arquitetura/transporte.md` |

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **section**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **line**: 100; **sha256**: c00e82090170872591cc5318f89b83d0a7af37294daae9e76e17795ec314e4e9; **quote**: | RF-014 | [origem L20](../00-produto/requisitos.md) | Direção APROVADA na origem; vínculo individual a ADR ainda a conciliar | [01-arquitetura/transporte.md](../01-arquitetura/transporte.md) | Vint Cerf — proposto por especialidade, confirmação por requisito pendente | Observar: IPv6 preferencial e IPv4 fallback; extensibilidade futura. Negativa/degradação e limiares: completar no registro comportamental. | Aberto: conciliar decisão, aceite e cenário com o principal; não liberado | NOT_ASSIGNED | NOT_ASSIGNED | NOT_EXECUTED |

- **file**: docs/01-arquitetura/transporte.md; **section**: Contrato; Seleção de caminho; Protocolos e camadas; Segurança; **lines**: 7, 10-15, 19-28, 33-34; **sha256**: 4aa7e7c8eee421f21d12fdaa166f3694eabe06cda1a6cb4606b5539351363f76; **quotes**: - **IPv6-first** para enlaces IP entre nós e para clientes capazes, com fallback transparente a IPv4 quando necessário.; - 3. Preferir IPv6 medido como alcançável; tentar IPv4 de maneira escalonada (*Happy Eyeballs*), sem esperar um timeout excessivo.; - Tentar relay autenticado se conexão direta falhar; impor quotas e evitar loops.; - A versão de transporte e a versão **GMP** são negociadas independentemente.

- **file**: docs/01-arquitetura/estrategia-tecnologica.md; **section**: Zona B; Negociação compatível; **lines**: 53-64, 80-85; **sha256**: f79e0be0294f612c472d478358d123756fa6b83e124e08ce60d72750bb332c7d; **quotes**: - IPv6 preferencial com IPv4 funcional; ambos testados; - se QUIC não estiver disponível, uma sessão compatível pode usar TCP/TLS, HTTP ou um relay autenticado sem redefinir a semântica GMP

- **file**: docs/06-planejamento/decisoes.md; **section**: ADR-006 e ADR-007; **lines**: 12-13; **sha256**: 11321182f7b87fbfdd2c2f12dd3273216e64fc623727cf03413728954b2f8903; **quotes**: - | ADR-006 | IPv6 preferencial, IPv4 alternativo | Aprovada |; - | ADR-007 | QUIC/libp2p como base candidata para federação (validar) | Em validação |

### aplicabilidade

**products**: - Personal

- Teams

- Node

- Platform/SDK

**platforms**: Every IP-capable component that implements a dual-stack transport. Desktop, iOS and Android clients participate only for transports exposed by their OS and declared by their provider; availability is unverified. Radio-only or LoRa endpoints have no direct IP-family test; the associated IP gateway remains in scope.

**not applicable**: Direct IPv4/IPv6 behavior is not applicable to a radio-only endpoint with no IP interface. Record the reason and assess its IP gateway separately. Do not infer platform support or lack of support without evidence.

### decisoes pendentes

**items**: - **decision**: Define the versioned route profile, reachability evidence and observation window; approve attempt timing, timeout, hysteresis, recovery and cost/egress by service after baseline; decide when authenticated relay is eligible; ratify QUIC/libp2p against ADR-007.; **alternatives**: - One global profile: simpler, but may not fit different services and networks.; - Independent local profiles: flexible, but can diverge between providers.; - Common invariants with versioned per-service/platform parameters: recommended; requires owner and approval per profile.; **recommendation**: Adopt common invariants and versioned profiles; do not set numeric thresholds before baseline and approval.; **consequence if open**: Selection, fallback and recovery criteria remain open and may differ by provider.; **scope**: - IP-capable GodMesh components; - all services using route selection; - provider implementations

### dependencias

- RF-024 transport/version negotiation and its future conformity criteria

- ADR-006 approved IPv6 preference

- ADR-007 QUIC/libp2p remains in validation

- Chet Haase and Donny Wals for platform transport capabilities

- Radia Perlman for media path requirements; Leslie Lamport for partition and failover semantics

- Brendan Gregg for measurement method and evidence

- Product owner for service priorities, cost and interruption policy

### documentary links

- ADR-006

- ADR-007

- PI-vint-001

- GMD-A24-001

### criterio fechamento documental

The transport document and traceability matrix reference one versioned route profile; each profile identifies eligible route evidence, thresholds, owner, unit, window, baseline, version and approval; future cases cover dual-stack, fallback, protocol negotiation, denied paths and recovery. Documentary completion does not establish runtime validation.

### estado

acceptance proposed; profile parameters and product decisions open; implementation not verified; validation NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-015

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-kim.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Node instalado com chave/ID local e sem aprovação de federação; peers candidatos e trust store disponíveis; parâmetro de classe de certificado configurado (pendente para prontidão produtiva).

### Estímulo

Descobrir peer, autenticar handshake, apresentar convite/certificado com issuer, audience, escopo, validade, proof e epoch; solicitar ingresso e snapshot escopado; repetir sem convite, com issuer/audience errado, escopo ampliado, estado expirado/revogado/unknown/stale ou epoch antigo.

### Esperado

Instalação e descoberta deixam o nó em awaiting_authorization/isolated; só certificado autorizado permite joining/synchronizing dentro do escopo; snapshot privado e readiness federada não são publicados antes da aprovação.

### Invariante

Descoberta, DNS, IP ou handshake não significam trust; o nó não amplia tenant_scopes; nenhum dado privado é replicado sem ingresso e autorização.

### Negativo

Certificado inválido/ausente/fora do escopo ou status incerto mantém peer isolado sem snapshot, governança, stream privado ou readiness federada.

### Degradação e recuperação

Sem autoridade/frescor, ingresso novo falha fechado; grants offline só podem suportar ações locais reversíveis se duração/escopo forem aprovados. Na reconexão validar estado atual, aplicar revogação antes de retomar stream e sincronizar eventos sob epoch vigente; instalação/upgrade interrompidos seguem procedimento operacional futuro.

### principal documental

**nome**: Kim Cameron

**status**: proposto_na_matriz_confirmado_para_gate_de_autorizacao

**justificativa**: Adequado para identidade/certificado, proof, escopo, frescor e revogação do ingresso; o fluxo de descoberta/instalação continua interface de arquitetura e autoridade federada, sem transferência de domínio.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos integrais; **trecho**: Instalação/descoberta automáticas com ingresso autorizado; **linhas**: 21

- **arquivo**: docs/01-arquitetura/descoberta-ingresso.md; **secao**: Definição de autoativação; **trecho**: **Participação na federação requer autorização** por convite de curta validade, atestação ou credencial pré-provisionada.

- **arquivo**: docs/01-arquitetura/descoberta-ingresso.md; **secao**: Certificado de ingresso e validação; **trecho**: Descoberta e handshake criptográfico autenticam o peer, mas não aprovam sua entrada nem o acesso a dados.

- **arquivo**: docs/07-contratos/identidade-autorizacao.md; **secao**: Chaves, rotação e revogação; **trecho**: Uma credencial com status `unknown` ou `stale` não autoriza ingresso, emissão de grant PTT

### aplicabilidade

**produtos**: - Node

- Teams/Platform como operadores ou autoridades autorizadas

- Personal como cliente que descobre/seleciona serviço

**plataformas**: - servidor Linux Rust como node/peer

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

- web dentro do escopo de clientes

**nota**: O fluxo completo de instalação/ingresso de servidor é específico de Node; clientes não viram full nodes por inferência. Nos demais, descoberta/seleção e autenticação de destino são aplicáveis conforme API.

### decisoes pendentes

- **tema**: Validação durante partição; **alternativas**: - online para todo ingresso/operação crítica; - grant offline assinado limitado; - híbrido por classe; **recomendacao**: Híbrido: não aceitar peer novo nem replicação privada sem autoridade atual; permitir apenas operação local reversível sob grant limitado aprovado.; **consequencia**: Frescor forte diminui disponibilidade; modo offline aumenta janela de estado revogado. Sem limites definidos não há prontidão.

- **tema**: Convite, trust store, validade e revogação; **alternativas**: - emissor preprovisionado; - atestação/delegação federada por escopo; **recomendacao**: Documentar issuer/audience/escopo/epoch e revogação em contrato único, conciliado ao GMP e autoridade de escrita.; **consequencia**: Determina quem pode admitir peer e como impedir ampliação de escopo e liderança obsoleta.

### dependencias

- Autoridade federada/Leslie Lamport: epoch, transferência e partições

- Graydon Hoare: paths, errors e handshake GMP

- Martin Kleppmann: replay/sync/checkpoint

- Bruce Schneier: threat review

- Alan Turing: conciliação de arquitetura

### criterio fechamento documental

Fluxo de estados, proofs, issuer/audience/scope, resultado de status, erro e comportamento de réplica offline coerentes entre descoberta-ingresso, identidade/autorização, GMP, eventos e autoridade de escrita; produto/plataformas confirmam aplicabilidade.

### ids futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite_documental_proposto_pendente_revisao_e_parametros

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-016

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-leslie.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Manifesto/artefato de software tem versão, digest/integridade e assinatura verificáveis; nó, tenant, serviço e ator têm identidade/autorização. Cada classe/cópia tem finalidade, escopo, residência, destino autorizado, modo (`full-authorized`, `regional`, `edge-cache`), retenção e revogação efetivos. Política indefinida bloqueia cópia durável. Snapshot informa origem, escopo, stream, epoch quando aplicável, schema, digest e cursor contíguo.

### Estímulo

Propagar release/configuração/migração assinada; ativar serviço habilitado; requisitar cópia/snapshot/replay por destino permitido e proibido; revogar consentimento/grant ou expirar retenção durante existência/transmissão de cópia; restaurar nó por backup separado e sincronizar após partição.

### Esperado

- Rejeitar artefato com assinatura/integridade inválida ou versão/protocolo incompatível; reportar estado por módulo e dependência, sem inferir health pela presença de release.

- Cada cópia de dado permanece vinculada a tenant, classe, finalidade, política, residência e ACL; autorização é conferida antes do snapshot e de cada replay.

- Reproduzir apenas eventos depois do cursor contíguo validado; deduplicar repetição; cursor expirado exige snapshot/rebootstrap explícito; não pular lacunas.

- Propagar exclusão/tombstone como evento de domínio conforme política, mas não afirmar que tombstone apaga imediatamente todas as cópias.

- Separar backup de réplica e, durante restauração, reaplicar revogações/tombstones mais recentes antes de servir conteúdo sensível; registrar resultado mínimo sem payload privado.

### Invariante

- Software/serviços espelhados não implicam dados irrestritos; réplica não é backup independente.

- Cópia nunca excede autorização, escopo, finalidade, residência ou retenção eficaz.

- Cache de autorização não reabilita grant revogado; replay não ressuscita tombstone/revogação mais recente.

- Lacuna não avança cursor; ACK/progresso de Inbox não afirma automaticamente materialização completa.

- Classe forte não passa a eventual por conveniência do failover.

### Negativo

Negar persistência/replicação/leitura se escopo, finalidade, país/região/residência, retenção, autorização ou prova faltarem/estiverem revogados; não copiar localização privada globalmente por padrão; recusar assinatura inválida, snapshot parcial, replay fora de ordem com lacuna, destino cross-tenant e tentativa de apresentar tombstone como eliminação global confirmada.

### Degradação e recuperação

Se serviço de política/autorização estiver indisponível, bloquear cópia/replay persistente e readiness dependente, mantendo motivo/estado unknown ou pendente. Em partição, permitir somente atividade local explicitamente prevista e subset previamente autorizado; não alterar ACL/revogação global. Ao reconectar, revalidar política atual e revogações, obter snapshot se cursor expirou e replay idempotente. Se política de cópia legada ou estado de revogação não puder ser demonstrado, isolar a cópia e não servir conteúdo sensível.

### limiares

Jurisdições, prazos por classe/cópia, RPO/RTO, tamanho/idade de backlog e targets não escolhidos; nenhum valor foi presumido.

### principal documental

**name**: Leslie Lamport

**status**: confirmed_for_replication_scope_and_consistency

**reason**: The requirement asks for mirroring and data-copy policy, which includes distributed replication scope and convergence.

**interfaces**: - Martin Kleppmann: event synchronization

- Bruce Schneier: authorization/privacy/revocation

- Michael Stonebraker: persistence/projections

- Alan Turing: integration

**operational owner**: not assigned by this confirmation

**conflict**: No principal reassignment recommended; policy and storage interfaces are explicit because the requirement spans domains.

### evidencias

- **file**: docs/00-produto/requisitos.md; **section**: Requisitos integrais; **lines**: 22; **quote**: RF-016 | Espelhamento de software e serviços; políticas de cópias de dados | APROVADO

- **file**: docs/01-arquitetura/nos-regioes.md; **section**: O que é espelhado; **lines**: 28-31; **quote**: Software: releases assinadas, migrações, manifestos, módulos e políticas de configuração.

- **file**: docs/01-arquitetura/replicacao.md; **section**: Classes de informação; Snapshot + replay; Deleções e retenção; Escopo de replicação; Autoridade de escrita e isolamento; **lines**: 6-15,35-43,51-55; **quote**: `full-authorized`, `regional`, `edge-cache`; `tenant_id`, `data_class`, `residency_policy` e `replication_acl` em todos os fluxos.

- **file**: docs/04-seguranca/retencao-revogacao.md; **section**: 1-5; 8-9; **lines**: 3-9,52-99,132-153; **quote**: Dados sem política efetiva não podem ser tratados como autorizados para persistência e replicação.

- **file**: docs/06-planejamento/decisoes.md; **section**: ADR-009, ADR-010, ADR-011; **lines**: 15-17; **quote**: Snapshots e eventos persistentes por escopo; sem SQL global multi-writer livre | Aprovada

### aplicabilidade

**provedores**: - full-node Linux

- edge-node somente para subset autorizado

- operador/control plane responsável por releases e cópias

**produtos**: **Personal**: políticas de cópia por titular/classe e residência ainda precisam de especificação

**Teams**: política por tenant/organização, finalidade e operadores precisa ser aprovada

**Node**: nó regional/edge executa somente software/serviços habilitados e dados expressamente autorizados

**Platform/SDK**: consome manifestos/políticas versionados sem poder sobrepor autorização do servidor

**plataformas**: **macOS universal, iOS, Windows, Linux desktop, Android, Web**: podem receber estado ou manter cache/offline conforme contrato próprio; não são réplicas globais nem backup por terem cache local

**nao aplicavel como provedor**: cliente UI não espelha serviços regionais; aplicabilidade limita-se a cache/sync cliente segundo sua política, a ser detalhada por autores de plataforma.

### decisoes pendentes

- **question**: Quais modos de cópia por tenant/classe, finalidade, país/região e destino?; **alternatives**: - regional/edge-cache minimizado como default e full-authorized somente com autorização explícita, recomendação coerente com ADR-010 mas ainda dependente de escolha de política; - full-authorized multi-região quando aprovado para tenant/finalidade; - sem cópia durável até resolver política; **recommendation**: Nenhuma cópia além do escopo já efetivamente autorizado; bloquear quando política/jurisdição faltarem.; **consequence**: Menos cópias pode reduzir disponibilidade/localidade; cópia ampla eleva riscos de privacidade, residência, retenção e custo.

- **question**: Qual duração/gatilho de retention, revogação, tombstone, backup e expiração de cada cópia?; **alternatives**: - apagar payload e manter recibo mínimo conforme prazo aprovado; - tombstone/bloqueio até expiração física aprovada; - crypto-erase só se ciclo de chaves/cópias/restauração for demonstrado; **recommendation**: Aplicar recomendação documental RR-001 sem converter 'prazo curto' em número; decisão de produto e revisão jurídica continuam obrigatórias.; **consequence**: Sem política efetiva, persistência/replicação/readiness da classe ficam bloqueadas.

### dependencias

- C08 política de retenção/revogação e revisão de jurisdição; sem prazo ou país presumido.

- C10 eventos/snapshot/cursor/ACK/replay; Michael Stonebraker para persistência/materialização; Bruce Momjian/Brendan para backup e recuperação conforme suas atribuições.

- C06 autoridade/quorum para classe forte e fencing; C07 interfaces GMP se aplicáveis.

- Produto/usuário aprova finalidade, consentimento e escopo por produto/tenant.

- Revisão independente e validação futura autorizada de cópia permitida/negada, revogação, partição, tombstone e restore.

### criterio fechamento documental

Critério da matriz identifica cada cópia por classe/tenant/purpose/policy/destino/residência/retenção; contratos de domínio e sync usam fonte única de estados; alternativas e decisão por classe têm owner e estado; teste/SPEC futuros ficam não atribuídos até criação e revisão independente confirma compatibilidade com ADR-009/010/011. Documento não declara implementação validada.

### estado

proposed_pending_policy_and_review

### future spec

NOT_ASSIGNED

### test case

NOT_ASSIGNED

### observed

NOT_EXECUTED

### implementation

NOT_VERIFIED

### related ids

Nenhum item declarado.

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-017

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-gregor.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Usuário/administrador inicia descoberta e define mecanismo/escopo autorizado, ou existe cadastro explícito de instância/credencial.

### Estímulo

Iniciar descoberta local.

### Esperado

Consultar somente alvos/interfaces/instâncias dentro do escopo; retornar candidatos e estado/origem. Descoberta não estabelece sessão, não coleta payload, não concede autorização, não cria rota e não habilita TX.

### Invariante

O escopo não se expande silenciosamente para redes/dispositivos adicionais; candidato não equivale a capability verificada, autorização ou readiness.

### Negativo

Sem gesto, escopo ou autorização, não iniciar busca ativa ampla; indicar descoberta não executada ou negada.

### Degradação e recuperação

Permissão de SO negada, interface ausente ou mecanismo indisponível resulta em estado indisponível/desconhecido, sem fallback para varredura mais ampla. Mudança de permissão/escopo exige nova ação e observação; resultado antigo não prova autorização atual.

### spec

NOT_ASSIGNED

### teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### principal documental

**nome**: Gregor Hohpe

**estado**: confirmado pelo autor para contratos do Gateway/integração

**interfaces**: - Erich Gamma: módulo/interface

- Bruce Schneier/Kim Cameron: permissões e autorização

### evidencias

- **arquivo**: Projects/GodMesh/docs/00-produto/requisitos.md; **linha**: 23; **trecho**: Descoberta autorizada de serviços externos locais, sem varredura indiscriminada; **estado fonte**: ESPECIFICAR

- **arquivo**: Projects/GodMesh/docs/03-integracoes/gateway-universal.md; **linhas**: 24-31; **trecho**: Detecção não invasiva via emparelhamento autorizado, BLE/USB/LAN ou cadastro por conta/credencial.

- **arquivo**: Projects/GodMesh/docs/03-integracoes/gateway-universal.md; **linhas**: 69-76; **trecho**: Não anunciar TX externo só porque um servidor rádio foi descoberto.

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linha**: 28; **trecho**: Consultar capacidades/servidores compatíveis conhecidos, sem varreduras invasivas

### aplicabilidade

**produtos**: Papéis de cliente, Teams, Node e Platform podem variar entre descobrir equipamento, gerir nó e registrar integração; todos preservam autorização e escopo explícito.

**plataformas**: - macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

- servidor Linux/Node quando houver descoberta local correspondente

**racional**: O requisito é comum; mecanismos e permissões nativas variam por plataforma e devem ser especificados com documentação oficial antes de escolha. Não se presume que servidor execute discovery de cliente.

**nao aplicavel**: Nenhuma plataforma excluída; operação específica não aplicável quando não houver interface/dispositivo local, situação a declarar pelo produto sem ativar fallback.

### decisoes pendentes

- **pergunta**: Quem inicia cada modalidade, quais mecanismos locais são permitidos, que escopo se aplica e como se representa permissão negada?; **alternativas**: - cadastro explícito apenas; - descoberta iniciada pelo usuário em alvos escolhidos; - métodos variáveis por plataforma com limites equivalentes; **recomendacao**: Cadastro explícito ou ação iniciada pelo usuário sobre alvos enumerados; sem varredura genérica.; **consequencia**: Alvo fora do escopo só pode ser incluído por cadastro/ação autorizada; frescor e retenção do resultado dependem de política.

### dependencias

- permissões de cada plataforma

- política de identidade/autorização

- perfil de capabilities do Gateway

- política de logging/retention

### criterio fechamento documental

Contrato define iniciador, escopo, métodos permitidos, dados retornados, estados de permissão e falhas por plataforma; explicita que discovery não cria sessão/rota/autorização e não tem fallback expansivo.

### estado

aceite_proposto; dono confirmado; requisito ESPECIFICAR

### referencias existentes

Nenhum item declarado.

### estado conhecimento

**especificado**: mecanismos candidatos e limite contra varredura invasiva documentados

**aprovado**: requisito ainda ESPECIFICAR

**implementado**: não verificado

**validado**: não verificado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-018

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-gregor.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Integração tem perfil/versão autorizado, principal de serviço, credenciais limitadas ao endpoint e contrato explícito de operações.

### Estímulo

SDK inicia operação externa, ou bridge sofre falha/encerramento.

### Esperado

Fronteira autentica chamador e valida versão, operação, escopo e payload antes de aceitar; bridge recebe somente dados/privilégios necessários; solicitação fora do perfil é recusada antes do efeito; desconexão libera recursos/lease local e mantém serviço nativo disponível.

### Invariante

Bridge não obtém autoridade global, identidade GodMesh nem acesso entre tenants; segredo não aparece em envelope/log comum; mídia permanece separada do barramento de eventos.

### Negativo

Chamador/credencial não autorizado, versão incompatível, tenant/endpoint fora de escopo, payload inválido ou autorização revogada não produz efeito externo.

### Degradação e recuperação

Timeout, crash ou exaustão isolam a integração e não interrompem serviço nativo. Resultado externo incerto não recebe retry automático com nova identidade; qualquer retry usa lifecycle C07/C10/C15 e revalida autorização. Não relaxar isolamento para compatibilidade.

### spec

NOT_ASSIGNED

### teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### principal documental

**nome**: Gregor Hohpe

**estado**: confirmado pelo autor para contrato de bridge

**interfaces**: - Erich Gamma: fronteira SDK/módulo

- Graydon Hoare: autorização/GMP

### evidencias

- **arquivo**: Projects/GodMesh/docs/00-produto/requisitos.md; **linha**: 24; **trecho**: SDK de bridges e adaptadores com isolamento de privilégios; **estado fonte**: APROVADO como direção

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 7-10; **trecho**: O SDK de bridge converte serviços externos em capacidades GodMesh, mas não delega a eles identidade, autorização global ou autoridade de canal.

- **arquivo**: Projects/GodMesh/docs/03-integracoes/bridges-voz.md; **linhas**: 39; **trecho**: Protobuf/gRPC ou interface IPC autenticada são candidatos para separar bridge de terceiros do processo principal.

- **arquivo**: Projects/GodMesh/docs/03-integracoes/gateway-universal.md; **linhas**: 75-76; **trecho**: Adaptadores com privilégios mínimos, IPC autenticado, segredo isolado por conexão, auditoria, rate limiting, fila de comandos com TTL, policy engine e prevenção de ciclos.

### aplicabilidade

**produtos**: SDK/Bridge Host aplica-se aos processos GodMesh que hospedam conectores e às integrações expostas por Personal, Teams, Node e Platform; clientes não herdam privilégios do host.

**plataformas**: - servidor Linux

- macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

**racional**: Contrato de privilégio é comum; implantação/proc/IPC variam. Servidor/Node pode hospedar bridge; clientes usam capacidades mediadas quando disponíveis. Nenhuma plataforma é excluída sem inventário de hospedagem.

**nao aplicavel**: Nenhuma plataforma declarada não aplicável; ausência de bridge hospedada em certo cliente limita a implantação, não o requisito de isolamento do host.

### decisoes pendentes

- **pergunta**: Qual fronteira, protocolo, versão, identidade de serviço, escopo de segredo, limites e regra de contenção serão adotados por integração e plataforma?; **alternativas**: - processo separado com IPC autenticada; - IPC autenticada dentro de arquitetura de processos existente; - Protobuf/gRPC ou outro contrato versionado após compatibilidade; **recomendacao**: SDK versionado e fronteira autenticada com credenciais por integração; processo separável quando compatibilidade e ameaça exigirem.; **consequencia**: Sem fronteira e evidência de contenção revisadas, conector permanece inelegível; ADRs não selecionam IPC ou biblioteca.

### dependencias

- ADR-022 e ADR-032

- C07 envelope/versões/erros

- C09 identidade e autorização

- C10/C15 lifecycle de comandos e efeitos incertos

- revisão de segurança e plataformas

### criterio fechamento documental

Contrato define fronteira autenticada, least privilege por operação, credenciais, versões, validação de entrada, isolamento de falhas, redaction e recuperação; escolhe tecnologia apenas após compatibilidade/revisão. Validação de contenção requer etapa futura autorizada.

### estado

aceite_proposto; dono confirmado; tecnologia específica e implementação pendentes

### referencias existentes

- ADR-022

- ADR-032

### estado conhecimento

**especificado**: direção e controles conceituais registrados

**aprovado**: opcionalidade/isolamento como direção; IPC/Protobuf/gRPC são candidatos

**implementado**: não verificado

**validado**: não verificado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-019

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-graydon.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Cliente usa API major, schema/profile e credencial conhecidos; servidor tem rota, schema, autorização e compatibilidade publicados. Mutação tem tenant/principal autenticados e chave idempotente. API major, versão OpenAPI e GMP são distintas.

### Estímulo

Chamar rota válida e inválida; autenticar com scope ausente; usar versão compatível N/N-1 e incompatível; repetir mutação com mesma chave/fingerprint após timeout; reutilizar chave com conteúdo diferente; expirar cursor/grant ou indisponibilizar dependência.

### Esperado

Artefato OpenAPI aprovado corresponde a rotas, schemas, media types, autorização e erros e passa validador compatível com sua versão. SDK preserva request/status/erro sem convertê-los em prova de entrega. 202 significa aceitação, não conclusão. Versão incompatível falha sem downgrade/efeito. Repetição idêntica retorna estado/operação canônica; fingerprint divergente conflita sem segundo efeito. Só anunciar N/N-1 com compatibilidade aprovada.

### Invariante

Principal/tenant derivam do contexto autenticado; IDs de tenant fornecidos pelo cliente não elevam acesso. API major, OAS, schema, GMP e transporte não colapsam. Erro estável não depende de texto localizado e não revela segredo. ACKs e lifecycle C10 permanecem distintos.

### Negativo

Cross-tenant, scope/token inválido, media type/schema errado, feature crítica desconhecida, versão incompatível, cursor de outro principal/filtro, chave conflitante, segredo em erro ou tratar 202 como entrega não produzem efeito indevido nem exposição. Falha pré-commit não cria recibo; commit sem resposta mantém resultado consultável.

### Degradação e recuperação

Timeout/UNAVAILABLE/429/RESOURCE_EXHAUSTED e cursor expirado só informam garantias documentadas; retry condicional não é promessa. Mutação incerta consulta a mesma key/operation; outcome_unknown não vira sucesso/falha inventados. Rebootstrap após cursor expirado depende de ACL. Auth stale ou dependência indisponível falha sem reduzir autorização ou mudar semântica. Prazos/limites permanecem abertos.

### principal documental

**nome**: Graydon Hoare

**estado**: confirmado como adequado para API pública, SDK e versionamento

### evidencias

- **fonte**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linha**: 105; **trecho**: RF-019 | 00-produto/requisitos.md:25 | ESPECIFICAR | 07-contratos/api-inicial.md | Graydon Hoare — proposto por especialidade, confirmação por requisito pendente

- **fonte**: docs/00-produto/requisitos.md; **linha**: 25; **trecho**: RF-019 — API pública, SDK de clientes e gerência de versões — ESPECIFICAR

- **fonte**: docs/07-contratos/api-inicial.md; **secoes**: - Convenções; - Endpoints; - Stream/ACK; **trecho**: tenant derivado de autenticação, API versionada, idempotência e ACKs limitados

- **fonte**: docs/07-contratos/gmp-api-v1.md; **secoes**: - 1-8; - 10-12; **trecho**: schemas, versionamento, erros, status e conformidade propostos

- **fonte**: docs/07-contratos/eventos-sincronizacao.md; **secoes**: - 4.1; - 4.1.3; **trecho**: IDs, cursor, idempotência, ACK e lifecycle

- **fonte**: docs/06-planejamento/decisoes.md; **adrs**: - ADR-029; **trecho**: versionamento de APIs e transportes

- **fonte**: entregas C07 anteriores; **id**: GMD-A03-002; **trecho**: proposta API; OpenAPI formal e referências ainda por integrar

- **fonte**: contribuicoes/graydon.json; **id**: PI-graydon-001; **trecho**: relação confirmada com OpenAPI/gerador/API tooling

### aplicabilidade

**escopo**: API compartilhada entre Personal/Teams/Node e clientes/plataformas; servidor Linux e clientes macOS universal/iOS primeiro, demais plataformas preservadas.

**na**: False

**justificativa na**: Não aplicável: cliente sem SDK gerado ainda segue o contrato HTTP/GMP comum.

### decisoes pendentes

- **tema**: OpenAPI 3.2.1 proposta versus 3.1.x e gerador/validator; **alternativas**: - Manter 3.2.1 se ferramentas/consumidores forem confirmados; - Selecionar versão formalmente suportada por todos; **recomendacao**: Não publicar SDK até validar ferramenta e consumidores; **consequencia**: Afeta codegen e interoperabilidade, não o significado funcional.

- **tema**: SDK, suporte/depreciação, N/N-1, media types, limites e rate limits; **alternativas**: - Publicar matriz versionada por consumidor; - Manter compatibilidade como bloqueio de prontidão; **recomendacao**: Preencher após decisão de plataforma e evidência de ferramenta; **consequencia**: Define janela de suporte e disponibilidade; sem números inventados.

- **tema**: Mecanismo de token/scopes e retenção de recibos; **alternativas**: - Ratificar C09/C08/C10; - Manter mutações dependentes bloqueadas; **recomendacao**: Usar decisões canônicas de identidade, eventos e retenção; **consequencia**: Controla autorização, dedupe e consulta pós-timeout.

### dependencias

- C09 principal/scopes/revogação

- C10 cursor/idempotência/ACK/retry

- C08 retenção de recibos

- C06 autoridade

- Alan integração da matriz/OpenAPI

- responsáveis de plataformas e SDK

### criterio fechamento documental

Artefato formal versionado; rotas, schemas, media types, erros e autorização reconciliados; matriz cliente/compatibilidade e crosswalk erro/status publicados; revisão e decisões registradas. API/SDK/testes não são validados por esta proposta.

### estado

**planejamento aceite**: proposto

**SPEC**: NOT_ASSIGNED

**teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

**requisito origem**: ESPECIFICAR

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-020

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-margaret.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Produto/perfil (Personal, tenant Teams ou Node) foi decidido; identidades, permissões, capacidades, conectividade e políticas de dados estão conhecidos ou explicitamente desconhecidos. Contratos por perfil ainda precisam ser aprovados.

### Estímulo

Percorrer jornadas acordadas para cada perfil: iniciar uso nativo, acionar capacidade incluída, negar/revogar permissão, perder bridge externa/rede/dependência e restaurar rede; testar sessão nova e recuperação.

### Esperado

Cada perfil habilita somente capacidades suportadas e autorizadas; exibe estado/falha por serviço sem afirmar sucesso não evidenciado; respeita escopo de dados; falha de bridge opcional não interrompe serviços nativos internos; após reconexão sincroniza pelo contrato sem marcar pendências como entregues. Cada jornada referencia RF/RNF de domínio e critérios correspondentes.

### Invariante

Escopo completo não é reduzido por fases ou primeira plataforma; autorização, consentimento e política governam dados e capacidades; nenhum status falso de entrega, suporte ou disponibilidade; serviço externo opcional não se torna requisito do núcleo. População, quota, latência, disponibilidade, retenção e custo ficam abertos até haver decisão e cenário com fonte.

### Negativo

Capacidade não autorizada, não decidida ou não suportada não é habilitada silenciosamente; acesso cross-tenant/fora de escopo é negado; bridge indisponível não é rotulada como indisponibilidade do núcleo.

### Degradação e recuperação

Offline/dependência indisponível: apresentar estado local e pendências sem declarar entrega; limitar dados às políticas autorizadas. Recuperação: reconciliar segundo contrato idempotente, aplicar revogações e não antecipar sucesso. Fila, conflitos e limites aguardam contrato; portanto não há limiar fechado.

### principal documental

**nome**: Margaret Hamilton

**estado**: confirmado para aceitação de produto/cenários; owners de domínio confirmam contratos próprios

### evidencias

- **fonte**: GodMesh/docs/00-produto/requisitos.md; **secao**: Requisitos integrais; **linhas**: 26; **trecho**: Sistema completo para Personal, Teams e Node — APROVADO

- **fonte**: GodMesh/docs/00-produto/visao.md; **secao**: Produtos sobre o mesmo núcleo / Plataformas aprovadas / Capacidades integrais; **linhas**: 10-32; **trecho**: Personal, Teams, Node, Platform & SDK; conjunto de capacidades e plataformas planejadas

- **fonte**: GodMesh/docs/06-planejamento/decisoes.md; **secao**: Índice de ADRs; **linhas**: 29; **trecho**: ADR-023: GodMesh é sistema integral, não reduzido a versão mínima — Aprovada

- **fonte**: GodMesh/docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual, RF-020; **linhas**: 106; **trecho**: Aceite atual apenas “Observar: Sistema completo”; critérios e cenários ainda abertos

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

**platform sdk**: Visão inclui Platform & SDK, mas RF-020 não o nomeia; tratar como interface que expõe capacidades conforme contrato futuro, sem ampliar requisito por inferência.

**plataformas**: - Servidor Linux em contêiner

- macOS universal arm64/x86_64 com SwiftUI

- iOS com SwiftUI

- Windows desktop

- Linux desktop

- Android

- Web

**regra**: Todas permanecem no escopo integral. Cada jornada aplicável tem critérios por plataforma; quando uma capacidade não existir, declarar limite real e fallback, não marcar plataforma como fora do escopo.

### decisoes pendentes

- **questao**: Fronteira funcional e comercial de cada oferta/perfil; **alternativas**: - Catálogo de capacidades comum com permissões por perfil (recomendação); - Pacotes rígidos por perfil; - Capacidades configuráveis por tenant; **recomendacao**: Catálogo comum governado por capacidade/permissão, com diferenças de operador/empacotamento explícitas.; **consequencia**: Exige matriz de capacidades e política comercial; não presume que produto/tenant receba todo serviço.

- **questao**: Operação, licença, região, suporte, quotas e compromisso de disponibilidade por oferta; **alternativas**: - Hosted; - Self-hosted; - Híbrido por oferta (comparar recomendado); - Adiar escolha até estimativa de cenário/custo; **recomendacao**: Comparar híbrido com hosted-only e self-hosted-only por cenários e custos; não definir preço ou SLO nesta proposta.; **consequencia**: Define responsável por operação/dados/egress, suporte e promessas mensuráveis.

### dependencias

- Decisão de produto do usuário: ofertas, mercados, operador, licença, região e apetite de custo

- Grady/Brendan: método e premissas verificáveis para cenários de população/carga e custo; sem medições alegadas

- Donos de domínios: contratos/aceites para RF/RNF relacionados e consistência de dados, autorização, retenção e sync

- Revisão da matriz e dos gates antes da futura etapa de SPECs

### criterio fechamento documental

Fronteiras e capacidades por oferta/perfil aprovadas; jornadas e cenários de população/carga explicitam premissas e método; dependências, responsabilidades operacionais, política de dados, limites e métricas têm decisão/fonte; referências harmonizadas entre visão, requisitos, roadmap, pendências e matriz; revisão independente. Futura SPEC/teste permanece NOT_ASSIGNED até autorização; observado permanece NOT_EXECUTED.

### estado

proposto_aberto; direção de produto APROVADA não equivale à implementação.

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-021

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Principal/device autenticados; tenant, canal, rede, finalidade, audiência e classe/precisão local definidos; estado de autoridade e grants conhecidos; desconhecido não equivale a autorização.

### Estímulo

Solicitar habilitação, TX RF ou leitura de localização; repetir sem membership, com tenant divergente, grant vencido/revogado ou autorização/licença de operador ausente.

### Esperado

Liberar somente operação, rede, audiência, finalidade e precisão explicitamente autorizadas; negar TX sem direito e consentimento aplicáveis; negar localização fora do escopo; devolver estado sanitizado e recibo de auditoria sem payload/credencial/coordenada.

### Invariante

Autenticação, posse de senha, sala pública ou membership não concede privilégio RF nem localização; não atravessar tenant, domínio de rádio, finalidade ou audiência; canal não concede posição precisa SOS.

### Negativo

Emissor/ACL/controlador/licença indisponível, autoridade unknown, replay ou revogação negam operação sensível; sem fallback para TX, posição precisa ou bridge não autorizada.

### Degradação e recuperação

Apresentar pendência quando confirmação faltar; reconexão revalida grant/revogação antes de retomar. O modo offline exige decisão específica por classe.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos do GodMesh; **linhas**: 27; **trecho**: Habilitação de funções sem acesso não autorizado a RF ou localização; **estado origem**: APROVADO como direção

- **arquivo**: docs/04-seguranca/uso-das-redes.md; **secao**: Requisitos de bridge; **linhas**: 12; **trecho**: Verificar licenças e políticas de rede; se faltarem, bloquear TX.

- **arquivo**: docs/04-seguranca/uso-das-redes.md; **secao**: Requisitos de bridge; **linhas**: 16; **trecho**: Sem autorização de transmissão RF, operar apenas em laboratório IP, recepção autorizada ou demonstração sem RF.

- **arquivo**: docs/07-contratos/identidade-autorizacao.md; **secao**: Decisão de autorização; **linhas**: 145-148; **trecho**: Verificar consentimento e finalidade específicos, incluindo janela temporal e destinatário para localização/Guardian/SOS.

### aplicabilidade

**produtos**: Personal/Teams/Node/Platform conforme função, tenant e capacidade

**plataformas**: Servidor Linux; macOS universal, iOS, Windows, Linux desktop, Android e web

**nao aplicavel**: Subcaso RF físico só para instalação sem rádio/gateway autorizado; justificar hardware/integracao ausente. Autorização lógica e localização continuam aplicáveis.

### decisoes pendentes

- **tema**: jurisdição/modalidade RF e autoridade de prova; **alternativas**: - negação sem prova atual; - grant offline limitado a definir; **recomendacao**: Negar RF/localização precisa quando autoridade é unknown; **consequencia**: Maior proteção e menor disponibilidade offline; grant aumenta janela residual de abuso

- **tema**: frescor por classe; **alternativas**: - online-only para operação sensível; - grant assinado com validade aprovada; **recomendacao**: Decidir separadamente por operação e sensibilidade; **consequencia**: Valores dependem do usuário e de avaliação regulatória por país

### dependencias

- Radia Perlman: redes/voz

- Lawrence Rosen: legislação/licença por jurisdição

- Paul Ramsey: dados geoespaciais

- Kim Cameron: identidade/autorização

### criterio fechamento documental

Matriz por operação documenta principal, scope, tenant, finalidade, precisão, rede, prova e comportamento offline; inclui normal, negação, revogação, recuperação e justificativas N/A; fontes normativas são conciliadas.

### estado

proposta comportamental; decisões abertas; sem validação de implementação

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-022

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Operação identificada (mensagem, PTT, transcrição, SOS/localização ou bridge), perfil de proteção aprovado, audiência e chaves atuais definidas.

### Estímulo

Enviar conteúdo por rota nativa, por bridge, com chave/perfil revogado, incompatível ou destinatário não autorizado; solicitar transcrição ou fallback.

### Esperado

Fluxo declarado E2EE só permite leitura por endpoints autorizados; declarar explicitamente serviço terminador, plaintext e metadados visíveis quando aplicável; bridge não herda alegação E2EE nativa; incompatibilidade não amplia audiência.

### Invariante

E2EE, TLS e cifragem em repouso são propriedades distintas; sem downgrade silencioso, chave desconhecida ou uso cruzado entre propósito/tenant.

### Negativo

Perfil/chave/receptor ausente ou revogado interrompe/adia envio; bridge sem autorização nega encaminhamento; transcrição incompatível com E2EE fica desativada salvo mudança explícita e autorizada de modo.

### Degradação e recuperação

Oferecer alternativa apenas com escolha informada e proteção declarada; após rotação, revalidar perfil/chaves antes de retomar.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos do GodMesh; **linhas**: 28; **trecho**: Criptografia E2EE onde suportada, sem falsa promessa em bridges; **estado origem**: ESPECIFICAR

- **arquivo**: docs/04-seguranca/seguranca.md; **secao**: Fronteiras do GodMesh Native e das bridges; **linhas**: 45; **trecho**: Uma bridge de transcodificação normalmente termina a proteção de mídia; apresentar aviso explícito aos participantes e não alegar E2EE universal.

- **arquivo**: docs/01-arquitetura/midia-nativa.md; **secao**: Criptografia; **linhas**: 45-46; **trecho**: Se uma bridge precisa transcodificar, normalmente acessará áudio legível; não declarar E2EE preservada entre redes externas.

### aplicabilidade

**produtos**: Todos os produtos com operações de conteúdo sensível

**plataformas**: Servidor Rust/Linux; macOS universal e iOS primeiro; Windows/Linux desktop, Android e web permanecem no escopo

**nao aplicavel**: Subcaso bridge quando nenhuma bridge estiver configurada; princípios anti-downgrade e perfil por operação continuam aplicáveis.

### decisoes pendentes

- **tema**: E2EE por operação; **alternativas**: - E2EE entre endpoints; - cifra até serviço autorizado; - perfil distinto conforme operação; **recomendacao**: Matriz versionada por operação; não alegar E2EE quando serviço recebe plaintext; **consequencia**: E2EE limita processamento/recuperação central; terminação facilita processamento e eleva confiança no serviço

- **tema**: suite/gestão/recuperação de chaves; **alternativas**: - decidir após compatibilidade e revisão; - manter fluxo bloqueado sem perfil; **recomendacao**: Não selecionar suíte nesta etapa; **consequencia**: Sem decisão, operação não pode ser declarada pronta

### dependencias

- Kim Cameron: chaves/identidade

- Radia Perlman: mídia

- Gregor Hohpe: bridges/UDNL

- Graydon Hoare: API

- usuário: modelo de proteção por operação

### criterio fechamento documental

Matriz de atores que leem conteúdo/metadados e falhas está alinhada entre API, mídia, bridge, ASR e SOS; alegações E2EE revisadas; decisão de política e revisão independente registradas.

### estado

proposta comportamental; permanece ESPECIFICAR; não é SPEC ou prova criptográfica

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-023

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-fowler.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Duas entidades GodMesh nativas e uma bridge opcional; serviço externo não é pré-requisito.

### Estímulo

Desligar/desconectar bridge ou negar grant durante comunicação nativa.

### Esperado

Serviço nativo segue funcional; somente rota dependente da bridge degrada e é identificada; núcleo não sofre alteração de identidade/ACL.

### Invariante

Nenhum estado ou credencial externa obrigatório no caminho nativo; bridge não modifica autoridade/ACL.

### Negativo e degradação/recuperação — bloco combinado

Descoberta sem grant não transmite; falha ou loop da bridge não interrompe serviço interno; reconexão só reabilita rota após revalidar grant/capacidade e não duplica eventos/comandos.

### limiares abertos

Definir saúde interna, observabilidade, carga e bridges cobertas sem inventar limites.

### principal documental

Martin Fowler — proposto por especialidade na matriz; confirmação por requisito pendente

### evidencias

- **fonte**: 00-produto/requisitos.md:29; **trecho**: GodMesh nativo independente de quaisquer servidores externos; bridges devem ser desconectáveis sem impactar os canais e serviços internos.; **hash**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664

- **fonte**: 01-arquitetura/estrategia-tecnologica.md §§Tese, Zona A, Zona C, Garantias (linhas 5-11,44-46,66-72,110-116); **trecho**: Independência operacional obrigatória; bridges opcionais e isoladas; desconexão sem impacto nos serviços internos.; **hash**: f79e0be0294f612c472d478358d123756fa6b83e124e08ce60d72750bb332c7d

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

**plataformas**: - servidor Linux

- macOS universal

- iOS

- Windows

- Linux desktop

- Android

**justificativa**: Propriedade arquitetural compartilhada; UI por SO não altera o invariante. Validar cada peer se capacidades diferirem.

### decisoes pendentes

- **questao**: Definir saúde interna e conjunto de integrações exercitadas.; **alternativas**: - Sinalizar somente disponibilidade do serviço; - Definir indicadores por serviço (recomendado); **recomendacao**: Indicadores por serviço, coordenados com dono operacional.; **consequencia**: Permite verificar isolamento, mas exige contrato de observabilidade e cenários específicos.

### dependencias

- contratos de serviços internos

- GMP

- UDNL/Gateway

- operação/observabilidade

### criterio fechamento documental

Critérios associados a donos de serviço/GMP/operação; definição de saúde/degradação e recuperação; confirmação do principal; revisão independente. SPEC/testes futuros seguem NOT_ASSIGNED e NOT_EXECUTED.

### estado

proposta documental; direção origem APROVADO; atribuição não confirmada; não SPEC, não implementado, não validado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-024

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-fowler.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Martin Fowler. Subescopos e deltas vinculados: [Conciliação fowler](#conciliacao-fowler). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Peers anunciam versões/features requeridas, opcionais e suportadas; perfil e suporte real conhecidos.

### Estímulo

Negociar par compatível; tentar major incompatível, feature crítica ausente, enum crítico desconhecido, transporte primário indisponível ou versão inferior.

### Esperado

Selecionar interseção permitida; incompatibilidade crítica falha antes de efeito; ignorar opcional desconhecida somente sem alterar semântica; fallback autorizado e anunciado com estado de degradação.

### Invariante

Sem suporte declarado por desejo; sem downgrade que enfraqueça política; transporte não altera autorização/identidade/semântica GMP.

### Negativo e degradação/recuperação — bloco combinado

Rejeitar downgrade/versão incompatível sem efeito parcial; usar só fallback negociado e seguro; sem rota segura, erro explícito; rollout malsucedido pode pausar e recuperar perfil compatível, se reversível.

### limiares abertos

Versões, janela N/N-1, perfis, feature crítica, política downgrade/fallback e canary/rollback.

### principal documental

Martin Fowler — proposto na matriz; adequado à evolução arquitetural. Co-responsabilidade documental com Graydon/GMP a confirmar; não transferida.

### evidencias

- **fonte**: 00-produto/requisitos.md:30; **trecho**: Transportes e versões evolutivos com negociação de capacidades, fallback seguro, conformidade e atualização gradativa.; **hash**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664

- **fonte**: 01-arquitetura/estrategia-tecnologica.md §§Zona B, GMP—evolução, garantias, linhas 50-64,76-86,102-116; **trecho**: Suporte real, proteção contra downgrade, negociação e compatibilidade.; **hash**: f79e0be0294f612c472d478358d123756fa6b83e124e08ce60d72750bb332c7d

- **fonte**: 07-contratos/gmp-api-v1.md §§3,4.2,4.5,6,10,11; **trecho**: Negociação de versão/features, limites, erros e conformidade; propostas técnicas sem validação.; **hash**: 17a225a4ee2bdf8679037d1f6bb6dff55fcf24bbbe2ed8501d3e02f007d159aa

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal

- iOS

- Windows

- Linux desktop

- Android

**justificativa**: Contrato de peer comum; exigir matriz de pares se suporte variar por plataforma.

### decisoes pendentes

- **questao**: Fechar perfis e suporte N/N-1, semântica de feature crítica, downgrade, fallback e rollback.; **alternativas**: - Política versionada por perfil com falha segura (recomendado); - Política única fixa para todos os peers; **recomendacao**: Política versionada por perfil e falha segura em semântica crítica.; **consequencia**: Interoperabilidade não é declarada antes de perfis/vetores e evidência multi-plataforma.

### dependencias

- GMP/Graydon

- arquitetura/Fowler

- transporte

- operação/canary

- interfaces por plataforma

### criterio fechamento documental

Semântica única ou cross-reference sem divergência; tabela de perfis/compatibilidade e decisões necessárias aprovadas; revisão independente. Validação futura permanece NOT_EXECUTED.

### estado

proposta documental; direção origem APROVADO; detalhe de negociação pendente; não SPEC, não implementado, não validado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-025

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-daniel.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Canal e sessão `talk_session_id` existem; observador tem ACL para timeline; política, consentimento, processador/capacidade e retenções de metadados, texto e áudio estão resolvidos. Se política ou consentimento não autorizar, ASR inicia desativado.

### Estímulo

Transmissão nativa ou bridge autorizada começa, continua, termina ou é interrompida; ASR produz parcial/final/correção/indisponibilidade; entrega pode repetir ou reordenar eventos.

### Esperado

Timeline contém atividade mesmo sem ASR. Eventos permitidos ligam-se ao mesmo `talk_session_id`; parcial é transitório, final automático é identificado e correção cria revisão relacionada. Proveniência distingue GMC verificado, ID externo escopado/reportado e identidade desconhecida. Duplicatas não criam nova entrada.

### Invariante

Negar ou desligar ASR não interrompe PTT nem remove atividade observável. Não inventar texto ou identidade. Diarização não prova identidade. ASR não substitui evento de voz nem SOS. Áudio claro não sai do processador autorizado.

### Negativo

Política/consentimento nega captura: não capturar e exibir motivo apropriado. ACL de leitura/correção nega a operação sem expor conteúdo ou alterar revisão vigente.

### Degradação e recuperação

Falha de worker/processador/link publica motivo indisponível, sem payload em log ou texto inventado; parciais cedem sob backpressure. Queda de link durante fala marca `interrupted` segundo timeout ainda pendente. Reconexão deduplica por sessão; retentativa de áudio só ocorre se retenção temporária autorizada.

### limiares

Timeout, qualidade, idiomas cobertos, buffers e SLOs não definidos; requer decisão/avaliação, nenhum número presumido.

### principal documental

**nome**: Daniel Povey

**estado**: confirmado nesta contribuição

**base**: Especialidade ASR/transcrição; matriz integrada linha 111 propõe Daniel.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **linha**: 32; **trecho**: Cada canal de voz oferece timeline de chat com transcrição autorizada, GMC ou ID externo preservado, marcadores parcial/final/correção e retenção controlada; **estado**: origem aprovada como direção

- **arquivo**: docs/01-arquitetura/transcricao-multimodal.md; **linhas**: 26-30; **trechos**: - Toda fala PTT identificável deve produzir uma entrada própria na timeline do canal, independentemente da disponibilidade de transcrição.; - A transcrição é um enriquecimento opcional vinculado pelo mesmo `talk_session_id`.

- **arquivo**: docs/01-arquitetura/transcricao-multimodal.md; **linhas**: 42-57, 61-69; **observacao**: Estados de indisponibilidade, identidade, políticas, E2EE, worker ASR e retenção independente de áudio/texto.

- **arquivo**: docs/07-contratos/transcricao-sos.md; **linhas**: 7-12, 37-40; **observacao**: Eventos timeline de voz, parcial, final, corrigido e indisponível; contrato ainda proposta.

- **arquivo**: docs/06-planejamento/decisoes.md; **linhas**: 47, 51; **observacao**: ADR-038 e ADR-040 aprovadas como direções; implementação/legislação ou implementação pendentes.

### aplicabilidade

**produtos**: **Personal**: aplicável quando usa canal/transcrição autorizada

**Teams**: aplicável quando usa canal/transcrição autorizada

**Node**: aplicável ao serviço de canal/worker habilitado

**Platform/SDK**: aplicável a produtores/consumidores do contrato

**plataformas**: **servidor Linux**: worker/timeline onde provisionados

**macOS universal e iOS SwiftUI**: cliente quando exibe ou opera canal

**Windows/Linux desktop e Android**: cliente quando exibe ou opera canal

**web**: cliente quando previsto no produto integral

**nao aplicavel**: Nenhum produto ou plataforma foi excluído pelas fontes; separar papel de worker e de interface, e validar capacidades específicas na conciliação.

### decisoes pendentes

- **tema**: Semântica de confiança; **alternativas**: - Estados categóricos até existir calibração; - Pontuação normalizada após validação por idioma/modelo/ruído; **recomendacao**: Estado categórico inicialmente; **consequencia**: Não haverá comparação de pontuação entre modelos sem evidência; usuário decide política visível.

- **tema**: Timeout de interrupção; **alternativas**: - Timeout contratual configurável; - Encerrar apenas ao receber evento final; **recomendacao**: Definir após análise de rede e produto, sem número inventado; **consequencia**: Afeta estado `interrupted` e certeza da duração.

- **tema**: Correção e acesso; **alternativas**: - ACL e revisão auditável por canal; - Correção desabilitada até política definida; **recomendacao**: Preservar origem/revisão e exigir ACL; **consequencia**: Graydon define envelope e Lea interface; usuário aprova visibilidade do produto.

- **tema**: Idiomas/processadores; **alternativas**: - Catálogo local; - Servidor próprio; - Remoto opt-in por canal; **recomendacao**: Nenhum fallback cruzando fronteira de confiança sem autorização; **consequencia**: Compatibilidade, licença, preço e cobertura permanecem dependências.

### dependencias

- Radia Perlman: sessão, prioridade e backpressure

- Graydon Hoare: GMP, deduplicação, revisão

- Lea Verou: estados e consentimento na interface

- Bruce Schneier: autorização/privacidade

- Michael Stonebraker: retenção por classe

- Decisões do usuário sobre idiomas, processadores, custos e prazos

### criterio fechamento documental

Revisão registra política e estados, evento e proveniência, estado inicial, estímulo, resultado, invariantes, caso negativo e recuperação, permissões, aplicabilidade por produto/plataforma e decisões aprovadas. Sem limiares não medidos. Não é SPEC nem validação.

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite documental proposto; decisão e revisão independente pendentes; produto não validado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-026

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Origem autenticada/autorizada; test_mode explícito; política de audiência/escopo válida; localização opcional e, se presente, fonte, precisão, frescor e grant preservados. Sem política/rota/destinatário, não presumir elegibilidade.

### Estímulo

Acionamento explícito por usuário/dispositivo habilitado em modo real e teste, online/offline; ASR ausente/incerto deve ser irrelevante ao disparo.

### Esperado

Um incidente idempotente e auditável é registrado; ACK local só após persistência comprovada; cada tentativa/destino aparece separadamente; claim só por ação humana autenticada; localização precisa só por grant específico; test_mode não alcança efeito/destinatário real.

### Invariante

ASR, ACK local, gateway, rede, push, leitura ou satélite não provam claim nem despacho; overdue Guardian não cria SOS; posição precisa não vaza; cancelamento/resolução não apaga história.

### Negativo

Negar origem sem prova, replay/duplicata com conflito, localização sem grant, bridge não autorizada e tentativa sem rota sem criar entrega/claim fictícios.

### Degradação e recuperação

Offline mantém estado pendente e ausência de confirmação; reconnect reconcilia sem duplicar efeitos; falha regional, bateria baixa ou transporte inelegível não encerra incidente nem declara entrega.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: True

**estado**: adequado_ao_escopo_sos

**principal recomendado**: Nancy Leveson

**interfaces recomendadas**: - Margaret Hamilton para política de produto

- Graydon Hoare para contrato SOS

- Martin Kleppmann para sync

- Bruce Schneier/Kim Cameron para autorização e privacidade

- Brendan Gregg para observabilidade

**justificativa**: O requisito trata explicitamente segurança operacional, confirmação humana, escalonamento, modo de teste e auditoria SOS.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linhas**: 33; **trecho**: SOS multimodal independente de ASR ... ACK de rede/dispositivo e humano separados, modo teste e auditoria; **estado origem**: APROVADO como direção

- **arquivo**: docs/02-dominio/servico-sos.md; **secao**: Finalidade; estados; escalonamento; teste/abuso; **linhas**: 3, 7-16, 24-35, 37-57, 63-78, 105-114; **trecho**: Um atendimento só deve ser marcado como `claimed` por uma identidade humana autorizada.; **estado**: especificação; expressamente não operacional para emergência real

- **arquivo**: docs/07-contratos/transcricao-sos.md; **secao**: GMP — Eventos SOS; endpoints; **linhas**: 1-3, 42-64; **trecho**: Não inferir sucesso de despacho por respostas HTTP 200/202; **estado**: proposta não congelada

- **arquivo**: docs/06-planejamento/decisoes.md; **secao**: ADR-039; **linhas**: 49; **trecho**: Aprovada como direção; serviço ainda não validado.

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 112; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

### aplicabilidade

**produtos**: - Personal

- Teams

- Node como serviço de roteamento/armazenamento

- Platform/SDK sob autorização

**plataformas**: - Servidor Rust/Linux

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

- Web

- dispositivos/gateways autorizados

**regra**: Cada superfície só promete capacidades realmente suportadas; servidor não se declara agente humano e cliente declara limites de background/transporte.

**nao aplicavel**: Nenhum produto/plataforma excluído de forma global. Uma fonte sem localização ou ACK pode deixar essa evidência ausente, não dispensa honestidade do estado.

### decisoes pendentes

- **decisao**: Definir destinatários/cobertura, claim, prazos, retries, esgotamento e fallback.; **alternativas**: - Política configurável por organização/canal; - Melhor esforço sem prazo; - Compromisso global; **recomendacao**: Política explícita por organização/canal; sem número padrão; exibir sem responsável/sem compromisso quando assim configurado.; **consequencia**: Sem decisão, não existe SLA nem comportamento de escalonamento completo.

- **decisao**: Definir resolução de claims concorrentes e relógio/reconciliação offline.; **alternativas**: - Um humano assume exclusivamente; - Equipe compartilha responsabilidade; **recomendacao**: Registrar ator/escopo e deixar desconhecido até contrato definir precedência/tempo.; **consequencia**: Afeta estado e mensagem ao solicitante; exige contrato sync e política de plantão.

- **decisao**: Definir integração pública e retenção/jurisdição.; **alternativas**: - Sem integração pública; - Integração somente por convênio/validação; **recomendacao**: Nenhum envio automático implícito; só avaliar mediante acordo, revisão e validação.; **consequencia**: Sem autorização legal/operacional, não anunciar despacho público.

### dependencias

- Destinatários e responsáveis humanos reais

- Política e procedimento por organização/canal

- Contrato versionado de eventos SOS

- Semântica temporal e sync

- Autorização, privacidade, retenção e jurisdição

- Revisão independente e validação futura autorizada

### criterio fechamento documental

Contrato e domínio definem acionamento, estados/transições, destinatários, ACKs, claim, fallback/ausência de claim, offline, modo de teste, localização e auditoria; alternativas e prazos são decididos ou explicitamente não prometidos; cenários cobrem positivo/negativo/degradação/recuperação; GMD-A28-001 só fecha após validação do critério integrado, não pela proposta.

### relacionamentos comprovados

- GMD-A28-001

- PI-nancy-001

- ADR-039

- ADR-041

- RF-011

- RNF-009

- RNF-012

### estado

principal confirmado nesta contribuição; decisões/integracao/revisão pendentes; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-027

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-margaret.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Usuário autenticado na jornada; contexto ausente começa unknown; consentimentos/permissões de sensores, localização, audiência e atividade são explícitos; qualquer contexto declarado tem fonte e período vigente conforme contrato futuro.

### Estímulo

Selecionar manualmente atividade/ambiente/role; receber sugestão de sensor ou perfil; apresentar sinal ambíguo (velocidade, altitude, dispositivo, rede); expirar contexto ou revogar localização/Guardian; tentar usar funcionalidade de rádio baseada no contexto.

### Esperado

Seleção manual prevalece e é revisável; sugestão é rotulada como sugestão, nunca confirmação compulsória; informação desconhecida fica unknown; frescor/incerteza/fonte são expostos quando disponíveis. Contexto pode adequar interface/sensores/conectividade/opções sem enviar posição, criar SOS ou conceder acesso RF. Eventos/logs não convertem sugestão em fato confirmado.

### Invariante

Contexto é atividade temporária, não atributo permanente; sensores/velocidade/altitude/equipamento/rede não provam ambiente ou papel; contexto não equivale a permissão RF, localização compartilhada, SOS, Guardian ou consentimento; chat/PTT e precisão obedecem política de localização; unknown não bloqueia funções fundamentais.

### Negativo

Inferência contraditória, expirada ou sem proveniência não substitui a seleção manual nem habilita frequência/transmissão; permissão negada não é contornada; ação indisponível por país/dispositivo/API não é alegada como executada.

### Degradação e recuperação

Sem sensor, permissão, conectividade ou frescor, conservar declaração ainda válida e marcar sinais indisponíveis unknown/uncertain; explicar limite e permitir revisão manual. Ao recuperar, não reinterpretar histórico como prova retroativa nem elevar privilégio sem autorização nova. Expiração, conflitos entre dispositivos e recuperação detalhada aguardam contrato.

### principal documental

**nome**: Margaret Hamilton

**estado**: confirmado para semântica de produto e cenários; domínio, segurança, plataforma e validação legal são dependências, sem transferência de responsabilidade operacional

### evidencias

- **fonte**: GodMesh/docs/00-produto/requisitos.md; **secao**: Requisitos do Guardian, CrowdFind e contingência; **linhas**: 56; **trecho**: Contexto operacional terra/ar/mar por atividade, sem inferência compulsória ou habilitação de RF por tipo presumido — APROVADO

- **fonte**: GodMesh/docs/02-dominio/contexto-operacional.md; **secao**: Cabeçalho, objetivo, modelo, regras e pendências; **linhas**: 3-22,37-38; **trecho**: atividade atual; seleção manual prevalece; sensores não provam ambiente/papel nem habilitam RF; unknown não bloqueia funções fundamentais; taxonomia/acessibilidade/país/compatibilidade pendentes

- **fonte**: GodMesh/docs/06-planejamento/decisoes.md; **secao**: ADR-042; **linhas**: 55; **trecho**: contexto terra/ar/mar é da atividade corrente; Guardian voluntário/autorizado/expirável; testes pendentes

- **fonte**: GodMesh/docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual, RF-027; **linhas**: 125; **trecho**: Aceite atual “Observar” e campos de negativo/degradação ainda a completar

### aplicabilidade

**produtos**: - Personal e Teams quando atividade atual afeta interface, sensores, conectividade ou acompanhamento

- Node somente para transportar/processar contexto escopado que lhe foi explicitamente fornecido, sem inferir atividade humana

- Platform/SDK somente quando extensão futura definir transporte/contexto; não requisito para serviços sem atividade humana

**plataformas**: - Web

- macOS universal arm64/x86_64

- iOS

- Windows desktop

- Linux desktop

- Android

- Servidor Linux em contêiner

**regra**: Clientes em todas as plataformas permanecem no escopo, com capacidades reais por SO/hardware e fallback unknown; serviço de servidor não presume semântica pessoal. Não aplicável a manutenção interna de serviço sem usuário/contexto humano, registrar justificativa no cenário.

### decisoes pendentes

- **questao**: Taxonomia de atividade/role; **alternativas**: - Lista fixa versionada; - Núcleo versionado com extensão custom declarada (recomendação); - Vocabulário livre; **recomendacao**: Núcleo versionado com extensões explicitamente não interpretadas como autorização.; **consequencia**: Exige regras de compatibilidade, rótulo, migração e semântica de unknown.

- **questao**: Sugestões com base em contexto/sensores; **alternativas**: - Somente sob pedido; - Sugestão automática explicável, descartável e não privilegiadora (recomendação); - Inferência automática confirmada; **recomendacao**: No máximo sugestão automática explicável e dispensável; nunca habilitar capacidade ou RF.; **consequencia**: Requer proveniência, acessibilidade, permissões e fallback por plataforma.

- **questao**: Jurisdição, atividade aérea/marítima e dispositivos/APIs elegíveis; **alternativas**: - Definir por país e classe de aparelho após pesquisa (recomendação); - Aplicar conjunto universal sem validação; - Restringir recursos até verificação local; **recomendacao**: Definir por país/aparelho com evidência oficial e manter funcionalidades fundamentais unknown/degradadas quando não elegíveis.; **consequencia**: Sem estas decisões, não há confirmação de atividade/compatibilidade ou autorização legal.

### dependencias

- Taxonomia final, regras por país, compatibilidade de dispositivo e acessibilidade conforme contexto-operacional.md

- Lea e donos de plataforma: jornadas e capacidades reais de sensores/permissões

- Bruce/Paul e donos de localização/Guardian: consentimento, precisão, retenção e projeção

- Jouni/Jouni ou dono de integração RF e autoridades aplicáveis: contexto não autoriza transmissão

- Contrato de domínio para fonte, período, precedência, expiração e concorrência entre dispositivos

### criterio fechamento documental

Taxonomia/role e extensão, fonte/proveniência/frescor/incerteza, transições/expiração/conflito, política de consentimento/localização, acessibilidade, matriz de APIs/entitlements por plataforma e dispositivos, regra por país e fronteira explícita com RF/aviação/navegação aprovadas; cenários de seleção, sugestão, unknown, revogação, offline/degradação e recuperação ligados a contrato e donos; revisão independente. Validação técnica/legal é futura; SPEC/testes NOT_ASSIGNED e observado NOT_EXECUTED.

### estado

proposto_aberto; decisão arquitetural ADR-042 aprovada, mas testes pendentes e domínio declara especificação proposta.

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-028

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Margaret Hamilton. Subescopos e deltas vinculados: [Conciliação margaret](#conciliacao-margaret). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Sessão draft; finalidade, horários, destinatários e grants individuais escolhidos; permissões independentes e precisão configurada; prazo/TTL válido.

### Estímulo

Enviar/aceitar/recusar convite; ativar sessão; consultar com grant válido; passar check-in; revogar/expirar/completar; revogar permissão SO; offline; tentar consulta pós-revogação.

### Esperado

Somente destinatário aceito lê dados no escopo/prazo; overdue envia apenas aviso de ausência de check-in; expiração/revogação/completion impedem novas leituras e encerram beacon; offline mostra idade/qualidade da posição.

### Invariante

Overdue/stale/sem resposta não é SOS; consentimento/grant não se estende além da finalidade e prazo; administração Teams não substitui transparência; precisão não vaza a destinos públicos.

### Negativo

Recusa, grant revogado, convite expirado ou falta de permissão nega acesso; tentativa de consulta após revogação falha; tentativa de escalar atraso a resgate público não autorizado é negada.

### Degradação e recuperação

Offline indica buffering/stale/unknown; check-in tardio preserva occurred_at e received_at sem retroagir confirmação; reconnect não restaura grant revogado nem cria SOS.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: False

**estado**: conflito_de_escopo

**principal recomendado**: Margaret Hamilton

**interfaces recomendadas**: - Nancy Leveson para overdue não virar SOS

- Bruce Schneier para privacidade/autorização

- Lea Verou para UX

- Paul Ramsey para localização/geoespacial

**fonte**: CONTEXTO-E-PAPEIS.md: Margaret lidera produto; Nancy lidera SOS; Paul/ Bruce/Lea têm interfaces de geo, privacidade e UX. Não há principal funcional Guardian explicitamente nomeado.

**justificativa**: O requisito cobre a jornada funcional de Guardian inteira; apenas sua fronteira overdue/SOS pertence diretamente à especialidade Nancy.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos Guardian/CrowdFind/contingência; **linhas**: 57; **trecho**: convite, prazo, rastreamento autorizado temporário, revogação, check-in e aviso de atraso sem SOS automático

- **arquivo**: docs/02-dominio/guardian-acompanhamento.md; **secao**: Estados, configuração, fluxo, check-ins e aceite; **linhas**: 8-20, 22-34, 47-51; **trecho**: overdue é um alerta de check-in, não um SOS.

- **arquivo**: docs/07-contratos/guardian-crowdfind.md; **secao**: Sessão/overdue/autorização; **linhas**: 8-15, 40-47; **trecho**: gmp.guardian.overdue.v1: check-in pendente, sem equivalência automática a SOS.; **estado**: especificação lógica; sem implementação verificada

- **arquivo**: docs/05-operacao/testes-mobile-guardian.md; **secao**: Guardian e privacidade; **linhas**: 12-19; **trecho**: Atraso de check-in gera notificação, mas não dispara SOS automaticamente.

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 126; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

### aplicabilidade

**produtos**: - Personal

- Teams

- Node como serviço autorizado

- Platform/SDK quando habilitado

**plataformas**: - Android

- iOS

- macOS

- Windows

- Linux

- Web

- Node Linux

**regra**: Mobile pode coletar background conforme APIs/permissões; interfaces desktop/web podem gerir/consultar capacidades permitidas; servidor valida grants/estado.

**nao aplicavel**: Sem sensor, background ou API, coleta contínua específica é unsupported/unavailable com explicação; convite, autorização, prazo e revogação continuam aplicáveis.

### decisoes pendentes

- **decisao**: Confirmar dono funcional Guardian.; **alternativas**: - Nancy como principal para o requisito completo; - Margaret como principal de produto com Nancy/Bruce/Lea/Paul em interfaces; **recomendacao**: Margaret lidera jornada e critérios de produto; confirmar nomeação formal do owner Guardian.; **consequencia**: Sem owner confirmado, critérios são proposta e não encerram a cobertura funcional.

- **decisao**: Definir tolerância de check-in, frequência, retenção e regra de atraso/relógio.; **alternativas**: - Valores globais; - Configuração por atividade/política/organização; **recomendacao**: Definir explicitamente por política após decisão de produto, sem default numérico aqui.; **consequencia**: Afeta overdue, notificações e experiência; não implica SOS.

### dependencias

- Política de consentimento e autorização

- Regras de retenção/localização

- Capacidades oficiais Android/iOS e outros alvos

- Owner Guardian nomeado

- UX acessível

- Plano de teste futuro

### criterio fechamento documental

Owner funcional confirmado; estados, convite, grants, término, atraso, revogação, tempo/frescor e offline estão relacionados a critérios positivos/negativos/recuperação; não aplicáveis por plataforma são justificados; overdue nunca vira SOS; parâmetros estão decididos ou explicitamente abertos.

### estado

proposta comportamental documental; atribuição conflitante; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-029

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-paul.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Capacidades e permissões atuais de cada fonte são conhecidas. Sessão Guardian e destinatários têm autorização válida. Disponibilidade de sensor não é presumida pelo modelo do dispositivo.

### Estímulo

Enviar observações GNSS, IMU/PDR, Wi-Fi, BLE e UWB em condições permitidas, negadas, indisponíveis, sem rede e com qualidade degradada; incluir RSSI ou sighting sem posição confiável.

### Esperado

Resultado preserva fonte ou combinação declarada, instante medido/recebido, qualidade e incerteza. Fonte sem suporte declara estado indisponível/unsupported. PDR segue regra aprovada de aumento de incerteza. RSSI não produz orientação/ponto. Sighting sem GNSS do observador registra proximidade sem posição do beacon.

### Invariante

Nenhuma fonte recebe precisão que não forneceu. Observação antiga não aparece como atual. Origem e incerteza não se perdem na fusão; identificadores e coordenadas brutas de AP/beacon não são publicados.

### Negativo

Permissão revogada impede nova coleta/publicação que dependa dela. Sensor ausente não é simulado. RSSI isolado não vira distância/direção. Beacon não recebe coordenada inventada. Campo de frescor ausente não é fabricado.

### Degradação e recuperação

Tela bloqueada, bateria baixa, restrição de SO ou falta de rede comunica coleta reduzida, offline e idade da última observação. Retomada conserva os tempos medido/recebido, ordem temporal e a diferença entre medição e interpolação. Limiar stale, taxa de amostragem e aumento numérico de incerteza aguardam decisão/homologação.

### decisions open

- Representação comum de incerteza

- Regra de combinação e dependência entre sensores

- Frescor/stale, frequência e orçamento de energia por plataforma

### principal documental

**owner**: Paul Ramsey

**status**: proposto_por_especialidade_e_confirmado_pelo_contribuidor

**basis**: A atribuição corresponde à incerteza, frescor e semântica de localização. Regras de sensores e capacidades móveis são interface com responsáveis de plataforma.

### evidencias

- **source**: docs/00-produto/requisitos.md:58; **literal**: Sensor fusion GNSS/IMU/Wi-Fi/BLE/UWB conforme hardware e APIs, com incerteza/frescor; sem falsas direções Wi-Fi 360°

- **source**: docs/01-arquitetura/guardian-localizacao-trajetoria.md:5-29,40-44; **literal**: Fontes condicionais; PDR tem erro crescente; RSSI não indica posição exata nem orientação; API conceitual separa coordenadas precisas.

- **source**: docs/02-dominio/modelo-dados.md:38-40,161-162; **literal**: Origem GNSS/BLE/estimada e posição de beacon corresponde à observação do gateway ou estimativa, não GNSS da tag.

- **source**: docs/05-operacao/testes-mobile-guardian.md:5-19; **literal**: Matriz documental exige distinguir fonte/precisão/idade e restrições de permissão e segundo plano.

### aplicabilidade

**products**: - Personal e Guardian

- Teams com aparelho/sensores autorizados

- Node como consumidor de observações recebidas

- Platform/SDK conforme capacidade declarada

**platforms**: - iOS SwiftUI

- Android

- macOS universal SwiftUI

- Windows

- Linux desktop

- Rust/Linux serviço futuro

**not applicable**: UWB, RTT ou outra fonte não se aplica a combinação sem hardware, API ou permissão correspondente; deve ser declarada indisponível, não um resultado positivo nem falha do serviço. Serviço Rust remoto não afirma acesso a sensores locais.

**future platform rule**: Aplicar por capacidade real e preservar Windows/Linux/Android após a primeira fase Rust/Linux, macOS universal e iOS.

### decisoes pendentes

**alternativas**: - Preservar distribuições/incerteza por fonte e fundir só com regra aprovada

- Fusão com política declarada por modalidade e incerteza propagada

- Exibir fontes separadas enquanto não houver modelo comum

**recomendacao**: Preservar a incerteza por fonte e exibir fontes separadas até aprovar regra de combinação.

**consequencia**: Sem limiar de stale, matriz de capacidades e algoritmo aprovado, não há aceite quantitativo de erro/frescor nem prova de suporte por plataforma.

### dependencias

- Chet Haase e Donny Wals: APIs e ciclo de vida por plataforma

- Alex Crichton: fronteira do núcleo local

- Michael Stonebraker: representação persistida

- Homologação de aparelhos/SO conforme pendências

### criterio fechamento documental

Contrato define fonte, timestamp, incerteza e qualidade para cada resultado, regras de fonte indisponível, PDR, RSSI, observação BLE e retomada offline; critérios se alinham a RF-029, modelo e política. Matriz por versão/plataforma fica identificada como pendente até homologação. Nenhuma execução é presumida.

### estado

criterio_proposto; modelo de fusao e parametros pendentes; implementação não verificada

### spec id

NOT_ASSIGNED

### test id

NOT_ASSIGNED

### observed execution

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-030

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Beacon desabilitado até opt-in; sessão autenticada, dispositivo vinculado, consentimentos separados para emitir/observar, grant/política válidos, plataforma com API compatível.

### Estímulo

Emitir/rotacionar token, observar e submeter avistamento, pausar/encerrar/revogar ou enviar token desconhecido, expirado, repetido e após partição.

### Esperado

Anúncio contém token efêmero sem identidade/coordenadas; só audiência autorizada recebe avistamento com incerteza/frescor; token revogado/expirado não atualiza posição; estado de revogação pendente é exibido quando dispositivo offline.

### Invariante

Permissões Guardian/beacon/observe/leitura precisa são independentes; rotação não garante anonimato nem prova distância; relay não comprova localização; posição do observador não é posição do beacon.

### Negativo

Sem autoridade, frescor, consentimento, grant ou chave, não emitir nem resolver localização; sem fallback para rede Find My/Find Hub ou terceiro; fila sem autorização comprovável é recusada segundo política.

### Degradação e recuperação

Exibir unsupported para API BLE indisponível; reconexão revalida sessão, epoch e política antes de publicar backlog, rejeitando dado fora da janela aprovada.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos Guardian/CrowdFind/contingência; **linhas**: 59; **trecho**: CrowdFind próprio com Beacon opcional e ID rotativo, sem depender de Find My/Find Hub e sem rastreamento não autorizado; **estado origem**: APROVADO como direção

- **arquivo**: docs/04-seguranca/privacidade-crowdfind.md; **secao**: Permissões independentes; **linhas**: 12; **trecho**: Nenhuma permissão implica automaticamente outra.

- **arquivo**: docs/04-seguranca/privacidade-crowdfind.md; **secao**: Minimização e ameaça; **linhas**: 15-18; **trecho**: Beacon transmite somente token efêmero; não transmite coordenadas, nome, GMC nem telefone.

- **arquivo**: docs/07-contratos/credenciais-crowdfind.md; **secao**: Falhas, partição e revogação; **linhas**: 100; **trecho**: Servidor de revogação desconhecido/stale: negar emissão e consulta precisa.

### aplicabilidade

**produtos**: Personal/Teams/Node/Platform conforme papel de titular, observer ou serviço

**plataformas**: Servidor e clientes macOS, iOS, Windows, Linux desktop, Android e web; BLE somente onde SO/aparelho suporta API e permissão

**nao aplicavel**: Aparelho sem API/permissão BLE não anuncia; deve declarar unsupported sem simulação. Outros papéis preservam controles de autorização/consulta.

### decisoes pendentes

- **tema**: token e janela offline; **alternativas**: - token aleatório de janela curta; - janela maior para tolerar partição; **recomendacao**: Decidir máximo por compatibilidade e risco antes de produção; **consequencia**: Janela curta reduz replay/correlação e aumenta renovações/indisponibilidade; longa aumenta exposição

- **tema**: suite de cifra, retention e limites de consulta; **alternativas**: - perfil criptográfico por serviço receptor; - não habilitar dados precisos sem perfil; **recomendacao**: Não tratar proposta HPKE como aprovação; **consequencia**: Depende de Kim, Martin, política C08 e decisão de jurisdição

### dependencias

- Kim Cameron: credenciais/chaves

- Martin Kleppmann: sync/replay

- Chet Haase e Donny Wals: APIs BLE/plataforma

- Jack Ganssle: hardware beacon

- Paul Ramsey: localização/qualidade

### criterio fechamento documental

Transições, audiência, token, frescor, falhas e N/A por plataforma concordam em integração, privacidade e contrato de credenciais; decisões e revisão independente registradas.

### estado

proposta comportamental; sem homologação BLE ou teste de produto

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-031

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-vint.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

The platform exposes a signal through an allowed API or permitted system hint; the observer records source and time without unnecessary private identifiers. Device radio state, IP path, GodMesh service reachability, satellite evidence and per-service capability are separate. Without a current source, initial state is unknown or unavailable, not an inferred loss of service.

### Estímulo

Observe available cellular signal present/absent, IPv6/IPv4 change, local Wi-Fi/LAN, permitted satellite indication, user-only report, system hint, and source unavailable, revoked or stale. Assess service capability separately on each path.

### Esperado

Produce an operational state with observation time, path class, evidence method and source/confidence under a versioned GMP contract. Classify a carrier satellite or satellite-terminal Wi-Fi path only when evidence distinguishes it; otherwise retain unknown. A system satellite indication does not claim that GodMesh can transmit; service eligibility requires its own permitted check. Preserve user reports as user reports. Report last delivery confirmation separately from connectivity.

### Invariante

No cellular service does not mean no internet. Lost 4G, SSID, brand assertion or terminal availability does not prove carrier satellite or GodMesh app eligibility. Do not claim satellite activation, universal coverage, SOS delivery or human response without separate evidence. Public telemetry does not include IMSI, ICCID, telephone number, cell ID, BSSID or precise location; local use requires an explicitly authorized purpose.

### Negativo

When no allowed API exists, report unknown/unavailable or classify the source as user_report/system_hint. Do not report platform-confirmed satellite. If an operator, device or plan limits service to SMS or approved apps, do not claim GodMesh data capability. A refused, stale or invalid source cannot support an active status; offer manual guidance where applicable.

### Degradação e recuperação

If cellular service is lost while Wi-Fi remains available, preserve the IP path and reassess the service over Wi-Fi. If no route is evidenced, report offline or unknown according to evidence and keep delivery unconfirmed. When an allowed source returns, update its time and method, reassess service eligibility and expire stale information under the freshness policy, whose values remain pending. A network change does not create an SOS or close an incident.

### future spec and test ids

NOT_ASSIGNED

### observed

NOT_EXECUTED

### principal documental

**name**: Vint Cerf

**status**: confirmed_for_connectivity-state-and-route-classification

**basis**: The matrix assigns Vint to RF-031 and the transport/degradation semantics fit the assignment. Chet Haase and Donny Wals remain reviewers for platform API capabilities; this does not transfer ownership.

### evidencias

- **file**: docs/00-produto/requisitos.md; **section**: Requisitos Guardian, CrowdFind e contingência; **line**: 60; **sha256**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664; **quote**: | RF-031 | DICE monitora conectividade celular/IPv6/IPv4/local e classifica satélite somente com evidência permitida | APROVADO | `01-arquitetura/conectividade-contingencia-satelite.md` |

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **section**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **line**: 129; **sha256**: c00e82090170872591cc5318f89b83d0a7af37294daae9e76e17795ec314e4e9; **quote**: | RF-031 | [origem L60](../00-produto/requisitos.md) | Direção APROVADA na origem; vínculo individual a ADR ainda a conciliar | [01-arquitetura/conectividade-contingencia-satelite.md](../01-arquitetura/conectividade-contingencia-satelite.md) | Vint Cerf — proposto por especialidade, confirmação por requisito pendente | Observar: DICE monitora conectividade celular/IPv6/IPv4/local e classifica satélite somente com evidência permitida. Negativa/degradação e limiares: completar no registro comportamental. | Aberto: conciliar decisão, aceite e cenário com o principal; não liberado | NOT_ASSIGNED | NOT_ASSIGNED | NOT_EXECUTED |

- **file**: docs/01-arquitetura/conectividade-contingencia-satelite.md; **section**: Objetivo e princípios; Classes de caminho; Máquina de estados; Prioridade; Implementação; Privacidade e observabilidade; **lines**: 5-11, 13-22, 24-43, 67-90; **sha256**: 7e12dcd5f6ab13945321e6809d5f5b003f59f18bc51b33ddc63f6b81d9c2796f; **quotes**: - Distinguir `radio_service_state` (registro celular), `network_path` (caminho IP), `reachability` (consegue comunicar com serviços GodMesh), `satellite_state` (disponibilidade declarada/observada) e `capability` (o que o serviço realmente suporta).; - `no_service` não é sinônimo de `no_internet`: uma rede celular pode perder sinal e Wi-Fi continuar funcionando.; - Falha de conexão não prova que existe satélite disponível. É proibido anunciar “Starlink ativado” a partir de simples perda de 4G.; - Indicador `satellite_detected` terá `verification_method=platform_api|system_hint|user_report|unknown`, sem afirmar uma certeza superior à fonte.

- **file**: docs/07-contratos/conectividade-contingencia.md; **section**: Estados; Exemplo de alteração de rede; Mensagens de erro; Priorização e privacidade; **lines**: 5-11, 13-30, 49-57; **sha256**: 6050c3a0e238088a03bcaf65c2bd2114c3eef75dce93c884bc5625f8a12f66f1; **quotes**: - `satellite_evidence`: `platform_api|system_hint|user_report|none|unknown`.; - Na ausência de API, marcar `unverified`.; - `delivery_not_confirmed`: solicitação em fila ou encaminhada, sem ACK do destino.

- **file**: docs/06-planejamento/decisoes.md; **section**: ADR-045 e ADR-046; **lines**: 58-59; **sha256**: 11321182f7b87fbfdd2c2f12dd3273216e64fc623727cf03413728954b2f8903; **quotes**: - | ADR-045 | Monitoramento de rede móvel e transporte satelital pelo DICE: sinais de fonte/certeza, sem declarar satélite ativo sem evidência; Starlink Direct-to-Cell ≠ terminal Wi-Fi | Aprovada como arquitetura; APIs e operadores a validar |; - | ADR-046 | Automação somente com API/consentimento; quando indisponível, interface orienta **ação manual**; não fecha apps terceiros, não altera assinatura/operadora/satélite e preserva funções essenciais de Guardian e SOS | Aprovada como regra de produto |

### aplicabilidade

**products**: - Personal

- Teams

- Node

- Platform/SDK

**platforms**: IP/local connectivity monitoring applies to GodMesh components with IP interfaces and an allowed evidence source. Cellular/satellite signals apply only to devices/platforms that expose them through permitted APIs or system indications. The OS/version/device/operator/region support matrix is pending confirmation. No OS is excluded by assumption.

**not applicable**: Cellular/satellite sensor fields are not applicable to a Linux server instance with no such interface or allowed source; omit or declare unknown per contract while continuing IPv4/IPv6 checks. Satellite classification is not applicable to radio-only hardware with no satellite evidence source; assess its IP gateway separately.

### decisoes pendentes

**items**: - **decision**: Define the permitted evidence-source table and wording for platform_api, system_hint, user_report and unknown; define freshness/expiry; confirm APIs by OS version, device, operator and region; decide which GodMesh services may use carrier satellite paths and with what limits.; **alternatives**: - Accept only official platform assertions: conservative, but produces more unknown states.; - Keep platform assertions, system hints and user reports as distinct evidence levels: more informative, but requires distinct confidence and UI wording.; **recommendation**: Retain the existing separate evidence categories; never promote system_hint or user_report to proof of GodMesh service availability. Confirm terminology with platform owners before approving UI wording.; **consequence if open**: Satellite class and service availability remain unknown whenever the available source does not prove them; no thresholds, support or app capability can be presumed.; **scope**: - Android and iOS devices subject to platform confirmation; - other IP-capable clients only for exposed signals; - operators and regions; - GodMesh services using constrained paths

### dependencias

- ADR-045 and ADR-046 approved architecture and manual-action limits

- Chet Haase and Donny Wals for official platform API/version capabilities

- Margaret Hamilton and Lea Verou for product behavior and UI wording, where in scope

- Kim Cameron and Bruce Schneier for privacy and evidence-source policy

- Product owner for permitted service classes and satellite path policy

- RFC- or provider-level network profile for reachability, if the service uses an IP path

### documentary links

- ADR-045

- ADR-046

- PI-vint-001

### criterio fechamento documental

The contract distinguishes device, path, service reachability, satellite evidence and capability; each reported satellite class maps to a permitted evidence source and freshness policy; unknown/stale/unavailable behavior, privacy limits, manual guidance and per-service eligibility are specified; the platform and service matrix identifies supported and not-applicable cases with reasons. This does not verify runtime behavior or operator availability.

### estado

acceptance proposed; platform evidence matrix, freshness and service eligibility decisions open; implementation not verified; validation NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-032

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-lea.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

A combinação plataforma/versão/aparelho/autorização não oferece API oficial apta a realizar a ação; estado de rota, serviço, capability e proveniência são conhecidos ou explicitamente unknown.

### Estímulo

A pessoa abre uma orientação para recuperar conectividade ou alterar uma configuração sob seu controle.

### Esperado

Mostrar serviço afetado e nível de evidência; oferecer instrução voluntária específica da plataforma e abrir Ajustes apenas quando houver caminho suportado. A orientação fica suggested; abertura vira opened_settings; somente confirmação do usuário ou observação de API oficial autoriza estado confirmado. Sem evidência, mostrar unverified/unknown. Não alegar mudança de operadora/satélite/app de terceiro.

### Invariante

Nenhuma mudança de plano/assinatura/operadora, ativação satelital inferida, alteração/fechamento de app terceiro ou SOS automático. Sugestão e abertura de Ajustes nunca equivalem a mudança concluída. Não expor IMSI, telefone, célula, BSSID ou histórico preciso de deslocamento em chat/PTT.

### Negativo

API inexistente/restrita, permissão negada, ação impossível ou sugestão dispensada: explicar limite sem repetir coercivamente; manter serviços independentes; preservar unknown/unverified quando fonte insuficiente.

### Degradação e recuperação

Offline/limitado: preservar estado local e dizer que a ação não foi confirmada; não repetir automaticamente uma mudança externa. Após reconexão, consultar fonte autorizada; atualizar somente com evidência nova. Sem resultado verificável, permanecer unknown/unverified.

### principal documental

**name**: Lea Verou

**status**: proposto; adequado para UX e conteúdo de interação; confirmação formal pendente

**rationale**: A exigência trata do comportamento comunicativo da interface e ação voluntária. Confirmação de APIs/capacidades nativas depende dos responsáveis técnicos de plataforma/núcleo, não de Lea isoladamente.

**conflict**: None

**collaborators**: - Chet Haase (Android)

- Donny Wals (Apple)

- Alex Crichton (núcleo offline)

**source assignment**: docs/06-planejamento/matriz-rastreabilidade.md:130

### evidencias

- **source**: docs/00-produto/requisitos.md; **section**: Requisitos — conectividade e contingência; **lines**: 61; **excerpt**: Sem automação disponível, orientar ação manual, sem mudar configurações de operadora, satélite ou aplicativos terceiros; **status**: APROVADO na origem

- **source**: docs/07-contratos/conectividade-contingencia.md; **section**: Estados; Exemplo de orientação manual; Mensagens de erro; Priorização e privacidade; **lines**: 5-11, 32-57; **excerpt**: action_status separa suggested/opened_settings/confirmed_by_user/not_possible/dismissed; sem confirmação por usuário ou API oficial, marcar unverified; delivery_not_confirmed não implica ACK do destino.; **status**: proposta de eventos v1

- **source**: docs/01-arquitetura/conectividade-contingencia-satelite.md; **section**: Objetivo e princípios; Máquina de estados; Automação versus intervenção manual; Mensagens de UI; **lines**: 5-11, 24-65; **excerpt**: Usar somente caminhos disponibilizados/autorizados; orientar quando ação manual é necessária; não modificar preferências sem autorização.; **status**: especificação proposta, não implementação

- **source**: docs/06-planejamento/decisoes.md; **section**: ADR-046; **lines**: 59; **excerpt**: Automação somente com API/consentimento; quando indisponível, interface orienta ação manual; não modifica recursos de terceiros.; **status**: aprovada como regra de produto

### aplicabilidade

**products**: **Personal**: Aplicável às orientações para o próprio usuário, com visibilidade privada por padrão.

**Teams**: Aplicável; compartilhamento de estado depende de autorização/tenant e política definida.

**Node**: Aplicável somente a console humano de administração; serviço headless é N/A para apresentação, mas mantém proibição de mutação inferida e evidência de estado.

**Platform/SDK**: N/A para UI própria; deve preservar estado/proveniência e diferenciar sugestão de confirmação para os clientes consumidores.

**platforms**: **macOS universal e iOS SwiftUI**: Aplicável se a superfície oferecer a orientação; disponibilidade de API/caminho Ajustes precisa ser verificada por versão.

**Windows, Linux desktop e Android**: Continuam no escopo; aplicar por capability/API autorizada, sem supor escolha/versão final de framework.

**Web**: Aplicável somente a ações expostas pela superfície; APIs do SO não são presumidas. Se não houver UI/capacidade, justificar N/A e manter estado sem ação automática.

**nao aplicavel justificativa**: N/A limita-se à apresentação em produto headless ou superfície que comprovadamente não oferece a ação; não exclui os invariantes de evidência, autorização e não mutação. Ausência de implementação não justifica N/A.

### decisoes pendentes

- **decision**: Classes de ação, momento e prioridade da orientação; **alternatives**: - contextual e não bloqueante; - alerta prioritário quando risco operacional previamente aprovado exigir; **recommendation**: Contextual e não bloqueante por padrão; prioridade somente por política de risco aprovada.; **consequence**: Sem decisão, o caso permanece proposta e não determina copy/prioridade de produção.

- **decision**: Matriz de APIs e ações suportadas por plataforma/versão; **alternatives**: - matriz por sistema/versão/capability; - declaração genérica sem granularidade; **recommendation**: Matriz por sistema/versão/capability, validada por Chet/Donny/Alex.; **consequence**: Sem matriz não é possível afirmar que abrir Ajustes ou confirmar mudança está disponível.

- **decision**: Compartilhamento de estado de conectividade com perfil/contatos; **alternatives**: - privado local por padrão; - compartilhar dados mínimos sob autorização explícita existente; **recommendation**: Privado local por padrão; compartilhar somente sob autorização e finalidade já aprovadas.; **consequence**: Exposição e destinatários permanecem em aberto sem decisão de produto/privacidade.

### dependencias

- crosswalk técnico de estados e evidência do contrato GMP

- capacidade/API real por versão revisada pelos responsáveis de plataforma

- política de privacidade e autorização para compartilhamento

- decisão de produto sobre forma e prioridade da orientação

### criterio fechamento documental

Aprovada matriz de ação/evidência/fallback por plataforma; sugestões não alegam execução; confirmação tem fonte definida; casos de negação, dispensa, unknown, offline e recuperação possuem esperado observável; responsáveis técnicos revisam capacidades; produto decide visibilidade e redação.

### estado

aceite documental proposto; decisão e revisão pendentes; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-033

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Lea Verou. Subescopos e deltas vinculados: [Conciliação lea](#conciliacao-lea). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Cliente conhece estado e fonte de conectividade, capacidades do caminho, permissões e atividades simultâneas; pendências SOS/Guardian/mensagens continuam identificáveis.

### Estímulo

Limitar banda/energia/custo, perder celular mantendo Wi-Fi, trocar rota, entrar em local-only/offline, reportar satélite com capacidade desconhecida e acumular cargas de diferentes prioridades.

### Esperado

Aplicar ordem conceitual P0 SOS, P1 comunicação/check-in, P2 mensagens/sync, P3 descoberta/telemetria/manutenção; reduzir cargas do próprio GodMesh; declarar elegibilidade por serviço; instruir manualmente quando API não permite ação; preservar recursos oficiais de emergência.

### Invariante

Não controlar apps terceiros nem desligar funções OS; link/satélite não elegível não transporta nem se anuncia como capaz; degradação não rebaixa SOS silenciosamente; troca de rota preserva auth/idempotência; SOS não é disparado por conectividade.

### Negativo

Queda celular com Wi-Fi não significa offline total; SMS-only satélite não habilita GMP/PTT; sem API/permissão o sistema não finge mudança; sem rota fila declara pendência.

### Degradação e recuperação

Pausar ASR/anexos/telemetria opcionais quando política e recursos assim determinam; após retorno, reordenar por prioridade sem duplicação e atualizar somente com evidência real; bateria baixa não desativa comunicação oficial nem transforma pending em delivered.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: False

**estado**: conflito_de_escopo

**principal recomendado**: Lea Verou

**interfaces recomendadas**: - Chet Haase (Android)

- Donny Wals (iOS)

- Alex Crichton (offline/Rust)

- Brendan Gregg (método e observabilidade)

- Nancy Leveson (salvaguardas SOS)

**fonte**: CONTEXTO-E-PAPEIS.md: Lea lidera frontend/UX; Chet/Donny plataformas e Alex núcleo offline; Nancy SOS.

**justificativa**: O requisito trata comportamento transversal de cliente, energia/rede, plataformas e preservação de funções OS; excede SOS como domínio.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos Guardian/CrowdFind/contingência; **linhas**: 62; **trecho**: Em conexão restrita, priorizar SOS/check-in/mensagens e reduzir cargas não essenciais do próprio app, mantendo comunicação de emergência do SO; **estado origem**: APROVADO como direção

- **arquivo**: docs/01-arquitetura/conectividade-contingencia-satelite.md; **secao**: Princípios, política de prioridade e privacidade; **linhas**: 5-11, 67-89; **trecho**: Preservar ações essenciais ... Não orientar a desligar indiscriminadamente todos os aplicativos.; **estado**: arquitetura proposta; capacidades/limites pendentes

- **arquivo**: docs/07-contratos/conectividade-contingencia.md; **secao**: Estados/priorização; **linhas**: 5-9, 30-57; **trecho**: Nenhuma mudança de rede cria SOS automaticamente.; **estado**: proposta de eventos v1

- **arquivo**: docs/01-arquitetura/aplicativo-operacao-normal.md; **secao**: Priorização e energia; **linhas**: 66-70; **trecho**: P0: incidente SOS ativo ... P3: descoberta de Wi-Fi, telemetria voluntária e tarefas de manutenção.; **estado**: APP-ARC-001 aprovada; implementação pendente

- **arquivo**: docs/05-operacao/testes-mobile-guardian.md; **secao**: Monitoramento e SOS; **linhas**: 21-38; **trecho**: Satélite ativo mas limitado a SMS/apps específicas não recebe PTT, anexos e GMP inelegível.

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 131; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

### aplicabilidade

**produtos**: - Personal

- Teams

- Platform/SDK

- Node/backend para filas que hospeda

**plataformas**: - Android

- iOS

- macOS

- Windows

- Linux

- Web

- servidor Linux

**regra**: Clientes aplicam política dentro das APIs/permissões disponíveis; backend/Node aplica prioridades de suas filas, sem afirmar controle sobre sistema operacional.

**nao aplicavel**: Subcomportamento específico de OS pode ser unsupported quando API não existe; informar a limitação. A regra geral de prioridade e honestidade aplica a todos os clientes compatíveis.

### decisoes pendentes

- **decisao**: Definir se P0-P3 é global e como tratar fairness/preempção.; **alternativas**: - Prioridade estrita; - Prioridade com quotas/fairness; - Política específica por serviço/perfil; **recomendacao**: Formalizar por serviço/perfil e cenário de carga, com SOS/check-in preservados e starvation control decidido; valores após baseline.; **consequencia**: Sem isso não há critério quantitativo de latência/capacidade ou proteção contra starvation.

- **decisao**: Definir budgets, TTL/retry e capacidade satelital por dispositivo/região/operadora.; **alternativas**: - Supor suporte universal; - Declarar capabilities por fonte/perfil; **recomendacao**: Declarar por fonte/perfil e manter unknown quando sem evidência.; **consequencia**: Sem catálogo e teste por plataforma, só aceites de estado honesto podem ser considerados.

- **decisao**: Confirmar principal documental do comportamento cliente.; **alternativas**: - Nancy principal transversal; - Lea principal com responsáveis de plataforma/offline/SRE em interfaces; **recomendacao**: Lea como principal UX/cliente; Nancy revisa o perigo de suprimir SOS/serviços essenciais.; **consequencia**: Matriz precisa manter estado pendente até Alan/usuário resolver a atribuição.

### dependencias

- Decisões de produto sobre carga/prioridade

- APIs/permissões e versões de plataforma

- Cenários e baseline/budgets

- Capacidade e elegibilidade de rotas por operadora/região

- Offline/sync/idempotência

- Owners e procedimentos operacionais

### criterio fechamento documental

Principal UX/cliente e interfaces confirmados; estados/fonte/capability/priority e comportamento negativo/degradado/recovery definidos; políticas quantitativas aprovadas por cenário ou declaradas pendentes; justificativas de unsupported por plataforma rastreadas; teste futuro liga a estado e evidência sem alegar execução.

### estado

proposta comportamental documental; atribuição conflitante; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-034

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-lea.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Para o perfil e superfície existe um inventário declarado de áreas, capacidades, autorização, estado de rede/serviço e política de dados locais; atividades e recibos são obtidos das fontes de estado autorizadas.

### Estímulo

Pessoa abre app online/offline; inicia PTT e Guardian simultaneamente; SOS inicia ou chega; muda rede/permissão; ou retorna após reboot/encerramento de processo.

### Esperado

Áreas acordadas (Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS) são navegáveis na superfície suportada. Offline mostra dados locais autorizados, estado offline e pendências não entregues. PTT/Guardian mantêm estados simultâneos quando elegíveis; SOS pode ganhar prioridade sem apagar estado. Capacidade ausente ou permissão negada informa a função afetada e mantém outras funções independentes. A interface apresenta somente o estado evidenciado pelo contrato e a política de autorização/localização.

### Invariante

Não colapsar atividades simultâneas em estado mutuamente exclusivo; submissão/persistência local não vira entrega, confirmação do dispositivo, ACK humano ou incidente resolvido; sem autorização não ler/compartilhar localização; não expor dados restritos em superfície pública/tela bloqueada; não ultrapassar limites de background do SO.

### Negativo

Permissão negada/revogada, capability unavailable, bridge indisponível, hardware ausente, sessão expirada, erro de autenticação/ACL ou operação recusada: indicar serviço/causa sem sucesso genérico, preservar atividade/dados conforme política e oferecer cancelar/encerrar/recuperar; serviços independentes continuam disponíveis. Bridge opcional indisponível não desativa serviço nativo.

### Degradação e recuperação

Sem rota, iniciar/navegar dados locais autorizados e manter alterações no estado local previsto pelo contrato sem confirmação remota. Após reboot/rede recuperada, reconstruir do estado persistido, sincronizar idempotentemente e atualizar após evidência. Resultado incerto/conflito/expiração exige reconciliação segundo contrato, não novo efeito com identidade nova. Background limitado mostra frescor/limite; retoma apenas quando SO e autorização permitirem.

### principal documental

**name**: Lea Verou

**status**: proposto; adequado como principal de UX/frontend; confirmação formal pendente

**rationale**: Requisito define áreas e comportamento de aplicativo, atividades simultâneas e modo offline. Persistência/eventos e limites nativos requerem colaboração técnica específica.

**conflict**: None

**collaborators**: - Alex Crichton (offline/persistência cliente)

- Chet Haase (Android)

- Donny Wals (Apple)

- Martin Kleppmann e Graydon Hoare (eventos/API)

**source assignment**: docs/06-planejamento/matriz-rastreabilidade.md:132

### evidencias

- **source**: docs/00-produto/requisitos.md; **section**: Requisitos — aplicativo, Wi-Fi comunitário, mapa e presença dinâmica; **lines**: 68; **excerpt**: Aplicativo normal com Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS; modos simultâneos e offline; **status**: APROVADO na origem

- **source**: docs/01-arquitetura/aplicativo-operacao-normal.md; **section**: §§2-8 Navegação; modos; inicialização; primeiro/segundo plano; prioridade/energia; privacidade; critérios de aceite; **lines**: 9-89; **excerpt**: Sete áreas; atividades simultâneas; status por serviço; capacidades/permissões; limites de background; offline sem declarar entrega; SOS prioriza sem apagar atividades anteriores.; **status**: especificação aprovada, implementação pendente; estrutura Rust é ilustrativa (§3, L43)

- **source**: docs/06-planejamento/decisoes.md; **section**: ADR-047, ADR-053 e ADR-055; **lines**: 68, 74, 79-104; **excerpt**: Atividades simultâneas e prioridades aprovadas; plataformas aprovadas, recomendações de frameworks e critérios de suporte/validação permanecem pendentes.; **status**: decisões com qualificadores explícitos

- **source**: docs/06-planejamento/matriz-rastreabilidade.md; **section**: Cobertura individual; Registro comportamental e cobertura de cenários; **lines**: 81-85, 132, 148-152; **excerpt**: Atribuição é proposta; aceite é proposta; cenários, decisões e aplicabilidade por produto/plataforma devem ser completados; NOT_ASSIGNED/NOT_EXECUTED permanecem até atribuição/execução.; **status**: matriz de planejamento, não validação

### aplicabilidade

**products**: **Personal**: Aplicável ao cliente cotidiano e às áreas autorizadas.

**Teams**: Aplicável; conteúdo/camadas/ações dependem de membership, ACL e políticas do tenant.

**Node**: Aplicável a console do operador quando houver cliente com UI; processo headless é N/A para navegação, mantendo estados operacionais e administração sob seu contrato.

**Platform/SDK**: N/A para telas próprias do SDK; aplicável à preservação e exposição dos estados/capacidades para clientes, sem perda semântica.

**platforms**: **macOS universal e iOS SwiftUI**: Dentro da primeira implementação declarada; cada plataforma deve mapear áreas e limites de lifecycle/permissão.

**Windows, Linux desktop e Android**: Permanecem no escopo completo; frameworks recomendados ainda são propostas na ADR-055 e não definem aceite por si.

**Web**: Topologia lista Web; abrangência das sete áreas/capacidades não foi definida. Precisa decisão explícita de produto; não presumir paridade nem excluir Web do escopo integral.

**nao aplicavel justificativa**: N/A é aceitável para fluxo visual em SDK/serviço headless ou superfície que produto documente como não oferecida; preservar contrato e dados semânticos onde aplicável. Uma plataforma futura não é N/A apenas por implementar depois; ausência atual de código não prova inaplicabilidade.

### decisoes pendentes

- **decision**: Áreas comuns versus condicionadas por perfil/autorização; **alternatives**: - mesma navegação com affordances condicionadas por capability; - conjuntos de áreas distintos por perfil; **recommendation**: Catálogo comum de navegação, com affordances condicionadas por capability e razão acessível quando indisponível.; **consequence**: Sem decisão, não se fecha a matriz de cada produto/tenant.

- **decision**: Escopo funcional Web; **alternatives**: - cliente Web completo; - conjunto explícito de operações Web; **recommendation**: Definir capacidades operação a operação antes das SPECs, incluindo limites de background/permissões.; **consequence**: A presença de Web na topologia não define compatibilidade ou paridade.

- **decision**: Política de persistência local, frescor, fila e recuperação; **alternatives**: - parâmetros definidos por domínio/serviço com orçamento aprovado; - política genérica comum; **recommendation**: Parâmetros por classe de operação após conciliação Alex/Martin/Graydon; manter unknown se evidência insuficiente.; **consequence**: Duração, limites de fila, retenção e janela de sincronização não podem ser preenchidos sem decisão normativa.

- **decision**: Comportamento de prioridade concorrente sob restrição de recursos; **alternatives**: - regras P0-P3 com efeito/feedback por cenário; - apenas ordem nominal; **recommendation**: Definir efeito e feedback por cenário, preservando prioridade aprovada e limites do SO; medir parâmetros depois.; **consequence**: A tabela P0-P3 não determina sozinha quais tarefas podem interromper/retomar nem limites energéticos.

- **decision**: Acessibilidade e privacidade de superfície/notificação; **alternatives**: - critérios comuns com limites por superfície; - validações genéricas por cliente; **recommendation**: Critérios comuns de resultado, com comportamento e fallback por plataforma; alinhar à contribuição anterior requisitos UX sem declarar conformidade medida.; **consequence**: Sem decisão de conformidade e política de lock screen, aceite permanece aberto.

### dependencias

- contratos de estado local e recuperação com Alex

- estados de evento/API e idempotência com Martin/Graydon

- permissões/lifecycle com Donny/Chet

- política de localização, retenção e autorização

- decisão de produto sobre Web e escopo por perfil

- revisão independente da matriz de cenários

### criterio fechamento documental

Matriz aprovada por produto/perfil/superfície declara áreas, capabilities, autorização, fallback e N/A justificado; cenários normal/concorrência/offline/revogação/erro/reboot/recuperação têm esperado e origem de estado; nenhuma camada de UI promove aceitação local/remota a entrega sem evidência; rastreabilidade a requisitos e futuros casos está completa; decisões e parâmetros abertos têm dono e consequência.

### estado

aceite documental proposto; decisões e revisão pendentes; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-035

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jouni.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Inventário local não contém a observação; plataforma, versão, estado foreground/background e permissões/capabilities disponíveis são registrados. Nenhuma permissão para ler senhas salvas é solicitada.

### Estímulo

O SO disponibiliza observação/resultado de scan permitido; a sincronização privada opcional está desligada ou foi habilitada com consentimento e escopo.

### Esperado

Guardar observação local privada com origem/frescor; sem opt-in não enviar ao servidor. Com opt-in, aceitar somente wifi.inventory.private e não exibir em mapa nem para terceiros. Não criar owner_verified, share, grant, credencial ou publicação.

### Invariante

Scan, SSID/BSSID, senha conhecida, proximidade e conexão anterior não provam autoridade; inventário não lê nem transmite segredo.

### Negativo

Permissão recusada/revogada, API indisponível ou erro não gera observação nem sucesso falsos. O cliente informa permission_required, unknown ou unsupported e mantém o aplicativo utilizável.

### Degradação e recuperação

Resultado Android defasado/throttled ou ausência de scan geral no iOS preserva o registro anterior com instante original. Após recuperar permissão/serviço, somente observação nova renova frescor.

### decisao

Frescor/expiração, retenção, sincronização privada, API mínima, suporte macOS sandboxed e estados canônicos precisam ser decididos; sem valores inventados.

### principal documental

**nome**: Jouni Malinen

**estado**: confirmado_para_esta_complementacao; proposta da matriz ainda aguarda conciliacao de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linha**: 69; **trecho**: Wi-Fi AutoDiscover inventaria privadamente redes observáveis pelo SO, sem extração de senhas salvas ou cadastro de terceiros como públicos.

- **arquivo**: docs/01-arquitetura/wifi-autodiscover-autoshare.md; **secao**: 3.1–3.3; **linhas**: 65-82; **trecho**: Inventário privado; detecção não publica; sincronização usa wifi.inventory.private; sem varredura contínua.

- **arquivo**: docs/03-integracoes/wifi-android-ios-roteadores.md; **secao**: Android/iOS; **linhas**: 5-19; **trecho**: Android sujeito a permissão e throttling; iOS sem API pública geral de scan/lista de senhas.

### aplicabilidade

**Personal**: Aplica-se ao inventário local.

**Teams**: Aplica-se ao dispositivo inscrito sob política organizacional, mantendo o inventário privado.

**Node**: Scan pessoal não aplicável; inventário de SSIDs geridos só via Router Agent autorizado.

**Platform SDK**: Estados/escopos aplicam-se; scan local não aplicável a SDK sem API de rádio.

**plataformas**: **Android**: Scan permitido por API/permissão/frequência; pode retornar dados defasados.

**iOS**: Sem scan geral; usar somente rede atual/configurações próprias segundo API permitida.

**macOS**: Core WLAN é candidato; sandbox, versão e homologação em aberto.

**Linux Windows desktop**: APIs futuras não especificadas; não chamar de não aplicável.

**servidor Linux**: Scan do cliente pessoal não aplicável; serviço trata inventário privado com consentimento.

**Web**: Scan local não aplicável conforme APIs consultadas; sincronização/gestão remota continuam aplicáveis.

### decisoes pendentes

- Consentimento e política de sync privado

- Frescor/expiração e classe de retenção

- Versões e APIs suportadas por plataforma

- Suporte macOS sandboxed

### dependencias

- Contrato de capabilities/estados do cliente

- Política comum de retenção e revogação

- Revisão de plataforma Apple e integração Android

### criterio fechamento documental

Definir origem, frescor, consentimento, persistência/sync, estados de ausência/erro e plataformas suportadas. Manter scan e autoridade como conceitos separados e ligar os futuros casos aos requisitos. Não declarar SPEC ou teste executado.

### spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite comportamental proposto; dependências e parâmetros abertos

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-036

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jouni.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

AP privado sem share/grants ativos; ator autenticado; evidência de autoridade e delegação, escopo de AP/rede/tenant, validade e consentimento identificados.

### Estímulo

Ator ativa AutoShare e escolhe visibilidade/duração/política; publicação exata é solicitada separadamente com opt-in próprio.

### Esperado

Com autoridade e consentimento válidos, ativar somente a política dentro do escopo e registrar auditoria. Separar share ativo, grant e credencial. Herança para SSID novo limita-se a política explicitamente autorizada pelo controlador.

### Invariante

AutoShare enabled é intenção; scan/SSID/BSSID/senha/conexão não provam controle. Prova de um AP não autoriza outro.

### Negativo

Sem prova/consentimento, escopo incompatível, delegação expirada ou controlador revogado: negar publicação/grant, manter estado privado e registrar motivo sem expor prova sensível.

### Degradação e recuperação

Controlador offline resulta unknown/pendente, não prova confirmada. Revogação bloqueia novos grants; PSK já entregue permanece revocation_pending_router até confirmação do controlador/rotação. Na recuperação, revalidar autoridade, versão e consentimento antes de retomar.

### decisao

Definir evidências aceitas sem controlador, contestação/suspensão, duração, jurisdição/retenção e revogação sob partição. Escolher entre exigir controlador sempre ou manter caminhos administrativos homologados/convite assinado. Recomendo manter caminhos alternativos com prova, escopo e revogação próprios, sem exigir hardware universalmente.

### principal documental

**nome**: Jouni Malinen

**estado**: confirmado_para_esta_complementacao; proposta da matriz ainda aguarda conciliacao de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linha**: 70; **trecho**: AutoShare apenas AP comprovadamente administrado, com consentimento, políticas, validade e revogação.

- **arquivo**: docs/01-arquitetura/wifi-autodiscover-autoshare.md; **secao**: 4.1–4.2; **linhas**: 84-99; **trecho**: SSIDs/BSSID/senha/proximidade não provam autoridade; sem prova suficiente, cadastro somente privado.

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: 2 e 4; **linhas**: 12-18, 37-41; **trecho**: Publicação só após owner_verified e consentimento; PSK entregue fica pending até ação confirmada no controlador.

### aplicabilidade

**Personal**: Compartilhar somente AP administrado pelo ator com prova verificável.

**Teams**: Aplicar função, tenant e delegação por organização.

**Node**: Executar política somente dentro da autoridade do AP/região; réplica offline não concede grants obsoletos.

**Platform SDK**: Preservar escopos, consentimentos e estados sem conferir autoridade por si só.

**plataformas**: **Android iOS**: Clientes apresentam consentimento e capacidade do SO; não provam autoridade do controlador.

**macOS Windows Linux desktop**: Administração autenticada/manual pode ser aplicável; observar rede não prova autoridade e UX/API permanecem pendentes.

**Router Agent**: Opcional, somente para AP pareado e autorizado.

**servidor Linux**: Política/provisionamento conforme autoridade aprovada.

**Web**: Fluxo administrativo remoto aplicável; scan local não é pré-condição.

### decisoes pendentes

- Prova de administração/delegação

- Contestação e efeito cautelar

- Validade de grants/credenciais

- Jurisdição/finalidade/retenção

- Semântica de revogação em partição e recibo de controlador

### dependencias

- Identidade e delegação

- Router Agent/controladores homologados quando usados

- Política de consentimento e publicação exata

- Sync/cache/replicação e critério de recuperação

### criterio fechamento documental

Política e contrato distinguem prova, escopo, consentimento, share/grant/credencial e confirmação de revogação; cobrem prova inválida, delegação vencida, controlador offline e recuperação. Decisões de autoridade/contestação registradas pelo usuário.

### spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite comportamental proposto; política de autoridade e revogação aberta

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-037

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jouni.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Grant vigente/escopado, credencial válida, capacidade do SO conhecida e rede anterior/conectividade/permissões registradas. Sessões PTT/SOS ativas são conhecidas para evitar troca não autorizada.

### Estímulo

Usuário solicita conexão ou cliente apresenta sugestão/configuração autorizada. Capturar separadamente retorno do SO, associação observada, autenticação do controlador e verificação de serviço.

### Esperado

Registrar requested, decisão/consentimento do SO, associação como estado/evento do cliente, ap_authenticated somente por controlador autenticado, internet_verified somente após método contratual. Cada transição conserva origem, hora e evidência.

### Invariante

Pedido não implica associação; associação não implica autenticação; autenticação não implica Internet. Provisionamento não prova uso. SO pode escolher outra rede.

### Negativo

Recusa/timeout, fora de alcance, credencial vencida, captive portal, rede local sem Internet, API ausente ou controlador offline não resulta em estado posterior falso.

### Degradação e recuperação

Preservar rede anterior se permitido e oferecer fluxo manual quando automação faltar. Após recuperação, obter nova evidência; não reescrever falha anterior como sucesso.

### decisao

Definir estado canônico do SO, método/fonte/frescor de internet_verified, captive portal, mudança de rede sob PTT/Guardian/SOS, versões mínimas e fallback manual. Valores permanecem sem decisão.

### principal documental

**nome**: Jouni Malinen

**estado**: confirmado_para_esta_complementacao; proposta da matriz ainda aguarda conciliacao de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linha**: 71; **trecho**: AutoConnect solicita associação pelo SO conforme plataforma, distinguindo sugestão, associação, autenticação e internet verificada.

- **arquivo**: docs/03-integracoes/wifi-android-ios-roteadores.md; **secao**: Android, iOS e matriz resumida; **linhas**: 5-40; **trecho**: Android escolhe entre sugestões; iOS exige aprovação de configuração; evidência de cliente não confirma autenticação do AP.

- **arquivo**: docs/07-contratos/wifi-autodiscover-autoshare-api.md; **secao**: 4. Solicitação de conexão; **linhas**: 69-88; **trecho**: Estados separados e regra para não inferir requested, associated e internet_verified.

### aplicabilidade

**Personal**: Cliente solicita associação própria.

**Teams**: Política pode ser gerida pela organização; decisão de associação continua com o SO conforme capacidade.

**Node**: Não associa o cliente; Router Agent provisiona/controla somente AP autorizado.

**Platform SDK**: Expõe capabilities e eventos sem prometer APIs locais.

**plataformas**: **Android**: Suggestion não garante conexão; escolha pertence ao SO.

**iOS**: Configuração requer aprovação; sem scan geral.

**macOS**: Core WLAN candidato, sandbox/versão/retorno a homologar.

**Windows Linux desktop**: Capacidade/UX ainda não especificadas.

**servidor Linux**: Associação de dispositivo cliente não aplicável.

**Web**: Associação local não aplicável com as fontes atuais; estados remotos de sessão permanecem aplicáveis.

### decisoes pendentes

- Nomes/estados de decisão do SO

- Definição e frescor de internet_verified

- Mudança durante PTT/Guardian/SOS

- Captive portal

- Versões mínimas e fallback manual

### dependencias

- APIs de cada SO

- Grant/provisionamento

- Verificação de serviço e conectividade

- Política de continuidade de PTT/SOS

### criterio fechamento documental

Contrato define máquina de estados com fonte de evidência, erros, unknown, portal cativo, recuperação e plataformas; não afirma associação ou Internet sem sinal próprio.

### spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite comportamental proposto; semântica e parâmetros abertos

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-038

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-paul.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

AP físico está identificado dentro do tenant. Administrador demonstra controle ou delegação verificável. Coordenada disponível tem método/fonte, qualidade/precisão quando oferecida e instante observado. A autorização para compartilhar rede não equivale ao opt-in de posição exata.

### Estímulo

Cadastrar, corrigir, verificar ou remover posição de AP; consultar histórico como administrador autorizado; tentar cadastro a partir de scan, SSID duplicado ou BSSID observado.

### Esperado

Sistema associa ponto à entidade AP físico, não ao SSID. Persiste coordenada disponível sem criar precisão, fonte, qualidade, registrador, compartilhador quando aplicável, tempos e histórico. Sem coordenada confiável, mantém ausência explícita. Mapa exato continua sujeito ao requisito separado RF-039.

### Invariante

SSID/BSSID isolado não prova controle nem identifica sozinho o AP. Observação de visitante não substitui posição oficial. Registro não expõe identidade civil, visitante, BSSID bruto, credenciais ou posição privada sem autorização.

### Negativo

Sem prova de administração/delegação não se cria nem altera posição oficial. Scan não publica AP. Coordenada ausente não é inventada. Consulta exata sem opt-in específico não retorna ponto exato.

### Degradação e recuperação

Autoridade/controlador offline resulta em unknown ou pending, não em controle confirmado. Operação não aumenta confiança nem publica atualização sem revalidação. Ao reconectar, versão atual de autorização/consentimento prevalece; cache/réplica antigos não restauram posição ou publicação revogada. Propagação e TTL aguardam decisão.

### decisions open

- Provas aceitas e contestação/delegação

- Método de captura e regra para coordenada antiga

- TTL de mapa/cache, retenção e proteção de residência

### principal documental

**owner**: Paul Ramsey

**status**: proposto_por_especialidade_e_confirmado_pelo_contribuidor

**basis**: A atribuição cobre a localização geográfica de AP. Jouni Malinen é interface para prova de administração/provisionamento e Michael Stonebraker para persistência; a responsabilidade documental geo permanece com Paul.

### evidencias

- **source**: docs/00-produto/requisitos.md:72-73; **literal**: AP físico registra coordenadas exatas disponíveis, qualidade, origem, pessoa que cadastrou/compartilhou, data e histórico de mudanças. RF-039 separa publicação exata no mapa.

- **source**: docs/02-dominio/wifi-modelo-dados.md:23-34,154-164,224-260; **literal**: AP físico é entidade geográfica; SSID pode repetir; BSSID não prova posse; coordenada exata somente quando disponível; publicação é opt-in separado.

- **source**: docs/04-seguranca/wifi-compartilhamento-privacidade.md:14-27,50-58; **literal**: Administração exige competência/delegação verificável; mapa exato exige opt-in; coordenada e dados pessoais ficam protegidos.

- **source**: docs/05-operacao/testes-wifi.md:12-22; **literal**: Cenários planejados de autoridade, posição/proveniência e projeção sem coordenada exata não autorizada.

### aplicabilidade

**products**: - Teams e administradores de infraestrutura

- Node quando mantém registro autorizado

- Platform/API para leitura/escrita escopadas

**platforms**: - iOS SwiftUI

- Android

- macOS universal SwiftUI

- Windows

- Linux desktop

- Rust/Linux serviço futuro

**not applicable**: O registro cobre AP físico, não posição pessoal, avistamento BLE nem toda conta Personal. Uma interface sem administração/mapa de AP pode declarar o fluxo de UI como não aplicável; a persistência/API do escopo integral continua necessária.

**future platform rule**: Aplicar nos clientes/plataformas que ofereçam gerenciamento ou mapa de AP e preservar plataformas futuras do escopo.

### decisoes pendentes

**alternativas**: - Prova por controlador pareado

- Delegação verificável com prazo e titular

- Cadastro manual revisado sem tratar como prova automática

**recomendacao**: Armazenar fonte/qualidade e separar posição privada de publicação; não inferir estado verificado apenas porque há coordenada.

**consequencia**: Sem regra de autoridade, idade e invalidação, o registro não pode afirmar propriedade atual nem consistência da projeção após revogação.

### dependencias

- Jouni Malinen: autoridade e provisionamento de AP

- Michael Stonebraker: modelo/persistência e isolamento por tenant

- Bruce Schneier: privacidade e retenção

- RF-039: opt-in de publicação exata

- Decisão do usuário sobre prova, contestação e retenção

### criterio fechamento documental

Contrato define entidade AP, prova/delegação, campos de proveniência, tratamento de coordenada ausente/antiga, histórico e invalidação; exemplos cobrem SSID duplicado, BSSID sem prova, visitante, atualização e revogação; alinha RF-038/RF-039, modelo, política, testes e ADR-049/054. A publicação exata segue o aceite separado de RF-039.

### estado

criterio_proposto; prova, TTL e retenção pendentes; implementação não verificada

### spec id

NOT_ASSIGNED

### test id

NOT_ASSIGNED

### observed execution

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-039

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

AP com origem/incerteza, tenant e administrador verificados; precisão inicial coarse/hidden; permissão de acesso Wi-Fi separada da permissão de publicação exata.

### Estímulo

Ativar exact_opt_in, consultar mapa em audiência pública/privada, alterar role/tenant, revogar, ou apresentar AP residencial/contestação de autoridade.

### Esperado

Exato só após confirmação específica do administrador comprovado; resposta divulga melhor coordenada e incerteza à audiência autorizada e omite BSSID, visitante, credencial e identidade administrativa; revogação bloqueia nova resposta e invalida cache controlado.

### Invariante

Conexão, senha ou presença não prova autoridade; opt-in geográfico não autoriza dados de visitante nem localização pessoal; desconhecido não é resolvido por inferência.

### Negativo

Falha de prova, tenant mismatch, grant expirado, autoridade contestada ou autorizador indisponível nega precisão; não servir coordenada bruta/cache antigo como fallback.

### Degradação e recuperação

Mostrar unknown/pendente por destino; após reconexão revalidar grant e revogação antes de servir preciso. Ausência de controlador não prova remoção.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos de Wi-Fi; **linhas**: 73; **trecho**: Mapa publica AP exato somente mediante opt-in específico do administrador comprovado; usuário/visitante não é exposto; **estado origem**: APROVADO como direção

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: Prova de autoridade e publicação; **linhas**: 14; **trecho**: Ter a senha de uma rede salva, observar o BSSID ou conseguir conectá-la não prova direito de compartilhá-la.

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: Localização exata dos APs; **linhas**: 24; **trecho**: O mapa só retorna o ponto preciso se houver autorização explícita de divulgação; residências e redes privadas começam como `coarse`/`hidden`.

- **arquivo**: docs/07-contratos/wifi-autodiscover-autoshare-api.md; **secao**: Criar AP e consentir publicação exata; **linhas**: 67; **trecho**: A API não deve aceitar apenas esse booleano como prova suficiente: ela também valida titularidade/controlador, autenticação recente, escopo e trilha de auditoria.

### aplicabilidade

**produtos**: Papéis de administrador/usuário de AP em Personal/Teams/Node/Platform conforme recurso Wi-Fi habilitado

**plataformas**: Servidor e interfaces macOS, iOS, Windows, Linux desktop, Android e web; UI não pode restaurar campos omitidos pelo serviço

**nao aplicavel**: Sem AP/controlador cadastrado, cenário de publicação exata é N/A por ausência do domínio; autorização de API permanece coberta. Cliente sem mapa é N/A apenas para apresentação.

### decisoes pendentes

- **tema**: prova/delegação/contestação e audiência; **alternativas**: - controlador pareado; - delegação verificável; - verificação manual aprovada; **recomendacao**: Oculto/coarse até prova e opt-in específicos; **consequencia**: Método manual aumenta custo e risco de fraude; controlador depende de integração compatível

- **tema**: frescor do cache e cópias regionais; **alternativas**: - bloqueio central com invalidação quando chega; - limite offline aprovado por política; **recomendacao**: Não servir preciso quando status desconhecido; **consequencia**: Disponibilidade degradada sem autoridade atual; duração não definida

### dependencias

- Jouni Malinen: prova/admin e provisionamento

- Paul Ramsey: coordenada e qualidade

- Martin Kleppmann: propagação de revogação

- Michael Stonebraker: dados/cópias

### criterio fechamento documental

Modelo, API, audiência, campos omitidos, auditoria e estados de cache/revogação conciliados; owner e decisões registrados; casos residenciais/visitantes/tenant e recuperação definidos.

### estado

proposta comportamental; implementação e remoção de caches não verificados

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-040

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jouni.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

AP/share/grant e tenant são identificados; fonte de evento/controlador autenticada ou explicitamente ausente; papel de consulta conhecido; medição disponível ou desconhecida.

### Estímulo

App pede conexão; provisionador emite credencial; SO informa associação; controlador emite autenticação/sessão/contadores; ator consulta sob escopo.

### Esperado

App cria requested, nunca authenticated. Eventos carregam fonte, observado_em, nível e versão do coletor. Só controlador autenticado gera controller_attested; consumo não medido permanece nulo/desconhecido. Participante vê próprias sessões; admin vê mínimo dos próprios APs; operador requer função/auditoria; público não vê visitantes/sessões. Repetição não duplica sessão/consumo.

### Invariante

Não coletar URLs, navegação, mensagens, senha, token ou GNSS de visitante. Sem cruzamento de tenant/AP. Alegação do cliente não se eleva sozinha a atestado do controlador.

### Negativo

Sem telemetria, não afirmar volume/duração ou autenticação; consulta fora do escopo é negada; evidência repetida/out-of-order não duplica nem aumenta confiança.

### Degradação e recuperação

Controlador offline deixa autenticação, encerramento e revogação desconhecidos/pendentes. Ao reconectar, reconciliar evento com idempotência, versão e origem. Reset de contador/partição conserva incerteza e não inventa consumo.

### decisao

Definir classes, jurisdição, prazo, agregação, identificadores, correção de evento, identidade de controlador e regra/heartbeat para encerramento inferido. Não criar TTL ou limites não aprovados.

### principal documental

**nome**: Jouni Malinen

**estado**: confirmado_para_esta_complementacao; proposta da matriz ainda aguarda conciliacao de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos funcionais; **linha**: 74; **trecho**: Auditoria de acesso Wi-Fi por usuário/GMC, AP, grant, sessão, duração e volume quando medido pelo controlador; sem histórico de navegação.

- **arquivo**: docs/02-dominio/wifi-registro-acessos.md; **secao**: 1–3 e 6; **linhas**: 9-41, 78-84; **trecho**: Separar fontes de evento; consumo ausente fica null; consultas por papel; app não prova autenticação.

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: 4. Retenção e revogação; **linhas**: 37-41; **trecho**: Retenção por classe sem TTL escolhido; classe sem política não está pronta para persistência/replicação/exportação.

### aplicabilidade

**Personal**: Consulta a própria atividade.

**Teams**: Admin consulta apenas APs de seu tenant/papel.

**Node**: Registra eventos recebidos de controlador autenticado dentro de sua autoridade.

**Platform SDK**: Preserva origem, nível de evidência e unknown; não cria prova.

**plataformas**: **Android iOS**: Podem informar associação como alegação de cliente, não autenticação do AP.

**macOS Windows Linux desktop**: Não atestam autenticação do controlador por si sós; podem consultar dados autorizados.

**Router Agent RADIUS**: Fonte preferida de sessão/autenticação/consumo quando homologada e compatível.

**servidor Linux**: Persistência/processamento sujeito a política e escopo.

**Web**: Consulta autorizada aplicável; sem privilégio adicional.

### decisoes pendentes

- Retenção por classe/jurisdição/finalidade

- Identificadores mínimos

- Agregação e reset de contadores

- Correção/contestação de evento

- Autenticação/rotação do controlador

- Heartbeat/encerramento inferido

- Consistência offline e recibos de exclusão

### dependencias

- Política de privacidade/retenção

- Contrato de identidade de controlador

- Isolamento tenant/RLS e auditoria de consultas

- Sync/eventos e idempotência

- Compatibilidade de roteadores

### criterio fechamento documental

Contrato liga cada dado e evento à fonte/evidência e papel, declara null/unknown, retenção, dedupe, offline/replay e limites de privacidade. Casos futuros distinguem alegação, atestação, medição e consulta autorizada.

### spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite comportamental proposto; retenção e evidência operacional em aberto

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-041

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-graydon.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Toda solicitação identifica principal autenticado, tenant/ACL, scope mínimo, finalidade/consentimento, recurso e autoridade; origem app/usuário/controlador é distinguível. Estado inicial separa observação privada, AP, mapa, grant, pedido de conexão, sessão e evidência de uso. Segredos não aparecem em inventário/mapa.

### Estímulo

Exercitar famílias de rotas propostas: capacidades/inventário, claim/pair, AP, share/publicação separada, mapa, grant/enrollment, conexão, sessões/auditoria e revogação; repetir mutação com chave e fingerprint iguais e divergentes; receber alegação de sessão do app e evento de controlador atestado.

### Esperado

Rota sem tenant/ACL/scope/consentimento/prova/autoridade vigentes nega sem efeito. Observação de rede continua privada. Mutação idempotente retorna mesma operação para retry igual e conflito para chave reutilizada com pedido divergente. Cadastrar AP não publica coordenada exata: exige prova e opt-in específico; projeções coarse/hidden e mapa autorizado não expõem BSSID bruto, GMI, visitantes ou segredos. Alegação do cliente permanece client_claim; apenas prova válida pode chegar a controller_attested. requested, associated e internet_verified permanecem estados separados. Revogação reporta efeitos observados por destino; não afirma descarte/rotação não confirmados.

### Invariante

Descoberta não prova propriedade; auto_share não é consentimento de terceiros; principal/tenant vêm da autenticação; localização do AP não é localização pessoal; API não cria capacidade ausente no SO/controlador. Idempotency-Key não autentica. Não coletar histórico de navegação. Controlador offline/revogação pendente não equivale a sucesso; caches/replay seguem C08/C10.

### Negativo

Rede terceira descoberta, claim sem prova, tenant alheio, coordenada exata sem opt-in vigente, booleano isolado como consentimento, consulta pública a sessão/visitante/GMI/IP/MAC/segredo, origem falsificada como controller_attested, key divergente, enumeração de sessões alheias ou enrollment com grant expirado/revogado devem ser negados/redigidos sem efeito, exposição ou promoção de estado.

### Degradação e recuperação

Permissão de SO ausente, WIFI_OS_UNSUPPORTED, controlador desconectado ou AP inalcançável resultam em estado/capacidade degradados e alternativa manual, nunca alegação de conexão/internet. Revogação offline mantém destino pending/unknown e bloqueia nova emissão/publicação sensível segundo política; retry mantém key e reconcilia resultado incerto. Após recuperação, tombstones/revogação são aplicados antes de reabrir grants/mapa. PSK já entregue não é declarada revogada sem rotação/confirm observados.

### principal documental

**nome**: Graydon Hoare

**estado**: confirmado como adequado para contrato API/GMP, escopo e idempotência; limite funcional explicitado

### evidencias

- **fonte**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linha**: 139; **trecho**: RF-041 | 00-produto/requisitos.md:75 | ESPECIFICAR | 07-contratos/wifi-autodiscover-autoshare-api.md | Graydon Hoare — proposto por especialidade, confirmação por requisito pendente

- **fonte**: docs/00-produto/requisitos.md; **linha**: 75; **trecho**: RF-041 — APIs/GMP de inventário, APs, compartilhamento, grants, provisionamento, sessões e mapa com escopo e idempotência — ESPECIFICAR

- **fonte**: docs/07-contratos/wifi-autodiscover-autoshare-api.md; **secoes**: - 1-7; **trecho**: rotas, escopos mínimos, consentimento, estados, idempotência e eventos de controlador propostos

- **fonte**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secoes**: - 1-6; **trecho**: inventário privado, prova de autoridade, consentimento exato e privacidade

- **fonte**: docs/07-contratos/gmp-api-v1.md; **secoes**: - 2; - 6; - 7; - 8; **trecho**: tipo, autorização, erro, HTTP e idempotência

- **fonte**: docs/06-planejamento/decisoes.md; **adrs**: - ADR-048; - ADR-049; - ADR-050; - ADR-051; **trecho**: preferências, opt-in de localização, evidência de controlador e limites de SO

### aplicabilidade

**escopo**: API/GMP compartilhada entre Personal/Teams/Node e plataformas. Capacidades variam por SO/controlador; primeira sequência iOS SwiftUI, macOS universal SwiftUI e servidor Rust Linux; Android/Kotlin e Windows/Linux desktop seguem preservados.

**na**: False

**justificativa na**: Não aplicável: limitação de capacidade deve aparecer como indisponível por plataforma, enquanto rotas/serviços compatíveis permanecem no escopo; AP/controlador homologado ainda não escolhido.

### decisoes pendentes

- **tema**: Rotas/schemas, scopes, ownership proof e consentimento/mapa; **alternativas**: - Ratificar contratos por domínio e matriz de privacidade; - Bloquear publicação exata e novos grants até ratificação; **recomendacao**: Fail closed em disclosure/grant sem prova e consentimento efetivos; **consequencia**: Preserva autoridade e privacidade, com menor disponibilidade enquanto integrações não forem homologadas.

- **tema**: Revogação de PSK, caches, tombstones, idempotência e retenção; **alternativas**: - Credenciais individualizadas revogáveis quando suportadas; - Credencial compartilhada sob autoridade do administrador, rotação e condições do fabricante; **recomendacao**: Não declarar revogabilidade sem confirmação do controlador; ratificar C08/C10; **consequencia**: Define reconciliação e prazo de retenção; hardware/quotas/jurisdição seguem desconhecidos.

- **tema**: Plataformas, OS APIs, controladores e jurisdições; **alternativas**: - Homologar combinações com evidência oficial; - Marcar capacidades indisponíveis até homologação; **recomendacao**: Documentar matriz de capacidades por plataforma sem exclusão de escopo; **consequencia**: Afeta feature availability e rollout, sem escolha técnica aprovada antecipadamente.

### dependencias

- C09 identidade/scopes/subject/frescor

- C10 eventos/replay/idempotência

- C08 retenção/cópias

- C06 autoridade

- Michael modelo/integridade de dados

- Chet/Donny capacidades de plataforma

- decisões de produto/jurisdição e homologação de controlador/AP

### criterio fechamento documental

Cada rota define request/response, principal/scope/finalidade, prova de ownership, namespace idempotente, erro/efeito parcial e estados OS/controlador; matriz privacidade liga inventário/AP/mapa/grant/sessão a consentimento e cópias; capacidades Android/iOS/controlador e fallback têm fonte; decisões de produto são registradas. Sem isso RF-041 permanece ESPECIFICAR.

### estado

**planejamento aceite**: proposto

**SPEC**: NOT_ASSIGNED

**teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

**requisito origem**: ESPECIFICAR

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-042

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-donny.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Lea Verou. Subescopos e deltas vinculados: [Conciliação lea](#conciliacao-lea), [Conciliação chet](#conciliacao-chet). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Atividade SOS, Guardian ou PTT iniciada/autorizada; evento de domínio autenticado; elegibilidade e permissões avaliadas; resumo redigido. A atividade de domínio não depende da superfície de Live Presence.

### Estímulo

Iniciar cada atividade e combinações PTT+Guardian e SOS+Guardian; bloquear tela; atualizar; perder rede; encerrar/expirar; dispensar projeção; retomar app após suspensão/terminação.

### Esperado

Superfície elegível mostra apenas categoria, resumo e estado correspondentes ao último evento autenticado; SOS tem prioridade visual e outras atividades seguem acessíveis no app. Estados locais, técnicos e humanos permanecem distintos. Superfície inelegível/dispensada usa fallback no app/notificação e não inventa status.

### Invariante

Visibilidade ou remoção da Activity/notification não muda o estado de domínio. Nenhuma tela bloqueada mostra coordenadas exatas, identidade civil, credenciais, SSID/BSSID ou auditoria. Rota disponível, push aceito, ACK técnico ou estado active não significam atendimento humano.

### Negativo

Sem permissão/elegibilidade, evento atual/autenticado ou enquanto o evento está revogado, expirado ou encerrado: não iniciar ou recriar projeção; mostrar fallback e estado desconhecido/antigo conforme contrato. Mensagem avulsa não se torna atividade persistente.

### Degradação e recuperação

Queda de rede ou atualização ausente marca projeção stale/desatualizada e preserva a última evidência. Após retorno, reconciliar evento autenticado; atualização recuperada altera a projeção sem apagar estado de domínio. A duração máxima documentada do ActivityKit exige encerrar/reconciliar apenas a projeção, sem terminar SOS/Guardian/PTT.

### limiares abertos

- prazo de stale por atividade

- frescor e atraso aceitáveis

- limites de resumo

- matriz de elegibilidade por versão/modelo

### evidencia futura

Versão do SO/modelo/permissão, estado inicial, evento de origem redigido, estímulo, estados esperados/observados, fallback e captura sem dados privados.

### principal documental

**proposto na matriz**: Donny Wals

**confirmacao**: adequado_para_iOS; insuficiente_como_unico_principal_multiplataforma

**recomendacao**: - **responsabilidade**: iOS, SwiftUI e ActivityKit; **principal**: Donny Wals

- **responsabilidade**: Android, Kotlin e Live Updates; **principal recomendado**: Chet Haase

- **responsabilidade**: jornada, acessibilidade e fallback de UX; **principal recomendado**: Lea Verou

**conflito**: A matriz dá a Donny o requisito inteiro, mas o requisito e contrato abrangem iOS e Android; a distribuição e CONTEXTO-E-PAPEIS reservam Android a Chet e Apple a Donny. Proposta de subresponsabilidades, sem alterar atribuição nesta entrega.

### evidencias

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linhas**: 81-86, 140, 148-150; **trecho**: RF-042; Donny Wals — proposto por especialidade, confirmação por requisito pendente; aceite ainda precisa de estado inicial, estímulo e resultado.

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos — aplicativo, Wi-Fi comunitário, mapa e presença dinâmica; **linhas**: 76; **trecho**: Live Presence no iOS/Android com SOS/Guardian/PTT e estados reais, sem exposição de dados precisos na tela bloqueada.

- **arquivo**: docs/01-arquitetura/live-presence.md; **secao**: Objetivo / prioridade / ciclo de vida / segurança; **linhas**: 7-21, 23-29, 33-38, 47-51; **trecho**: Atividades elegíveis, destaque SOS, estado stale e proibição de dados sensíveis no lock screen.

- **arquivo**: docs/07-contratos/live-presence.md; **secao**: Políticas de projeção; **linhas**: 26-33; **trecho**: SOS state=active não implica socorro confirmado; evento não força exibição pelo SO.

- **arquivo**: docs/05-operacao/testes-app-live-presence.md; **secao**: iOS / Android; **linhas**: 14-24; **trecho**: A matriz pede elegibilidade, fallback, privacidade, fases SOS, reboot/kill/expiração, dispensa e bateria; não constitui resultado.

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Rastreabilidade adicional (aplicativo e Wi-Fi); **linhas**: 29-35; **trecho**: Associação histórica explícita de RF-042 à ADR-052.

- **arquivo**: Shared/GodMesh/CONTEXTO-E-PAPEIS.md; **secao**: Responsabilidades / cobertura transversal; **linhas**: 35, 73-74, 81; **trecho**: Chet cobre Android; Donny Apple; Lea lidera UX.

- **arquivo**: Apple Developer Documentation; **url**: https://developer.apple.com/documentation/activitykit/displaying-live-data-with-live-activities; **data consulta**: 2026-11-15; **versao**: página rolling; target não definido; **trecho**: ActivityKit tem estado stale e limite documentado de duração; a projeção não tem acesso direto a rede/localização.

- **arquivo**: Android Developers; **url**: https://developer.android.com/develop/ui/views/notifications/live-update; **data consulta**: 2026-11-15; **versao**: documentação corrente; compatibilidade OEM pode variar; **trecho**: Live Updates destinam-se a atividades em andamento, iniciadas pelo usuário e sensíveis ao tempo; usuário pode dispensar/rebaixar.

### aplicabilidade

**aplica**: - iOS/SwiftUI/ActivityKit

- Android/Kotlin/Live Updates ou notificação elegível

- interfaces móveis de SOS/Guardian/PTT

**nao aplica diretamente**: - **plataforma**: servidor Linux; **motivo**: produz estado de domínio/eventos; não renderiza Live Activity móvel

- **plataforma**: desktop macOS; **motivo**: o requisito e contrato delimitam Live Presence ao iOS/Android; o cliente pode consultar o estado no app, sem esta superfície móvel

**condicao**: Se a plataforma não suportar ou a pessoa dispensar a superfície, manter estado no app e usar fallback suportado; não anunciar como falha de domínio.

### decisoes pendentes

- **decisao**: quais atividades são elegíveis e como lidar com início remoto; **alternativas**: - apenas início local explícito; - convite/remoto após opt-in anterior; **recomendacao**: opt-in anterior e início explícito autorizado; **consequencia**: reduz superfície acionada por terceiro, mas restringe automação; regras de cada SO continuam aplicáveis

- **decisao**: prazo stale/frescor por tipo de atividade; **alternativas**: - prazo único; - prazo por SOS/Guardian/PTT; **recomendacao**: prazo por tipo, definido por owner do domínio e plataforma; **consequencia**: um prazo único pode representar mal cadências diferentes; nenhum valor está aprovado

- **decisao**: schema e reconciliação da projeção; **alternativas**: - contrato Live Presence reduzido com derivação documentada; - projeção contém todos os campos necessários; **recomendacao**: fixar uma fonte canônica e mapear campos/transições; **consequencia**: depende de contrato GMP/eventos e itens A31-001/002 e PI-donny-003

### dependencias

- ADR-052 (associação histórica explícita)

- ADR-053 para prioridade e energia

- contrato live-presence

- contrato SOS/ACK

- identidade e autorização

- confirmação do owner Android

- frescor/expiração e schema

### criterio fechamento documental

Por plataforma e atividade, registrar elegibilidade, origem autoritativa, campos visíveis, lifecycle, stale, expiração, fallback, recuperação, negativos e owner; associar a critério futuro sem criar IDs de SPEC/teste. Decisões e limiares não resolvidos ficam marcados como abertos.

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

PARCIAL — aceitação documental proposta; compatibilidade, thresholds e responsabilidade Android ainda pendentes

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RF-043

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-alex.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Cliente com serviços autorizados, tarefa ou fila local e estado de energia/rede exposto pelo SO quando disponível; avaliar apenas trabalho do GodMesh.

### Estímulo

Ativar em conjunto atividades SOS, PTT e Guardian; impor restrição de bateria/rede ou suspensão de background que a plataforma informe.

### Esperado

Conservar estados simultâneos e ordenar tarefas conforme P0 SOS e fila mínima, P1 comunicação ativa/check-ins Guardian, P2 mensagens/sync autorizado, P3 descoberta Wi-Fi/telemetria voluntária/manutenção. Sob restrição, reduzir primeiro tarefas próprias não essenciais; informar tarefa indisponível ou ação manual necessária; não afirmar execução sem evidência.

### Invariante

Não controlar apps de terceiros; não marcar pendência como entregue; prioridade SOS não apaga Guardian/PTT; não prometer disponibilidade ou entrega de emergência.

### Negativo

Sem permissão ou API elegível de background, não iniciar tarefa restrita e reportar permission_required ou unavailable conforme a capacidade conhecida. Sem confirmação de gravação/envio, não reportar fila durável/entrega.

### Degradação e recuperação

Com restrição ou suspensão, adiar trabalho próprio de prioridade inferior e manter degradação visível. Quando permitido, retomar somente tarefas ainda autorizadas, sem duplicação e reconciliando pelo contrato de eventos/sync. Limiares e atrasos não definidos.

### limiares

Não especificados; dependem de metas de energia, disponibilidade e atraso aprovadas por plataforma e produto.

### principal documental

**nome**: Alex Crichton

**estado**: proposto na matriz; confirmado por Alex para o comportamento do núcleo cliente

**conflitos ou interfaces**: - Chet Haase: Android e limites de energia/background

- Donny Wals: Apple e limites de energia/background

- Lea Verou: estado e UX

- Brendan Gregg: método/evidência futura

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: RF-043; **linha**: 77; **trecho**: RF-043 | Operação prioritária e energia adaptativa preservam comunicação essencial sem controlar apps de terceiros | APROVADO | `01-arquitetura/aplicativo-operacao-normal.md`

- **arquivo**: docs/01-arquitetura/aplicativo-operacao-normal.md; **secao**: 6. Priorização e energia; **linhas**: 68-70; **trecho**: `P0`: incidente SOS ativo, estado e fila mínima de encaminhamento. `P1`: comunicação em andamento e check-ins Guardian. `P2`: mensagens e sincronização autorizadas. `P3`: descoberta de Wi-Fi, telemetria voluntária e tarefas de manutenção.

- **arquivo**: docs/01-arquitetura/aplicativo-operacao-normal.md; **secao**: 8. Critérios de aceite; **linha**: 83; **trecho**: A UI abre offline mostrando dados locais e estado `Offline`; não marca mensagens pendentes como entregues.

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK, quando o consumidor executar tarefas do cliente

**plataformas**: - macOS universal

- iOS

- Android

- Windows

- Linux desktop

- web quando houver tarefa equivalente

**nao aplicavel**: Política de bateria móvel não se aplica ao serviço de servidor; decisões de background/energia devem ser justificadas por plataforma e capacidade do SO.

### decisoes pendentes

- **decisao**: Definir comunicação essencial por produto, empates/concorrência e tarefas que podem ser adiadas.; **alternativas**: - Manter a ordem qualitativa atual e derivar limites quantitativos por plataforma; - Definir políticas distintas por perfil de produto, com invariantes comuns; **recomendacao**: Preservar ordem qualitativa já registrada e adiar apenas trabalho próprio menos prioritário.; **consequencia**: Sem definição, não há aceite quantitativo ou garantia de continuidade mensurável.

- **decisao**: Definir métricas e limites de energia/background por plataforma.; **alternativas**: - Metas por jornada e plataforma; - Metas por serviço/prioridade e plataforma; **recomendacao**: Aprovar após confirmar APIs e condições observáveis de cada plataforma.; **consequencia**: Sem limite aprovado, comportamento qualitativo pode ser documentado, mas não se declara desempenho validado.

### dependencias

- Contratos de evento e sync de Martin Kleppmann

- Chet Haase e Donny Wals para restrições nativas

- Lea Verou para estado/UX

- Metas futuras aprovadas por produto

### criterio fechamento documental

Atribuição a Alex mantida como proposta para núcleo cliente; subcritérios por plataforma revisados pelos donos correspondentes; prioridades, negativos e recuperação reconciliados com contratos, decisões e matriz; métricas não inventadas.

### estado

aceite proposto; requisito marcado APROVADO na origem; implementação não verificada

### futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-001

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-vint.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

The meaning of 'acessibilidade' is confirmed; route profiles and support declarations exist for the selected service; IPv6, IPv4, QUIC, secure TCP and authorized relay can be isolated as conditions where implemented. Expected service and evidence fields are defined by a future SPEC.

### Estímulo

Evaluate each declared path separately and in combination: reachable IPv6, unavailable IPv6 with IPv4, QUIC without usable UDP path, compatible secure TCP, no direct peer route with an authorized relay, all routes unavailable, and recovery of a family or direct route.

### Esperado

A future run records candidates, eligibility, evidence method, selected path, fallback reason and observed outcome. Prefer eligible IPv6; use compatible IPv4 when IPv6 is not reachable. Use secure TCP or authenticated relay only when declared supported and allowed. Reject incompatible protocol negotiation safely. Apply the route profile after recovery.

### Invariante

All paths preserve authentication and authorization; QUIC, TCP and relay support are never assumed. Preference for IPv6 cannot prevent a compatible authorized IPv4 path. A connection is not proof of message or event delivery. If UI accessibility is in scope, fallback and error states remain usable under accessibility requirements owned by Lea Verou.

### Negativo

Reject unsupported transports, unauthorized relay, insecure version downgrade and routes denied by policy. If no route is eligible, report service unavailability without claiming reachability. Confirm that loss of IPv6, QUIC failure or relay denial cannot trigger weaker authentication.

### Degradação e recuperação

Report partial path availability per route and service; use only an authorized fallback. When no path is eligible, report offline or unknown according to evidence. After recovery, follow approved hysteresis without oscillation. Latency, loss, window, retry, cost and stability limits remain open until baseline and approval.

### future spec and test ids

NOT_ASSIGNED

### observed

NOT_EXECUTED

### principal documental

**name**: Vint Cerf

**status**: confirmed_for_network_transport_acceptance

**basis**: The matrix assigns Vint to the IPv6, IPv4, QUIC, TCP and relay test criterion. Brendan Gregg remains a dependency for future evidence method. The meaning of 'acessibilidade' is ambiguous; if it means UI accessibility, Lea Verou must confirm that scope.

### evidencias

- **file**: docs/00-produto/requisitos.md; **section**: Não funcionais; **line**: 37; **sha256**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664; **quote**: | RNF-001 | IPv6-first sem prejudicar acessibilidade | Testes de IPv6, IPv4, QUIC, TCP e relay |

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **section**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **line**: 113; **sha256**: c00e82090170872591cc5318f89b83d0a7af37294daae9e76e17795ec314e4e9; **quote**: | RNF-001 | [origem L37](../00-produto/requisitos.md) | Critério na origem; aprovação quantitativa e vínculo a decisão não demonstrados nesta integração | [Plano de testes](../05-operacao/testes.md); contrato específico a conciliar pelo principal | Vint Cerf — proposto por especialidade, confirmação por requisito pendente | Observar: Testes de IPv6, IPv4, QUIC, TCP e relay. Negativa/degradação e limiares: completar no registro comportamental. | Aberto: conciliar decisão, aceite e cenário com o principal; não liberado | NOT_ASSIGNED | NOT_ASSIGNED | NOT_EXECUTED |

- **file**: docs/05-operacao/testes.md; **section**: Rede; **lines**: 6-12; **sha256**: 9c4e1d46c0e9cfd39021c48487e91829440f57470f68cb8516d019aa7d4dd727; **quotes**: - - [ ] Conexão preferencial IPv6 quando alcançável.; - - [ ] Fallback IPv4 com IPv6 indisponível e sem downgrade de segurança.; - - [ ] Conexão via relay autorizado quando peers não têm caminho direto.; - - [ ] Negociação de versões incompatíveis falha de forma segura.

- **file**: docs/01-arquitetura/estrategia-tecnologica.md; **section**: Zona B; Negociação compatível; Garantias e testes exigidos; **lines**: 53-64, 80-86, 112-118; **sha256**: f79e0be0294f612c472d478358d123756fa6b83e124e08ce60d72750bb332c7d; **quotes**: - IPv6 preferencial com IPv4 funcional; ambos testados; - Negociar versões realmente implementadas, inclusive QUIC v2 **somente se** a biblioteca de produção suportar e passar nos testes; nunca declarar suporte por desejo; - 3. **Teste dual-stack:** rede IPv6-only, IPv4-only e dual-stack com fallback e mudanças de endereço.

### aplicabilidade

**products**: - Personal

- Teams

- Node

- Platform/SDK

**platforms**: Every service whose network path uses the listed protocols, subject to declared provider and OS support. Node/Linux and clients are included when they implement the path. A component that does not expose QUIC, TCP or relay is not applicable to that individual transport, with the reason recorded; this does not remove other paths from scope.

**not applicable**: A radio-only endpoint with no IP transport is not subject to direct IPv6/IPv4, QUIC or TCP checks; its gateway's IP path remains applicable. UI accessibility is not marked not applicable because the source requirement does not define whether it is included.

### decisoes pendentes

**items**: - **decision**: Define whether 'acessibilidade' means network reachability, user-interface accessibility, or both. Define the service profiles and any quantitative thresholds only after approved baseline.; **alternatives**: - Network reachability only: fits the listed transport cases, but may omit UI accessibility.; - User-interface accessibility only: does not explain the transport test list.; - Both: preserve network reachability and require Lea's UI acceptance criteria; recommended until the source owner clarifies.; **recommendation**: Clarify with the requirement owner; provisionally plan both, with separate criteria and owners. Do not treat this provisional interpretation as approval.; **consequence if open**: The acceptance boundary is ambiguous; a pass on transport alone cannot close RNF-001.; **scope**: - all products using network transports; - all supported client platforms; - UI accessibility if confirmed

- **decision**: Set route thresholds, measurement windows, supported transport combinations and relay eligibility by product/service/platform.; **alternatives**: - Global fixed values: simpler but potentially unsuitable for distinct service and network classes.; - Versioned profiles with common security/reachability invariants: recommended after baseline.; **recommendation**: Leave numeric values open until owners approve a baseline and test coverage.; **consequence if open**: The criterion remains qualitative and cannot establish comparable performance conformance.; **scope**: - network service profiles; - client/provider compatibility; - future test definitions

### dependencias

- RF-014 route selection and fallback semantics

- Brendan Gregg for measurement population, environment and evidence method

- Lea Verou if user-interface accessibility is in scope

- Chet Haase and Donny Wals for mobile platform API capability review

- ADR-006 IPv6 preference and ADR-007 QUIC/libp2p validation status

### documentary links

- ADR-006

- ADR-007

- PI-vint-001

- GMD-A24-001

### criterio fechamento documental

The requirement owner clarifies accessibility scope; the plan maps each supported transport/family to a versioned profile and future case with preconditions, stimulus, expected result, negative and recovery behavior; each threshold has a baseline, unit, window and approval. Any UI accessibility criteria are separately linked to the UI owner. Completion remains documentary until authorized tests are run.

### estado

acceptance proposed; meaning of accessibility and quantitative profiles open; implementation not verified; validation NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-002

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-kim.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

GMI ou principal tipado já está ligado a credential id, finalidade, issuer, audience, chave ativa e epoch; consumidor/caches têm política de validade/frescor definida; segunda chave provisionada para cenário de rotação coordenada.

### Estímulo

Reiniciar processo/cliente e alterar IP sem alterar identidade; rotacionar chave com nova chave autorizada; revogar credencial; marcar chave como comprometida e iniciar contenção; repetir evento de rotação/revogação e manter uma réplica offline durante a mudança.

### Esperado

ID GMI/node não muda após reinício, IP ou rotação; nova chave funciona somente dentro do perfil/escopo/audience; chave anterior cessa após janela aprovada; consumidor online bloqueia ações conforme confirmação de revogação; réplica offline permanece unknown/pending e aplica revogação antes de restaurar acesso após reconexão. Eventos repetidos são idempotentes e divergência de conteúdo não substitui estado anterior.

### Invariante

Identidade estável não é credencial; IP não prova controle; rotação não amplia privilégio; estado revogado não é reativado por replay; não se declara propagação global enquanto destinos permanecem offline/desconhecidos.

### Negativo

Rejeitar key id desconhecido, assinatura inválida, audience/finalidade divergente, epoch regressivo, credencial antiga após janela, evento duplicado com conteúdo diferente, operação crítica com estado stale/unknown e uso do IP como autenticação.

### Degradação e recuperação

Autoridade indisponível não permite emitir credencial/grant sensível nem declarar revogação completa; somente classe de ação local coberta por grant assinado ainda válido e aprovado pode continuar. Compromisso exige invalidar credencial, reemitir substituta, revalidar sessões/consumidores e reconciliar cópias segundo C08; prazos e atendimento de incidentes não estão definidos.

### principal documental

**nome**: Kim Cameron

**status**: proposto_na_matriz_confirmado_como_adequado

**justificativa**: O requisito foca identidade estável e rotação segura; reinício/IP são estímulos para demonstrar independência de identidade. Rede/HA e eventos são dependências, não motivo para transferir o principal.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Não funcionais; **trecho**: Identidade estável e rotação segura de credenciais; **linhas**: 38

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Não funcionais; **trecho**: Reinício, mudança de IP, revogação, compromisso de chave; **linhas**: 38

- **arquivo**: docs/02-dominio/identidade-global.md; **secao**: Estabilidade, migração e federação; **trecho**: `gmi_id` permanece constante entre regiões, clientes, celulares, servidores e organizações.

- **arquivo**: docs/07-contratos/identidade-autorizacao.md; **secao**: Chaves, rotação e revogação; **trecho**: Rotação publica a nova chave antes de seu uso.

- **arquivo**: docs/01-arquitetura/descoberta-ingresso.md; **secao**: Certificado de ingresso e validação; **trecho**: Revogar um peer incrementa o epoch do escopo, rejeita novas sessões

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux Rust

- macOS universal SwiftUI

- iOS SwiftUI

- Windows desktop

- Linux desktop

- Android

- web no escopo integral

**nota**: Aplicável a todas as identidades/credenciais suportadas; proteção de chaves e APIs variam por plataforma e ainda exigem avaliação. Reinício/IP são estímulos transversais, não requisitos de rede por si.

### decisoes pendentes

- **tema**: Frescor e validade por classe; **alternativas**: - parâmetros por credencial/ação; - um limite uniforme; **recomendacao**: Configurar limites por classe/operação com falha fechada quando ausentes; sem sugerir números.; **consequencia**: Limites determinam exposição após revogação e disponibilidade offline; prontidão bloqueada até decisão e evidência.

- **tema**: Rotação planejada vs compromisso; **alternativas**: - overlap coordenado para ambos; - política distinta com corte acelerado após compromisso; **recomendacao**: Distinguir rotação planejada de resposta a comprometimento e definir autoridade de corte, sessão e consumer acknowledgement.; **consequencia**: Sobreposição melhora continuidade mas prolonga validade da chave suspeita; corte rápido pode interromper serviço.

- **tema**: Prova/retensão após revogação; **alternativas**: - trilha mínima de auditoria com tombstone; - expurgo conforme classes/cópias; **recomendacao**: Separar bloqueio de acesso de disposição/expurgo por cópia e seguir contrato C08.; **consequencia**: Revogar não significa apagar todas as cópias; jurisdição e retenção ainda não aprovadas.

### dependencias

- Graydon Hoare: credenciais/audience/erro e auth no GMP

- Martin Kleppmann: eventos idempotentes, replay e epoch

- Leslie Lamport: autoridade e convergência

- Bruce Schneier: política de comprometimento/retenção

- Vint Cerf/Brendan Gregg: efeitos de rede/continuidade e evidência operacional

- Alan Turing: consolidação

### criterio fechamento documental

Matriz por classe com janela de rotação, expiração, frescor, revogação, cache, comportamento offline, recuperação e efeitos de cópias; integração com eventos/GMP/autoridade; limiares aprovados e critérios observáveis. Falta de limites explícitos mantém bloqueio.

### ids futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite_documental_proposto_pendente_decisoes_quantitativas

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-003

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Fixar principal/device/tenant/recurso/operação/finalidade/audiência/grant/consentimento/revogação/classe/cópia e política; sink de auditoria autorizado e sem dados reais.

### Estímulo

Autorizar fluxo permitido e negar identidade, scope, tenant, consentimento, grant, chave, replay; revogar online/offline; consultar, exportar, replayar e restaurar cópia.

### Esperado

Allow só com todos requisitos satisfeitos; deny sanitizado sem enumerar titular; recibo de auditoria registra ator e decisão por referência opaca, sem credencial/coordenada/payload; resultado de cópia por destino diferencia bloqueio de acesso de descarte confirmado.

### Invariante

Scope não supera tenant/ACL/finalidade/consentimento; auditoria não vira canal de conteúdo; revogação não é global antes de resultados aplicáveis por destino.

### Negativo

Autorizador/auditoria indisponível, status unknown, réplica offline ou tentativa cross-tenant não gera allow implícito nem falsa confirmação.

### Degradação e recuperação

Comportamento de ações sensíveis e não sensíveis durante falha precisa de política; ao reconectar/repor backup, reaplicar revogações e políticas antes de disponibilizar conteúdo; não restaurar grant vencido.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Não funcionais; **linhas**: 39; **trecho**: Isolamento multi-tenant e localização consentida

- **arquivo**: docs/07-contratos/identidade-autorizacao.md; **secao**: Decisão de autorização; **linhas**: 138-148; **trecho**: O autorizador aplica as verificações nesta ordem e nega se uma delas falhar:

- **arquivo**: docs/04-seguranca/retencao-revogacao.md; **secao**: Cópias e estados de resultado; **linhas**: 82-83; **trecho**: `access_blocked` | Leitura/uso negado nesta cópia, embora conteúdo ainda exista.

### aplicabilidade

**produtos**: Todos os serviços com API, localização, CrowdFind, Wi-Fi, mídia ou administração e todos os tenants

**plataformas**: Servidor Linux e clientes macOS/iOS iniciais; Windows/Linux desktop, Android/web preservados

**nao aplicavel**: UI não existente para operação não dispensa teste de endpoint/autorização; ação offline N/A somente se contrato explicitamente não oferecer offline.

### decisoes pendentes

- **tema**: frescor e indisponibilidade; **alternativas**: - negar operação sensível offline; - grants offline assinados com limite por classe; **recomendacao**: Negar quando autorização desconhecida; decidir disponibilidade de operação por operação; **consequencia**: Maior confidencialidade versus menor disponibilidade; cache cria janela residual

- **tema**: auditoria/retention/restore; **alternativas**: - recibos minimizados por cópia; - retenção operacional mais ampla mediante finalidade aprovada; **recomendacao**: Recibo minimizado separado de telemetria e com prazo definido; **consequencia**: Jurisdição, finalidade e duração são decisões do usuário/revisão jurídica

### dependencias

- Kim Cameron: identidade/chaves

- Martin Kleppmann: propagação e replay

- Leslie Lamport: autoridade/fencing

- Michael Stonebraker e Bruce Momjian: cópias/restore

- Brendan Gregg: pipeline de evidência

### criterio fechamento documental

Matriz de autorização/cópia e casos positivos/negativos/revogação/replay/restore ligados aos contratos; states C08/sync conciliados; critérios e evidência futura têm owner.

### estado

proposta comportamental; NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-004

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-kleppmann.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Mutação e Outbox estão duráveis sob transação local; receptor conhece Inbox/checkpoint ou snapshot íntegro e escopado; identidade/digest/schema/autorização/retention aplicáveis podem ser verificados; checkpoint representa o último prefixo contíguo durável.

### Estímulo

Sequência planejada de envio normal, redelivery após ACK perdido, duplicação, reorder, lacuna, snapshot+replay, tombstone, partição e crash/restore antes/depois de commits de Inbox e projeção.

### Esperado

Redelivery idêntica não duplica efeito; conflito é rejeitado. Reorder mantém lacuna e não avança checkpoint; snapshot validado aplica somente eventos posteriores; tombstone replica como evento e não afirma eliminação física de todas as cópias. ACK ocorre apenas após persistência Inbox/checkpoint; projeção é atômica com Inbox se compartilham transação ou recuperável/idempotente se separada.

### Invariante

Sem perda silenciosa, duplicação de efeito, ACK prematuro, checkpoint não contíguo ou exatamente-uma-vez presumido. Identidade/dedupe, cursor e política permanecem vinculados a escopo; Inbox durável é separada da conclusão da projeção.

### Negativo

Crash antes do commit não ACKa; crash após commit e antes de ACK permite redelivery deduplicada. Inbox indisponível impede ACK. Identidade igual com digest diferente falha fechado. Evento 12 antes do 11 mantém checkpoint em 10. Digest/escopo inválido em snapshot não move cursor; cursor expirado não pula intervalo.

### Degradação e recuperação

Projeção separada mantém estado pendente e worker/reconstrução a retoma de forma idempotente. Falta de política eficaz/autorização bloqueia replay persistente. Restore reaplica/valida revogações antes de servir dados protegidos. Se estado de dedupe/checkpoint não puder ser provado, bloquear o efeito/serving afetado e reconciliar. Período, limiares, RPO/RTO e carga ficam pendentes.

### principal documental

**name**: Martin Kleppmann

**status**: confirmado_por_autor_como_principal_documental_de_idempotencia_replay_e_recuperacao

**conflito**: None

**limite**: Michael lidera persistência/dados; Graydon wire/GMP; Leslie autoridade distribuída; Bruce retenção.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao linha**: linha 40; **trecho**: Sincronização idempotente e recuperável | Partições, duplicação, reorder, snapshots e tombstones; **estado**: critério de origem

- **arquivo**: docs/01-arquitetura/replicacao.md; **secao linha**: §Fluxo linhas 23-33; §Snapshot + replay linhas 35-40; §Deleções e retenção linha 43; **trecho**: quando não compartilham [transação], o registro Inbox é a fonte durável e a projeção separada deve ser repetível, observável e reconstruível; **estado**: diretriz já especificada; fronteira de store e prova não verificadas

- **arquivo**: docs/07-contratos/eventos-sincronizacao.md; **secao linha**: §3 e §§5-8; **trecho**: casos de conformidade são critérios futuros, não testes executados; **estado**: proposta documental v0.1

- **arquivo**: docs/05-operacao/testes.md; **secao linha**: §Federação/replicação linhas 14-21; **trecho**: Reconexão recupera backlog com checkpoints; Eventos duplicados não geram registros duplicados; **estado**: plano de validação; não executado

### aplicabilidade

**produtos**: - Node e serviços que persistem/replicam eventos: direta

- Teams: direta no sync entre regiões/tenants

- Platform/SDK: direta quando expõe ou consome sync

- Personal: aplica a outbox/cursor local quando existir esse fluxo

**plataformas**: - Servidor Linux: direta

- macOS universal/iOS SwiftUI, Windows/Linux desktop, Android e web: critérios locais específicos quando persistirem outbox/cursor ou consumirem replay; armazenamento e ciclo de vida local ainda precisam de integração dos responsáveis de cliente

**nao aplicavel**: Não declarar plataforma inteira não aplicável. Componente sem estado local ou sync só pode ser excluído mediante justificativa revisada por seu dono; o serviço remoto que fornece sync continua coberto.

### decisoes pendentes

- **questao**: Fronteira Inbox-materialização; **alternativas**: - transação única quando compartilham store; - ledger Inbox durável e projetor idempotente/reconstruível em stores separados; **recomendacao**: transação única apenas com fronteira comum comprovada; caso contrário ledger e projeção versionada, observável e reconstruível; **consequencia**: define ACK, checkpoint, retry e recuperação; não assumir transação distribuída

- **questao**: Retenção de replay; **alternativas**: - período por classe/cópia aprovado; - replay bloqueado sem política eficaz; **recomendacao**: sem política eficaz, bloquear cópia/replay persistente; **consequencia**: afeta tombstones, backups, recibos, dedupe, nós offline e custo

- **questao**: Tecnologia de fila e cache; **alternativas**: - JetStream como transporte; - log próprio; Redis como cache derivado em qualquer caso se aprovado; **recomendacao**: comparar sem delegar autoridade nem retenção de domínio ao cache; **consequencia**: afeta operacionalidade, recuperação, versões mistas e custos

- **questao**: Metas de recuperação; **alternativas**: - definir RPO/RTO/SLO por produto após requisito e medição; - manter readiness qualitativa sem metas aprovadas; **recomendacao**: não inventar números; bloquear promessa quantitativa até decisão/medição; **consequencia**: capacidade e aceitação quantitativa permanecem abertas

### dependencias

- Michael/C11: fronteira transacional, Inbox/projeção e persistência

- Graydon/C07: ACK, erro/retry e wire

- Bruce/C08: retenção de eventos, tombstones, recibos e cópias

- Kim/C09: autorização/revogação

- Leslie/C06: epoch/handoff

- Alex: persistência local quando aplicável

- Brendan: recuperação/observabilidade e metodologia

- usuário: jurisdição, metas, carga

- revisão independente

### criterio fechamento documental

Para cada produto/plataforma aplicável, contrato e futura SPEC definem commit, ACK, dedupe, checkpoint, projeção, tombstone, snapshot e recovery para falhas antes/depois de commits; opções de fronteira e retenção são decididas e compatíveis. Evidência de teste/restore futura permanece separada do fechamento documental.

### estado

parcial

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-005

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-brendan.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Declarar topologia/escopos de falha, versão e protocolo compatíveis, rota/região autorizada, predicados e freshness aprovados, operação e comportamento das sessões existentes, workload do cenário, ambiente isolado e evidência redigida. Para falha zonal, declarar associação nó→zona; isso não está decidido na fonte.

### Estímulo

Em cenários separados, retirar uma instância, uma zona declarada, uma região autorizada ou o caminho de quorum; preservar o estado inicial e a classe de operação para cada execução.

### Esperado

O escopo sem evidência atual deixa de receber novas admissões. Destino alternativo recebe tráfego somente com autorização, residência/autoridade, versão, readiness e capacidade válidas. Sem destino seguro, informar degradação/indisponibilidade explícita. Em partição, bloquear somente operações que exijam autoridade/quorum ausente e permitir apenas operações locais explicitamente autorizadas. Replay/reconciliação ocorre antes do retorno quando exigido pelo contrato de dados.

### Invariante

unknown/stale nunca equivale a saudável, livre ou autorizado; failover não viola residência nem autoridade; persistência, idempotência e sessões seguem seus contratos de domínio. Não prometer migração contínua de voz/PTT ativa.

### Negativo

No-ACK, destino alternativo não autorizado, protocolo incompatível, freshness expirada ou ausência de quorum não são sucesso e não podem encaminhar tráfego como se estivessem prontos.

### Degradação e recuperação

Se todos os destinos seguros falham, declarar indisponibilidade e limitar operação ao que o contrato local autoriza. Após recuperação, exigir predicados atuais, readiness e reconciliação antes da admissão gradual; verificar estabilidade conforme histerese aprovada. Registrar esperado e observado em campos separados.

### principal documental

**nome**: Brendan Gregg

**estado**: proposto_por_especialidade_na_matriz; confirmacao_por_requisito_pendente

**adequacao**: Adequado como principal metodológico SRE para critérios de disponibilidade, degradação, evidência e gate; não como owner de semântica distribuída, operação ou targets.

**interfaces**: - Leslie Lamport: autoridade e partições

- Martin Kleppmann: replay/reconciliação

- Graydon Hoare e owners: serviços/rotas

- produto/organização: escopo e decisão

### evidencias

- **file**: docs/00-produto/requisitos.md; **location**: RNF-005, linha 41; **excerpt**: Alta disponibilidade com degradação explícita; falha de instância, zona, região e partições; **state**: documented_requirement_and_candidate_criterion

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **location**: Cobertura individual, linha 117; **excerpt**: principal Brendan proposto; SPEC/caso NOT_ASSIGNED; observado NOT_EXECUTED; **state**: documented_proposal_not_execution

- **file**: docs/01-arquitetura/balanceamento.md; **location**: § Escolha de nós, § Readiness e efeito de admissão, §§ Evitar oscilação e Sobrecarga, linhas 17–19, 32–36, 38–40, 61–63; **excerpt**: admissão exige autorização/capacidade; unknown bloqueia failover; indisponibilidade explícita; **state**: documented_contract_proposal

- **file**: docs/05-operacao/readiness.md; **location**: § Estados e transições e § Predicados por tipo de serviço, linhas 7–15, 23–28, 34–42; **excerpt**: liveness não prova capacidade; estado e efeito de admissão são escopados; **state**: documented_contract_proposal

- **file**: docs/01-arquitetura/nos-regioes.md; **location**: § Região e § Governaça sem SPOF, linhas 13, 34–40; **excerpt**: vários full nodes por região; disponibilidade durante partição depende da maioria; **state**: documented_architecture_proposal

- **file**: docs/05-operacao/testes.md; **location**: § Federação/replicação e § Capacidade, linhas 17–28; **excerpt**: queda de nó, partição, nó saturado, falha regional são itens de checklist; **state**: planned_checklist_not_execution

- **file**: Planejamento-Integral-2026-11-16/contribuicoes/brendan.json; **location**: items PI-brendan-001, PI-brendan-002 e PI-brendan-004; related_ids; **excerpt**: RNF-005 consta explicitamente nas relações registradas; **state**: verified_prior_planning_link_not_closure

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal

- Windows

- Linux desktop

- iOS SwiftUI

- Android

**regra**: Failover/partição é propriedade de serviço e rota no servidor; clientes por plataforma observam apenas transição e reconexão conforme contrato. O escopo futuro de entrega inclui todas as plataformas listadas; a primeira implementação futura Linux/Rust e macOS/iOS não reduz o escopo.

**nao aplicavel**: Se o cenário não tiver redundância zonal declarada, marcar somente o subcenário zonal como não aplicável com justificativa de topologia. Isso não fecha RNF-005 nem outras falhas.

### decisoes pendentes

- **questao**: Topologia/regiões/zonas, falhas toleradas, serviços gate, comportamento de sessões e operações locais; **alternativas**: - A: health/ping global; - B: contrato de estado por serviço/rota e destino elegível; - C: definir cenário após topologia e baseline; **recomendacao**: B com cenários parametrizados e baseline antes de alvos quantitativos; **consequencia**: Sem topologia e workload aprovados, o aceite permanece qualitativo; ping não demonstra capacidade, autoridade ou disponibilidade fim a fim.

- **questao**: Freshness, histerese, capacidade, disponibilidade/SLO, error budget e RTO; **alternativas**: - fixar valores agora sem baseline; - medir baseline e aprovar por operação; - não usar metas operacionais; **recomendacao**: Não inventar valores; obter entradas e aprovação por operação; **consequencia**: Sem alvos aprovados não há gate quantitativo nem declaração de cumprimento.

### dependencias

- topologia e workload aprovados por produto/infra

- fontes e predicados de readiness por rota

- semântica de autoridade e partição de Leslie

- sync/replay e integridade dos owners de dados/eventos

- procedimento e owner operacional/on-call ainda não atribuídos

- caso futuro isolado e autorização de execução

### referencias planejamento verificadas

- PI-brendan-001

- PI-brendan-002

- PI-brendan-004

### criterio fechamento documental

Registrar topologia e dependências por rota, efeitos em sessões/admissões, política de autoridade/reconciliação, freshness e decisões/targets com responsáveis; revisão documental confirmada. Essa conclusão não depende de teste, mas o gate de produto exige evidência futura de falha, unknown, degradação e recuperação, hoje NOT_EXECUTED.

### estado

proposto_documental_parcial; SPEC=NOT_ASSIGNED; teste=NOT_ASSIGNED; observado=NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-006

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-brendan.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Owners declaram operações críticas e opcionais, recursos/filas compartilhados, prioridades, reserva, capacidade segura, comportamento de rejeição/retry e fontes/cobertura. Workload nominal/pico e ambiente isolado são aprovados. Sem classificação/reserva aprovada, o caso não tem limite verificável.

### Estímulo

Aumentar tráfego sintético opcional em recurso/fila compartilhados até cada condição de admissão aprovada; em rodada separada tornar capacidade/fonte stale ou unknown; manter operação crítica elegível concorrente.

### Esperado

Limitar/adiar/rejeitar novas operações opcionais segundo política e fornecer motivo/retry explícito; observar fila e backpressure. Preservar a reserva crítica até o limite aprovado. Quando a capacidade segura acabar, retornar falha explícita em vez de prometer continuidade.

### Invariante

Carga opcional não rebaixa autorização, isolamento, residência nem prioridade de domínio; unknown não é capacidade livre/overflow. Eventos críticos não são descartados silenciosamente; nenhuma classe recebe garantia ilimitada.

### Negativo

Operação opcional acima do limite não entra silenciosamente nem cresce em fila não limitada; retry/backoff não cria loop; falta de sinal ou capacidade conhecida nunca produz healthy ou capacidade livre.

### Degradação e recuperação

Saturação de classe limita novas admissões dessa classe conforme política. Operações já ativas são mantidas, drenadas ou rejeitadas pelo contrato de domínio. Após carga cair e sinais atuais voltarem, reabrir gradualmente conforme thresholds/histerese aprovados e verificar backlog/retry; ausência de amostra não é recuperação.

### principal documental

**nome**: Brendan Gregg

**estado**: proposto_por_especialidade_na_matriz; confirmacao_por_requisito_pendente

**adequacao**: Adequado para método de aceitação transversal de admissão, backpressure e evidência; não determina sozinho prioridade de domínio ou capacidade.

**interfaces**: - Graydon Hoare: filas e contratos de serviço

- Grady Booch: recursos/capacidade

- Nancy Leveson/Radia Perlman e owners: prioridade SOS/PTT

- produto: classes críticas/opcionais e políticas

### evidencias

- **file**: docs/00-produto/requisitos.md; **location**: RNF-006, linha 42; **excerpt**: Não saturar serviços críticos com tráfego opcional; admission control, backpressure, reservas e prioridades; **state**: documented_requirement_and_candidate_criterion

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **location**: Cobertura individual, linha 118; **excerpt**: principal Brendan proposto; SPEC/caso NOT_ASSIGNED; observado NOT_EXECUTED; **state**: documented_proposal_not_execution

- **file**: docs/01-arquitetura/balanceamento.md; **location**: § Escolha de nós, §§ Sobrecarga e Cenários de capacidade, linhas 17–19, 26–28, 55–63; **excerpt**: capacidade reservada; limitar novas admissões opcionais; rejeição explícita; limites pendentes; **state**: documented_contract_proposal

- **file**: docs/05-operacao/observabilidade.md; **location**: § Contrato de emissão e catálogo de sinais, linhas 27–29, 40–41; § Alertas, linhas 103–110, 124; **excerpt**: cobertura/perda, fila/backpressure e limites após baseline; **state**: documented_candidate_signals_and_actions

- **file**: docs/05-operacao/readiness.md; **location**: § Envelope e campos, § Predicados por tipo de serviço, linhas 23–28, 34–42; **excerpt**: capacity tem estado/unidade do serviço; readiness define efeito por rota; **state**: documented_contract_proposal

- **file**: docs/06-planejamento/decisoes.md; **location**: ADR-053, linha 74; **excerpt**: SOS/Guardian/PTT têm prioridade no app sob bateria/link restritos; **state**: approved_app_policy_context_not_server_admission_policy

- **file**: Planejamento-Integral-2026-11-16/contribuicoes/brendan.json; **location**: items PI-brendan-001 e PI-brendan-002; related_ids; **excerpt**: RNF-006 consta explicitamente nas relações registradas; **state**: verified_prior_planning_link_not_closure

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal

- Windows

- Linux desktop

- iOS SwiftUI

- Android

**regra**: Controle de saturação é exercitado por serviço/fila no servidor; clientes validam apenas estado, erro e retry definidos por contrato. Mantêm-se todas as plataformas do escopo, mesmo que a primeira implementação use Linux/Rust e macOS/iOS SwiftUI.

**nao aplicavel**: Nenhum produto ou plataforma é excluído pelo texto de origem; recurso opcional que não exista em uma implantação pode ficar fora do cenário com inventário de serviço que justifique, sem fechar o requisito integral.

### decisoes pendentes

- **questao**: Classificação crítica/opcional, prioridade, unidade/reserva de capacidade, workload e janela; **alternativas**: - A: ordem global fixa; - B: política por serviço com interfaces de domínio; - C: só medir, sem controle de admissão; **recomendacao**: B, depois de confirmar domínios e baseline; preservar explicitamente os itens essenciais aprovados; **consequencia**: Sem classificação e reserva, não há resultado reproduzível de proteção nem limite quantitativo.

- **questao**: Thresholds, fairness, backpressure, quotas, overflow regional, retry e alertas; **alternativas**: - aprovar valores sem perfil; - definir por cenário e aprovar após baseline; - deixar thresholds pendentes; **recomendacao**: Definir parâmetros por serviço/regime a partir de baseline e decisão; **consequencia**: Não marcar o gate como PASS nem ativar ações por valores inventados; unknown bloqueia a hipótese de capacidade livre.

### dependencias

- lista de operações por domínio e prioridade aprovada

- Graydon e owners confirmam semântica/fonte das filas

- Grady e owners confirmam unidade/reserva de recursos

- owners SOS/PTT definem prioridade de domínio; ADR-053 cobre apenas app sob bateria/link

- baseline de carga sintética autorizada e perfil aprovado

- owner operacional/on-call e runbook ainda não atribuídos

### referencias planejamento verificadas

- PI-brendan-001

- PI-brendan-002

### criterio fechamento documental

Registrar para cada serviço classe e prioridade, recurso/fila, fonte/unidade, reserva, condições de admissão/backpressure/retry, degradação/recuperação e decisões aprovadas; revisão documental confirma que a ordem não contradiz domínios. Caso futuro requer workload e saturação controlada com esperado/observado separados; ainda NOT_EXECUTED.

### estado

proposto_documental_parcial; SPEC=NOT_ASSIGNED; teste=NOT_ASSIGNED; observado=NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-007

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-radia.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Ambiente futuro permite observar separadamente media path, GMP controle e publicação/persistência no event bus; clientes e canal autorizados; grant válido; transporte negociado; configuração, população, janela e método registrados.

### Estímulo

Trocar mídia e eventos de controle/timeline simultaneamente; observar payloads/bytes em media path e bus. Em cenários separados introduzir jitter/perda controlados; interromper mídia, bus e controle individualmente; mudar rota/nó e retomar sob grant/stream novos.

### Esperado

Nenhum RTP/AudioFrame/Opus é serializado, publicado ou persistido como payload no event bus. Controle e metadados de timeline contratados seguem distinguíveis do áudio. Mídia usa caminho próprio. Jitter/perda/floor/reconnect são medidos com população, janela, método e perda do coletor; sem threshold aprovado não se declara PASS de qualidade.

### Invariante

Separação mídia/eventos/governança/acesso; nenhum pacote de voz em NATS/Gossipsub por evento; transporte não contorna grant; event/timeline não carregam payload de mídia.

### Negativo

Tentativa de publicar payload de mídia no bus é detectada e impedida no limite definido. Requisição de voz em endpoint data-only LoRaWAN/BLE é negada por capability incompatível, não convertida em áudio fictício.

### Degradação e recuperação

Falha do bus não migra payload de áudio ao log/bus. Mídia só pode continuar enquanto grant/controle atuais e media path permanecem válidos; se frescor/autorização/path forem perdidos, parar TX. Queda de mídia não persiste frames para replay; recuperação cria session/stream atuais. Fora do perfil futuro aprovado, informar degradação sem codec/cifra inventados.

### parametros abertos

- definição exata de event bus e alcance (efêmero/durável)

- se payload proibido inclui apenas frames ou qualquer media metadata

- limiares de jitter/perda/atraso, janela e população

- se mídia continua durante indisponibilidade isolada do bus

- TTL/frescor/timeout na retomada

### principal documental

**nome**: Radia Perlman

**status**: confirmado_por_mim_como_principal_documental

**justificativa**: Mídia/PTT, jitter/perda e reconexão pertencem ao papel de voz; fronteira de evento exige interface de Martin Kleppmann, sem transferência do principal.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos não funcionais; **linhas**: 43; **trecho**: RTP/mídia não transportada pelo event bus | Teste de jitter, perda, reconexão e floor control; **estado origem**: critério documental de origem; sem limiares quantitativos

- **arquivo**: docs/01-arquitetura/protocolo-nativo.md; **secao**: Dados, sincronização e escalabilidade; **linhas**: 122; **trecho**: para transportar cada pacote de voz.

- **arquivo**: docs/01-arquitetura/midia-nativa.md; **secao**: Invariantes; Planejamento de capacidade; **linhas**: 34-43; **trecho**: Controle PTT e voz podem degradar de maneira diferente; eventos e SOS não dependem do codec.

- **arquivo**: docs/06-planejamento/decisoes.md; **secao**: ADRs-014 e 037; **linhas**: 20,46; **trecho**: Separação estrita de mídia, eventos, governança e acesso

### aplicabilidade

**produtos**: **Personal**: aplicável a mídia PTT

**Teams**: aplicável a mídia PTT

**Node**: aplicável a encaminhamento/serviço de mídia

**Platform SDK**: aplicável ao contrato de media/control

**plataformas**: **Linux server Rust**: aplicável

**web**: aplicável; RTP/WebRTC ou formato selecionado

**macOS universal SwiftUI**: aplicável conforme transporte aprovado

**iOS SwiftUI**: aplicável conforme transporte aprovado

**Windows desktop**: aplicável, implementação posterior

**Linux desktop**: aplicável, implementação posterior

**Android**: aplicável, implementação posterior

**nao aplicavel**: - **alvo**: endpoints LoRaWAN/BLE data-only; **justificativa**: Não transportam PTT baixa latência segundo visão.md:35–37; roteador deve recusar capability de mídia. O serviço de mídia e sua fronteira de eventos continuam aplicáveis.

**limite**: Bridges devem obedecer às fronteiras/media contracts, mas seu comportamento de interoperabilidade é coberto pelos RF-007/008; não são evidência de transporte de áudio por event bus.

### decisoes pendentes

- **assunto**: definição de event bus e payload; **alternativas**: - proibir frames/payload de voz em qualquer bus e permitir controle/metadados contratados; - proibir toda referência de sessão/timeline junto com voz; **recomendacao**: proibir payload RTP/AudioFrame/Opus em barramento de eventos; documentar controle e metadados permitidos separadamente; **consequencia**: requer mídia path próprio; proibição de todo metadado poderia quebrar timeline/correlação; **escopos**: - controle GMP; - event bus/log persistente; - mídia; - timeline/observabilidade; **responsavel da decisao**: usuário/arquitetura; interface Martin Kleppmann

- **assunto**: budgets de qualidade; **alternativas**: - limiares uniformes; - profiles por classe de rede/plataforma; **recomendacao**: profiles por rota após baseline, sem números presumidos; **consequencia**: sem decisão e medição só existe critério qualitativo, não PASS/qualidade garantida; **escopos**: - web; - mobile; - desktop; - relay/regiões

- **assunto**: indisponibilidade do event bus; **alternativas**: - mídia continua só enquanto grant/controle e media path válidos; - interromper todas as sessões quando o bus falha; **recomendacao**: independência apenas quando grant/frescor e media path são verificáveis; fail-closed se perdem autoridade; **consequencia**: disponibilidade depende de separar bus, controle e mídia; comportamento atual não verificado; **escopos**: - session lifecycle; - PTT; - eventos/timeline

### dependencias

- Martin Kleppmann para fronteira de event bus/eventos duráveis

- Vint para transporte/relay

- Brendan para método e população de medição

- PI-radia-001 para grant e resume

- decisão sobre topologia, thresholds, janelas e event bus

### criterio fechamento documental

Definir termos bus/payload/metadados e topologia; perfil de mídia e thresholds por população/janela/método aprovados; cobrir caminho normal, payload indevido, jitter/perda, falha separada de bus/controle/mídia e recuperação; interface Martin/Vint/Brendan revisada; SPEC/casos permanecem NOT_ASSIGNED e resultado NOT_EXECUTED.

### estado

planejado_parcial_com_decisoes_abertas

### futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### referencias comprovadas

- ADR-014

- ADR-037

- PI-radia-002

### natureza

critério de mídia e testes propostos; separação é direção arquitetural; implementação/quality gates não verificados

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-008

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-fowler.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Martin Fowler. Subescopos e deltas vinculados: [Conciliação fowler](#conciliacao-fowler). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Inventário de versões de peers, protocolo, firmware e features; coorte/canary e recuperação definidos.

### Estímulo

Executar coexistência N/N-1 conceitual; peer sem versão comum; firmware incompatível; abortar canary e iniciar rollback.

### Esperado

Comunicar somente em perfil explicitamente compatível; incompatibilidade detectada antes de efeito incompatível; rollout pausa; rollback restaura perfil seguro ou isola dispositivo.

### Invariante

Não anunciar compatibilidade não verificada; não descartar estado irreversível na janela definida; firmware não contorna autorização/autoridade.

### Negativo e degradação/recuperação — bloco combinado

Sem interseção, falhar explícito; violação do critério canary interrompe expansão; rollback inseguro não forçado, deixar estado degradado e caminho de recuperação documentado.

### limiares abertos

Definição N/N-1 por protocolo/schema/firmware, janela de coexistência, condições de canary/abort, reversibilidade, hardware/firmware.

### principal documental

Martin Fowler — proposto na matriz; adequado ao protocolo, com interface necessária a Graydon (GMP), Jack Ganssle (firmware) e operação. Exclusividade não demonstrada.

### evidencias

- **fonte**: 00-produto/requisitos.md:44; **trecho**: Evolução de protocolos/firmwares interoperável — Negotiation, compatibilidade N/N-1, rollout canary.; **hash**: c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664

- **fonte**: 01-arquitetura/estrategia-tecnologica.md §§GMP—evolução, garantias, linhas 76-86,110-116; **trecho**: Negociação de compatibilidade e testes de pares antigo/novo; não validação realizada.; **hash**: f79e0be0294f612c472d478358d123756fa6b83e124e08ce60d72750bb332c7d

- **fonte**: 07-contratos/gmp-api-v1.md §§3,6,10-11; **trecho**: Negociação do protocolo, erros e casos de conformidade futuros.; **hash**: 17a225a4ee2bdf8679037d1f6bb6dff55fcf24bbbe2ed8501d3e02f007d159aa

### aplicabilidade

**produtos**: - Node

- Platform/SDK

- Personal

- Teams

**plataformas**: - servidor Linux

- macOS universal

- iOS

- Windows

- Linux desktop

- Android

- dispositivos com firmware

**justificativa**: Contrato protocolo vale para peers; firmware só para Node/dispositivo gerenciado com firmware; não aplicável a cliente sem firmware nessa dimensão.

### decisoes pendentes

- **questao**: Definir dimensões versionadas e política N/N-1/canary/rollback.; **alternativas**: - Separar protocolo, schema e firmware (recomendado); - Tratar uma versão agregada; **recomendacao**: Separar dimensões e políticas por perfil.; **consequencia**: Evita falso pareamento de versões, requer inventário e responsabilidade conjunta.

### dependencias

- Graydon/GMP

- Jack Ganssle/firmware

- operação/canary

- política de release

### criterio fechamento documental

Owners de protocolo/firmware/operação definem dimensões e gates verificáveis; decisão/aceite rastreados e revisão independente. SPEC/teste NOT_ASSIGNED; NOT_EXECUTED.

### estado

proposta documental; quantitativos e decisão N/N-1 ausentes; não SPEC, não implementado, não validado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-009

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Martin Kleppmann. Subescopos e deltas vinculados: [Conciliação kleppmann](#conciliacao-kleppmann). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Comando autenticado/autorizado e identificado com chave idempotente/escopo; tentativa não iniciada; capacidade de recibo do dispositivo declarada.

### Estímulo

Persistir e encaminhar; injetar aceites de gateway/rede, recibo de dispositivo, reconhecimento humano, timeout, redelivery, partição e recuperação.

### Esperado

Timeline relaciona command/event/attempt/actor; persistido prova commit correspondente; confirmação de dispositivo exige recibo autenticado vinculado; reconhecimento humano exige ação autorizada; retry idempotente devolve operação existente sem efeito duplo.

### Invariante

ACK técnico ≠ ACK humano; replay não executa comando; estado terminal não regride; confirmação desconhecida não vira sucesso; escopo de tenant é respeitado.

### Negativo

Gateway/MQTT sem recibo não é confirmed_by_device; chave com fingerprint diferente gera conflito; protocolo sem ACK declara confirmation_unavailable; timeout pós-envio é outcome_unknown, não reexecução automática.

### Degradação e recuperação

Em partição/out-of-order não avançar checkpoint sobre lacuna; conservar tentativas e consultar mesma operação; após reconnect deduplicar e reconciliar em ordem permitida.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: False

**estado**: conflito_de_escopo

**principal recomendado**: Martin Kleppmann

**interfaces recomendadas**: - Nancy Leveson para ACK/estados SOS

- Graydon Hoare para contrato de serviço

**fonte**: CONTEXTO-E-PAPEIS.md: Kleppmann é o único principal transversal de eventos/sync durável; Nancy lidera semântica de segurança operacional SOS.

**justificativa**: O requisito cobre estados de comandos e ACK durável ao longo da sincronização, não apenas resposta SOS.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos não funcionais; **linhas**: 45; **trecho**: Rastreabilidade dos estados de SOS e comandos | ACK persistido, recebido, confirmado e reconhecido

- **arquivo**: docs/02-dominio/eventos.md; **secao**: Estados de um comando; ingestão; **linhas**: 39-47; **trecho**: O estado do comando, o resultado de cada tentativa de transporte e o reconhecimento humano são dimensões separadas.

- **arquivo**: docs/07-contratos/eventos-sincronizacao.md; **secao**: Estados, transições, retry; **linhas**: 13-47, 69-77; **trecho**: Timeout após submissão: cliente consulta com a mesma chave; ... não interpretar timeout como falha terminal.; **estado**: proposta v0.1, prazos não aprovados

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 121; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal

- iOS

- Windows

- Linux desktop

- Android

- Web

- dispositivos autorizados

**regra**: Todo produtor/consumidor de comandos ou SOS preserva os marcos definidos; suporte a ACK depende do perfil de transporte.

**nao aplicavel**: A confirmação de dispositivo não se aplica a perfil sem ACK fim a fim; ainda se exige declarar última etapa comprovada e ausência de confirmação.

### decisoes pendentes

- **decisao**: Ratificar vocabulário wire/crosswalk e predicado de completed por operação.; **alternativas**: - Vocabulário da proposta v0.1; - Mapeamento próprio por domínio; **recomendacao**: Consolidar crosswalk por operação e versão; não considerar proposta v0.1 congelada.; **consequencia**: Sem isso não há interoperabilidade ou critério final de PASS documental.

- **decisao**: Definir retenção, idade de chave, skew/relógio e política de reconciliação.; **alternativas**: - Prazo configurável por operação/tenant; - Tombstone sem expiração; **recomendacao**: Decidir por operação com requisitos de auditoria e custo; nenhum valor numérico nesta proposta.; **consequencia**: Afeta deduplicação, recuperação e privacidade.

### dependencias

- C07 contrato wire/API

- Kleppmann sync/replay/checkpoint

- Política de retenção C08

- Identidade e autorização

- Semântica de ACK SOS definida por Nancy

### criterio fechamento documental

Vocabulário e evidências, idempotência, estado unknown, replay, terminalidade, perfil sem ACK e retenção são definidos/revisados por domínio e cruzados com casos positivos, negativos e recuperação; principal geral aprovado como Kleppmann ou conflito explicitamente resolvido.

### estado

critério documental proposto; principal conflitante; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-010

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-brendan.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Declarar dados/tenant/região, backup e chaves autorizados, isolamento do alvo, invariantes lógicas e retenção; evento/fonte, campos permitidos, destino e cobertura da telemetria; e, no ramo RF, tipo de rede, região, titular, licença, operador e modo RX/TX. Não presumir backup de cliente nem uso universal de bridge.

### Estímulo

(1) restaurar snapshot autorizado em ambiente isolado e reconciliar dados/eventos necessários; (2) emitir evento sintético com campos permitidos/proibidos e simular falha de fonte/coletor/exportador; (3) solicitar TX sem autorização/lease/fencing válido. Só testar TX permitido com autorização separada, cenário sem RF ou equipamento de teste aprovado.

### Esperado

Marcar restore íntegro apenas após verificação física, invariantes lógicas e escopo aprovados; manter readiness/admissão bloqueadas até restore/replay/cobertura exigidos estarem atuais. Observabilidade apresenta evento/classe sanitizada e cobertura, sem campos proibidos; falta de fonte resulta unknown. Sem licença, opt-in, autorização de titular ou fencing, bloquear/auditar TX sem conteúdo sensível. Acesso a rede/servidor público não prova autorização.

### Invariante

Backup, réplica, SELECT ou processo saudável não equivalem a restore verificado; dados não saem do escopo autorizado; logs não divulgam segredo/PII fora da política; nenhuma transmissão RF ocorre sem autorização e controle exclusivo; teste de software não certifica conformidade jurídica.

### Negativo

Backup corrompido/incompleto, chave inacessível, escopo divergente ou integridade não demonstrada impedem sucesso/readiness. Falta de cobertura impede conclusão positiva. Política, licença, consentimento, titularidade ou fencing ausente/stale impede TX; validar o bloqueio seguro sem emitir RF.

### Degradação e recuperação

Manter restore falho isolado; selecionar outra recuperação apenas conforme decisão e repetir verificações antes de reabrir tráfego. Recuperar cobertura/exportação sem enviar backlog para fora da autorização. Restaurar autorização RF não liga TX automaticamente; nova operação requer autorização explícita. Comparar ponto/tempo recuperado a RPO/RTO somente após aprovação desses alvos.

### principal documental

**nome**: Brendan Gregg

**estado**: proposto_por_especialidade_na_matriz; confirmacao_por_requisito_pendente

**adequacao**: Adequado para integrar critério de operação/evidência; o requisito composto não torna Brendan owner técnico de backup, observabilidade, segurança, RF ou compliance.

**interfaces**: - Bruce Momjian: backup/restore físico

- Michael Stonebraker: integridade lógica

- Bruce Schneier/Kim Cameron/Richard Bejtlich: política, identidade e detecção

- responsáveis de bridge/RF, produto, operador e jurisdição: autorização/política local

### evidencias

- **file**: docs/00-produto/requisitos.md; **location**: RNF-010, linha 46; **excerpt**: Operação e compliance; backups restaurados, observabilidade e políticas RF; **state**: documented_requirement_and_candidate_criterion

- **file**: docs/06-planejamento/matriz-rastreabilidade.md; **location**: Cobertura individual, linha 122; **excerpt**: principal Brendan proposto; SPEC/caso NOT_ASSIGNED; observado NOT_EXECUTED; **state**: documented_proposal_not_execution

- **file**: docs/05-operacao/implantacao.md; **location**: § Backups, linha 79; **excerpt**: backups criptografados e restauração periódica; réplica não é backup; **state**: documented_policy_without_scope_or_cadence

- **file**: docs/05-operacao/observabilidade.md; **location**: linhas 27–29, 107, 111, 132, 136, 142–144; **excerpt**: cobertura/perda explícitas; unknown; separar réplica de restore; RPO/RTO/escopo/retenção pendentes; **state**: documented_contract_proposal

- **file**: docs/04-seguranca/uso-das-redes.md; **location**: § Três domínios separados e § Requisitos de bridge, linhas 3–16, 23–24; **excerpt**: sem licença/política/autoridade, bloquear TX; RF depende de jurisdição; **state**: documented_policy_requirement_not_legal_validation

- **file**: docs/06-planejamento/decisoes.md; **location**: ADR-028, linha 34; **excerpt**: bridge RF/FRN usa autoridade exclusiva, fencing e TX opt-in; **state**: approved_bridge_context_not_universal_compliance

- **file**: docs/05-operacao/testes.md; **location**: § Federação/replicação, § Convergência e § Privacidade, linhas 16–20, 36–47; **excerpt**: snapshot, recuperação, TX exclusivo e ausência de tráfego RF não autorizado; **state**: planned_checklist_not_execution

- **file**: Planejamento-Integral-2026-11-16/contribuicoes/brendan.json; **location**: items PI-brendan-001, PI-brendan-003 e PI-brendan-004; related_ids; **excerpt**: RNF-010 consta explicitamente nas relações registradas; **state**: verified_prior_planning_link_not_closure

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal

- Windows

- Linux desktop

- iOS SwiftUI

- Android

**regra**: Restore aplica-se ao escopo de serviço/dados efetivamente retido; observabilidade aos serviços operados; política RF a bridges/gateways capazes de transmitir e jurisdição correspondente. Escopo de backup por tipo/plataforma ainda não está definido. Mantém-se o produto integral e plataformas futuras.

**nao aplicavel**: Se cenário não habilitar bridge/transmissor, declarar não aplicável somente ao ramo de TX e justificar por inventário/configuração; verificar que TX não ficou habilitado por padrão. Isso não fecha RNF-010 nem a política RF integral.

### decisoes pendentes

- **questao**: Escopo, frequência, retenção, isolamento/chaves e invariantes de restore; RPO/RTO; **alternativas**: - A: inferir sucesso por réplica/healthcheck; - B: evidência física e lógica por escopo com RPO/RTO depois de decisão; - C: bloquear qualquer recuperação até metas numéricas; **recomendacao**: B; aprovar dados e políticas por owner/produto antes de comparar metas; **consequencia**: Sem escopo, chaves e invariantes aprovados, nenhum restore pode fechar o aceite; não inventar targets.

- **questao**: Sinais/cobertura, acesso/redaction, destino e resposta à falha da telemetria; **alternativas**: - tratar ausência como saúde; - expor unknown e bloquear conclusão positiva; - desabilitar validação de cobertura; **recomendacao**: Expor cobertura e unknown segundo contratos atuais; **consequencia**: Falta de cobertura não prova ausência de incidente nem permite readiness positiva.

- **questao**: Política RF, titular, licença, classe de rede, jurisdição e owner de conformidade; **alternativas**: - assumir conforme por conectividade; - bloquear TX até autorização e política comprovadas; - limitar teste a ambiente IP/sem RF; **recomendacao**: Bloquear TX não autorizado; usar apenas o cenário sem RF ou teste autorizado até definir jurisdição/owner; **consequencia**: Sem autoridade/regra local não há conclusão de compliance; teste de software não substitui validação legal.

### dependencias

- Bruce Momjian confirma procedimento e evidência física

- Michael Stonebraker define invariantes lógicas e isolamento

- owner de telemetria confirma fontes/cobertura/acesso/redaction

- responsáveis de bridge/RF e autoridades do operador definem política por rede/jurisdição

- produto/organização decide escopo e targets; owner operacional/on-call ainda não atribuído

- execução futura autorizada em ambiente isolado; RF real exige autorização própria

### referencias planejamento verificadas

- PI-brendan-001

- PI-brendan-003

- PI-brendan-004

### criterio fechamento documental

Desdobrar o requisito composto em ramos de backup físico, integridade lógica, observabilidade e política RF, cada um com owner/interface, escopo, evidência, decisão e condição de não aplicabilidade; confirmar retenção, chaves, invariantes, cobertura e autoridade documental. Isso só fecha a cobertura documental. Ensaios/restores/TX e verificação de compliance permanecem futuros e NOT_EXECUTED.

### estado

proposto_documental_parcial; SPEC=NOT_ASSIGNED; teste=NOT_ASSIGNED; observado=NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-011

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-daniel.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Sessão PTT ativa; política de modo, processadores permitidos, consentimento externo, gravação, acesso e retenção definida; escopo do tenant/canal e estado E2EE conhecidos; limites atuais de CPU, memória, latência e perda reconhecidos como pendentes.

### Estímulo

ASR concorre com PTT; induzem-se conceitualmente falha/sobrecarga/backpressure, consentimento ausente ou revogado, provedor não permitido, endpoint E2EE não autorizado, consulta cross-tenant e expiração independente de áudio/texto.

### Esperado

PTT mantém grant e mídia dentro do contrato enquanto ASR falha/suspende. Política nega processador remoto sem autorização e não troca silenciosamente de processador. Buffer persistente/retentativa obedece permissão; erro não registra áudio. E2EE não é anunciado como inacessível ao servidor quando este recebe áudio claro. Consulta de tenant não retorna conteúdo de outro tenant. Áudio e texto expiram por políticas próprias.

### Invariante

ASR opcional não consome recursos reservados a voz nem causa falha PTT. Áudio, texto e metadados têm autorização/retenção separadas; identificadores entre tenants não colidem em acesso. Falha ASR não é sucesso nem SOS.

### Negativo

Credencial/política inválida, tenant divergente, consentimento negado/revogado, E2EE sem endpoint autorizado ou processador não permitido resultam em negação sem saída externa, payload ou confirmação enganosa. Tentativa cross-tenant é negada sem revelar existência de conteúdo.

### Degradação e recuperação

Sob limite de capacidade ou link, suspender/reduzir ASR opcional e descartar parciais antes de afetar voz. Após reinício não restaurar dado descartado/expirado. Recuperação só retenta áudio se autorização, consentimento, prazo e limite vigentes permitirem. Revogação interrompe novos envios e aplica regra de descarte/tratamento de trânsito ainda pendente.

### limiares

Budget de CPU/GPU, memória, latência, perda, throughput e janela de medição não especificados; dependem de decisão e futura evidência, sem inventar números.

### principal documental

**nome**: Daniel Povey

**estado**: confirmado nesta contribuição

**base**: Especialidade ASR/transcrição; matriz integrada linha 123 propõe Daniel. Bruce Schneier mantém política de privacidade; Radia Perlman mantém mídia/PTT.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **linha**: 48; **trecho**: Transcrição não afeta QoS da voz; política de privacidade e ASR/retention verificáveis; **criterio original**: Testes de falha ASR, consentimento, E2EE e isolamento entre tenants

- **arquivo**: docs/01-arquitetura/transcricao-multimodal.md; **linhas**: 24, 55-69; **observacao**: Plano separado; voz não depende de ASR; políticas por canal; E2EE/endpoint autorizado; backpressure, worker, falha e retenções independentes.

- **arquivo**: docs/01-arquitetura/midia-nativa.md; **linhas**: 33-40, 45-46, 57-58; **observacao**: Invariantes PTT, permissões/retensão, E2EE e falha ASR que não degrada PTT.

- **arquivo**: docs/06-planejamento/decisoes.md; **linhas**: 47, 74; **observacao**: ADR-038 e ADR-053: transcrição remota opcional cede recursos sob restrição; direção aprovada, validação pendente.

### aplicabilidade

**produtos**: **Personal**: aplicável se habilitar ASR

**Teams**: aplicável se habilitar ASR

**Node**: aplicável aos workers e escopo multi-tenant

**Platform/SDK**: aplicável quando implementa ou expõe ASR/timeline

**plataformas**: **servidor Linux**: ASR e isolamento onde serviço for provisionado

**macOS universal/iOS SwiftUI**: quando cliente/processador local for habilitado

**Windows/Linux desktop e Android**: quando cliente/processador local for habilitado

**web**: quando parte do produto integral e compatível

**nao aplicavel**: E2EE só se aplica às sessões/canais que declarem esse modo; se ausente, registrar justificativa por sessão/canal. Multi-tenant pode não se aplicar como topologia a instalação de tenant único, mas negação de escopo cruzado continua propriedade a avaliar. Nenhuma exclusão ampla de produto/plataforma está aprovada.

### decisoes pendentes

- **tema**: Orçamento QoS; **alternativas**: - Definir orçamento após baseline por classe de dispositivo/carga; - Desabilitar ASR opcional em carga restrita até medição; **recomendacao**: Não fixar limites antes de baseline; documentar regra de admissão fail-safe; **consequencia**: Aceite quantitativo RNF-011 e claims de impacto ficam abertos até aprovação e medição.

- **tema**: Retenção/revogação; **alternativas**: - Áudio temporário explicitamente permitido com limite futuro; - Sem áudio retido e sem retentativa após falha; **recomendacao**: Sem retenção/retentativa por padrão; somente política explícita; **consequencia**: Prazos, finalidade e tratamento de dados em trânsito requerem decisão de usuário e política Bruce/Michael.

- **tema**: E2EE/provedor; **alternativas**: - Endpoint local/autorizado; - Sem ASR em canal sem endpoint autorizado; **recomendacao**: Não encaminhar áudio claro remotamente sem autorização expressa e aviso; **consequencia**: Pode reduzir disponibilidade de ASR, preservando afirmação de segurança honesta.

### dependencias

- Radia Perlman: QoS, grants e backpressure

- Bruce Schneier: consentimento, privacidade e revogação

- Michael Stonebraker: retenção/transação

- Graydon Hoare: contrato

- SRE e produto: baseline/budget

- Decisão do usuário sobre limites, jurisdição e retenção

### criterio fechamento documental

Contrato liga cada cenário a política, permissões, origem, esperado, invariante, negativa e recuperação; decisão define tratamento por canal/tenant/E2EE; limiares são aprovados e plano de evidência futura existe. Aceite documental não equivale a medição ou teste executado.

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite documental proposto; orçamento quantitativo, decisões e revisão independente pendentes; produto não validado

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-012

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-nancy.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Incidente explícito com modo real/teste identificado; evidências de destino/layer distinguíveis e estado desconhecido disponível.

### Estímulo

Executar fluxos conceituais com ACK local só; gateway aceito sem endpoint; endpoint confirmado sem claim; claim autorizado; falha gateway/região; reconnect.

### Esperado

Exibir somente última etapa comprovada e próxima pendente; local salvo só após persistência; humano assumiu só após claim; ausência de claim destacada. HTTP 200/202, sinal satelital ou reconnect não anunciam despacho/entrega.

### Invariante

Nenhum agregado oculta etapa ausente; claim não é resolução; unknown não é sucesso nem falha conclusiva; modo teste e localização protegida permanecem evidentes.

### Negativo

Gateway ou push sem ACK do endpoint não vira entregue; ACK endpoint sem ação humana não vira atendido; satélite inelegível não aparece como rota de SOS.

### Degradação e recuperação

Falha regional/sem rede preserva pendência e último destino confirmado; reconnect atualiza só por evidência nova, sem apagar histórico ou afirmar despacho público.

### principal documental

**na matriz**: Nancy Leveson — proposto por especialidade; confirmação pendente

**confirmado por autora**: True

**estado**: adequado_com_interface_sre

**principal recomendado**: Nancy Leveson

**interfaces recomendadas**: - Brendan Gregg para telemetria/SLI/alertas e evidência

- Margaret Hamilton para mensagens de produto

**justificativa**: Requisito define semântica operacional e comunicação segura dos estados SOS; medição SRE é interface necessária.

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Requisitos não funcionais; **linhas**: 49; **trecho**: SOS mostra evidência do avanço por camadas e não promete despacho ou entrega

- **arquivo**: docs/02-dominio/servico-sos.md; **secao**: Estados, escalonamento e apresentação; **linhas**: 24-35, 47-57, 66; **trecho**: Informar separadamente se o incidente foi somente registrado, se houve entrega técnica e se um responsável humano o assumiu.

- **arquivo**: docs/05-operacao/testes.md; **secao**: Transcrição multimodal e SOS; **linhas**: 49-53; **trecho**: testar ACK local, replicação, gateway, endpoint e reconhecimento humano como evidências separadas

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos; **linhas**: 124; **trecho**: Nancy Leveson — proposto por especialidade, confirmação por requisito pendente

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Integração C13; **linhas**: 40-47; **trecho**: C13-SIG-017; C13-SLI-006 ... alvo/owner operacional não designado; **estado**: método/targets pendentes

### aplicabilidade

**produtos**: - Personal

- Teams

- Node/serviços

- Platform/SDK

**plataformas**: - servidor Linux

- macOS universal SwiftUI

- iOS SwiftUI

- Windows

- Linux desktop

- Android

- Web

- clientes/dispositivos compatíveis

**regra**: Todo caminho que inicia, encaminha ou apresenta SOS mantém estado honesto; limitação de background/ACK se apresenta explicitamente.

**nao aplicavel**: Hardware sem etapa de confirmação não dispensa explicitar que a etapa é unavailable/unknown; não se exclui a regra de honestidade.

### decisoes pendentes

- **decisao**: Aprovar textos acessíveis e distinção entre salvo, enviado, recebido e assumido.; **alternativas**: - Mensagens comuns globais; - Mensagens por fase e perfil; **recomendacao**: Mensagens por fase e perfil, usando só evidências disponíveis.; **consequencia**: Sem cópia aprovada, critérios de compreensão e suporte não podem ser considerados completos.

- **decisao**: Definir SLI/alerta/prazo, owner/on-call e política de claim.; **alternativas**: - Ativar alertas antes de baseline; - Definir esquema e targets após decisão/baseline; **recomendacao**: Esquema separado; target/window só após decisão e owner; não ativar alerta sem runbook.; **consequencia**: Sem isso aceita-se regra de exibição, não desempenho ou cobertura.

### dependencias

- Contrato SOS e estados por camada

- C13-SIG-017/SLI-006

- Texto de interface

- Owner operacional e runbook

- Política de destinatários/claim

- Validação futura autorizada

### criterio fechamento documental

Cada etapa tem fonte, semântica, prova mínima, unknown/failure e apresentação; cenários correspondentes estão ligados a RF-026/RNF-009 e evidência redigida; alvos/on-call permanecem pendentes até aprovação e não são substituídos por esta proposta.

### relacionamentos comprovados

- GMD-A28-001

- PI-nancy-001

- C13-SIG-017

- C13-SLI-006

- RF-026

- RNF-009

### estado

principal confirmado nesta contribuição; SLI/on-call/decisões pendentes; SPEC NOT_ASSIGNED; caso NOT_ASSIGNED; observado NOT_EXECUTED

### spec id

NOT_ASSIGNED

### test case id

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-013

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Jouni Malinen. Subescopos e deltas vinculados: [Conciliação jouni](#conciliacao-jouni). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Scan permanece privado; AP com tenant/admin/delegação e método de prova configurados; segredo não é extraído nem usado como prova de titularidade.

### Estímulo

Detectar SSID/BSSID, consultar mapa, publicar/compartilhar, enviar senha, tentar com senha conhecida sem prova, com prova válida/expirada/contestada ou autoridade revogada.

### Esperado

Não publicar scan nem credencial por padrão; somente publicar com prova vigente de autoridade e consentimento específico; auditoria registra operação/decisão por referências sem senha, prova reutilizável ou BSSID público; tentativa não autorizada é negada.

### Invariante

Senha salva, conexão, scan, presença e auto_share não provam propriedade; nenhum segredo aparece em log, erro, telemetria ou evidência.

### Negativo

SO/controlador/API indisponível, prova inválida, tenant conflitante ou delegação revogada bloqueia publicação e exportação; não fazer fallback para rede pública.

### Degradação e recuperação

Mostrar unknown/pending e revalidar autoridade após reconexão; revogação remove projeção controlada e comunica destino sem confirmação inventada.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: conflito parcial; manter atribuição como proposta até conciliação de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Critérios adicionais não funcionais; **linhas**: 81; **trecho**: não publicar scans Wi-Fi nem senhas por padrão; provação de titularidade auditável

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: Prova de autoridade e publicação; **linhas**: 14-18; **trecho**: Ter a senha de uma rede salva, observar o BSSID ou conseguir conectá-la não prova direito de compartilhá-la.

- **arquivo**: docs/05-operacao/testes-wifi.md; **secao**: Titularidade / compartilhamento; **linhas**: 13; **trecho**: Login em Wi-Fi de terceiros não basta para publicar acesso.

### aplicabilidade

**produtos**: Personal como observador/proprietário; Teams/Node/Platform para APs, grants e API

**plataformas**: Cliente só inventaria redes onde SO/API e permissão suportam; servidor e API aplicam publicação em todas as plataformas

**nao aplicavel**: Sem API de scan, registrar unsupported e não simular inventário; AP sem controlador ainda exige método de prova aprovado, não presume N/A.

### decisoes pendentes

- **tema**: termo ‘provação’ e método de prova/delegação; **alternativas**: - controlador pareado; - delegação verificável; - verificação manual aprovada; **recomendacao**: Confirmar intenção do texto e selecionar método com Jouni; **consequencia**: Sem método a publicação fica bloqueada; manual aumenta esforço e risco de fraude

- **tema**: scan permitido por plataforma, auditoria e retenção; **alternativas**: - somente observação local; - inventário local persistente sob política; **recomendacao**: Não extrair segredos e manter escopo privado mínimo; **consequencia**: Permissões e API variam por SO; duração e jurisdição não escolhidas

### dependencias

- Jouni Malinen: domínio Wi-Fi, prova e provisionamento

- Bruce Schneier: revisão de privacidade/auditoria

- Graydon Hoare: API

- Kim Cameron: principal/grant

- Michael Stonebraker: dados

### criterio fechamento documental

Origem esclarece termo ambíguo; método de prova, estado, tenant, segredo, auditoria e falha conciliados entre contrato, API e testes Wi-Fi; cobertura futura por plataforma definida.

### estado

proposta comportamental; owner parcial em conflito; decisão de Alan pendente

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-014

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-jouni.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Jouni Malinen. Subescopos e deltas vinculados: [Conciliação jouni](#conciliacao-jouni). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Nova sessão com grant/ID; sem conexão estabelecida. Fontes de cliente, SO, controlador e teste de serviço são separadas e autenticadas conforme contrato, com hora e nível de evidência.

### Estímulo

Executar pedido; receber ou não decisão do SO; receber ou não autenticação do controlador; realizar ou não verificação de Internet/serviço. Cobrir aprovação, recusa, timeout e perda de rede entre fases.

### Esperado

Pedido aceito cria apenas requested. Só observação/callback do SO produz associação. Só controlador autenticado produz ap_authenticated/equivalente. Só verificação de Internet/serviço com método e validade definidos produz internet_verified. Trilha expõe fonte/hora/transição; sem evidência, manter unknown/pendente.

### Invariante

Não promover um estado sem evidência própria de cada fronteira. Credencial entregue não prova uso; associação/captive portal não prova Internet; evento do app não pode ser rotulado controller_attested.

### Negativo

Permissão/callback ausente deixa associação unknown; controlador offline deixa autenticação unknown; serviço/probe indisponível deixa Internet unknown. Não colapsar local_only, captive_portal e falha em sucesso.

### Degradação e recuperação

Após recuperação, exigir evidência atual e registrar uma nova transição. Evento atrasado não sobrepõe versão mais nova nem torna estado antigo atual.

### decisao

Normalizar nomes de estado versus evento (requisito usa os_associated/ap_authenticated; contratos usam os.associated/ap.authenticated e associated), definir fonte/método/frescor de internet_verified e representação de unknown/local_only/captive_portal.

### principal documental

**nome**: Jouni Malinen

**estado**: confirmado_para_esta_complementacao; proposta da matriz ainda aguarda conciliacao de Alan

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Critérios adicionais não funcionais; **linha**: 82; **trecho**: distinção verificável entre requested, os_associated, ap_authenticated e internet_verified.

- **arquivo**: docs/02-dominio/wifi-registro-acessos.md; **secao**: 1. Distinção de evidências; **linhas**: 11-21; **trecho**: connect.requested, os.associated, ap.authenticated e ap.session.started são fontes/eventos distintos.

- **arquivo**: docs/07-contratos/wifi-autodiscover-autoshare-api.md; **secao**: 4. Estados; **linhas**: 86-88; **trecho**: Estados de sessão separados; requested não é associated nem internet_verified.

### aplicabilidade

**Personal**: UI/serviço distinguem estados da própria sessão.

**Teams**: Operadores veem somente sessões do escopo autorizado, com os mesmos níveis de evidência.

**Node**: Controlador/Node atesta apenas quando fonte autorizada fornece prova; serviço sem controlador não afirma autenticação do AP.

**Platform SDK**: Preserva discriminantes e origem em respostas/eventos.

**plataformas**: **Android iOS**: Reportam somente estados expostos por APIs e permissões consentidas.

**macOS Windows Linux desktop**: Não fabricar estados de rádio/AP que não conseguem observar; preservar estados remotos recebidos.

**servidor Linux**: Aplica máquina de evidência a eventos; não infere sinal ausente.

**Web**: Apresenta estados remotos e unknown; associação local não aplicável com APIs da parcela.

### decisoes pendentes

- Vocabulário canônico de eventos e estados

- Método/fonte/validade de internet_verified

- Frescor e correlação

- Estados de callback ausente, unknown, local_only e captive_portal

- Idempotência e ordenação

### dependencias

- RF-037 e contrato de conexão

- Access Registry e identidade de controlador

- Conectividade/sonda de serviço

- Contrato de eventos/sync

### criterio fechamento documental

Normalizar eventos/estados e definir evidência, transições válidas, estados desconhecidos e recovery; ligar cada resultado ao respectivo contrato. A aceitação executável aguarda SPEC e caso atribuídos.

### spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

aceite comportamental proposto; vocabulário e evidência de Internet em aberto

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-015

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-schneier.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

### Pré-condição

Inventariar classes/cópias, tenant, função, finalidade, retenção, jurisdição, acesso e backup; política ausente mantém classe fora de prontidão para persistência/exportação/replicação conforme RR-001.

### Estímulo

Consultar/alterar AP, mapa, grant, sessão ou auditoria com roles/tenants diferentes; variar retenção vencida, exportação, revogação e restore.

### Esperado

Permitir somente função/tenant/AP/finalidade autorizados; administrador recebe metadado mínimo de seu AP; visitantes não veem terceiros; conteúdo vencido torna-se indisponível e segue disposição aprovada; remoção só é confirmada por cópia conforme evidência.

### Invariante

Role global não é universal; tenant é verificado em toda consulta/exportação; falta de prazo não autoriza retenção indefinida nem coleta; expiração não prova descarte; auditoria tem política própria.

### Negativo

Cross-tenant, role ausente, política/cópia desconhecida, auditoria indisponível ou snapshot antigo não revela dado nem restaura grant antigo.

### Degradação e recuperação

Restore reaplica retenções/revogações antes de servir; hold legal só existe com escopo, autoridade, jurisdição e prazo aprovados e bloqueia uso comum.

### ids

**spec**: NOT_ASSIGNED

**caso teste**: NOT_ASSIGNED

**observado**: NOT_EXECUTED

### principal documental

**nome**: Bruce Schneier

**status**: adequação confirmada por Bruce nesta contribuição; designação da matriz continua proposta, não operacional

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Critérios adicionais não funcionais; **linhas**: 83; **trecho**: informações administrativas de AP e visitantes protegidas por função/tenant e retenção definida

- **arquivo**: docs/04-seguranca/wifi-compartilhamento-privacidade.md; **secao**: Registro de visitantes e auditoria; **linhas**: 31-35; **trecho**: Administrador do AP acessa apenas metadados operacionais mínimos de seu próprio AP, preferencialmente GMC pseudônimo, status, duração e volume quando medido.

- **arquivo**: docs/04-seguranca/retencao-revogacao.md; **secao**: Aplicação e bloqueio de prontidão; **linhas**: 56; **trecho**: Se política, jurisdição, finalidade, autoridade ou prazo estiverem ausentes/ambíguos, falhar fechado para **persistência, nova divulgação, replicação e exportação**; não converter `null` em retenção indefinida.

### aplicabilidade

**produtos**: Serviços com AP/visitantes em Personal/Teams/Node/Platform, especialmente admin e suporte

**plataformas**: Servidor Linux e interfaces macOS/iOS/Windows/Linux desktop/Android/web recebem apenas campos autorizados

**nao aplicavel**: Instalação sem Wi-Fi Share/AP/registro de visitantes é N/A ao domínio, justificando ausência de AP; isolamento comum continua válido.

### decisoes pendentes

- **tema**: jurisdição, finalidade e retenção por cópia; **alternativas**: - matriz por classe e país; - retenção uniforme (não recomendada sem análise); **recomendacao**: Não persistir/replicar sem política efetiva por classe/cópia; **consequencia**: Sem aprovação, prontidão e exportação permanecem bloqueadas

- **tema**: roles de suporte e hold legal; **alternativas**: - sem acesso global; - acesso excepcional com autorização e auditoria; **recomendacao**: Acesso excepcional com escopo e justificativa previamente aprovados; **consequencia**: Exige decisão, auditoria, revisão jurídica e limites de retenção

### dependencias

- Jouni Malinen: AP/controlador

- Michael Stonebraker: modelo/isolamento

- Martin Kleppmann: cópia/sync

- Bruce Momjian: backup/restore

- usuário e revisão jurídica: jurisdição/retention

### criterio fechamento documental

Matriz classe×role×tenant×finalidade×cópia×prazo e casos de consulta, exportação, retenção, revogação e restore conciliados com C08/sync; critérios e owners definidos.

### estado

proposta comportamental; sem prazos/jurisdições aprovados e sem prova de expurgo

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-016

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-donny.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Lea Verou. Subescopos e deltas vinculados: [Conciliação lea](#conciliacao-lea), [Conciliação chet](#conciliacao-chet). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Evento de domínio autenticado com atividade, fase/ACK, destino e autorização; estado e evidência da origem disponíveis. SOS preserva registro, transporte e atenção humana em dimensões separadas.

### Estímulo

Percorrer SOS offline/created_local, accepted_by_node, queued, submitted, gateway_accepted, endpoint_confirmed, human claimed e resolved; gerar falha/ausência de ACK, ACK atrasado/fora de ordem/duplicado, perda de rede, stale, dispensa e recuperação.

### Esperado

Cada texto da projeção corresponde exatamente à evidência autenticada da sua fase e destino. Sem confirmação, mostrar pendente/não confirmado/desatualizado. ACK técnico pode afirmar apenas confirmação técnica da camada/destino indicado. `claimed` só após `gmp.sos.human.claimed.v1` de identidade humana autorizada; resolução requer estado de resolução registrado.

### Invariante

Nenhuma promoção por timeout, conectividade, notificação entregue, HTTP 200/202, estado `active` ou aceitação técnica. A projeção não é fonte do estado nem cria ACK; ela não expõe localização precisa, identidade civil ou credencial.

### Negativo

Rejeitar ou não aplicar evento não autorizado, de origem/atividade/destino incompatível, duplicado sem novidade, expirado, sem assinatura ou sem fase verificável. Evento atrasado não substitui estado mais recente. Ausência de confirmação humana nunca aparece como atendimento assumido.

### Degradação e recuperação

Se a projeção não puder atualizar, exibir último estado conhecido com idade/stale e fallback no app/notificação comum. Sem rede, preservar registro local e ausência de confirmação externa. Após reconexão, reconciliar eventos autenticados e ordenados pelo contrato de sync; atualizar somente evidência obtida. Remoção da superfície não cancela incidente ou sessão.

### limiares abertos

- freshness/stale por fase e tipo

- latência de projeção

- regras de precedência e ordenação

- conteúdo exato dos rótulos traduzidos por superfície

### evidencia futura

Cenários com eventos sintéticos, origem/assinatura, ACK por fase e estado esperado versus observado; captura redigida em cada sistema; teste de ausência/atraso/falha e reconciliação.

### principal documental

**proposto na matriz**: Donny Wals

**confirmacao**: adequado_para_regra_de_projecao_iOS; insuficiente_como_unico_principal_multiplataforma_e_semantica_SOS

**recomendacao**: - **responsabilidade**: ActivityKit e regra de projeção iOS; **principal**: Donny Wals

- **responsabilidade**: Live Updates e regra de projeção Android; **principal recomendado**: Chet Haase

- **responsabilidade**: semântica de ACK e estados SOS; **principal recomendado**: Nancy Leveson

- **responsabilidade**: texto, fallback e experiência; **principal recomendado**: Lea Verou

**conflito**: A regra cobre os clientes móveis Apple e Android e os significados de ACK/SOS. Donny é o especialista Apple, não o owner Android ou do estado operacional de SOS. Não transfiro a atribuição; solicito conciliação documental a Alan.

### evidencias

- **arquivo**: docs/06-planejamento/matriz-rastreabilidade.md; **secao**: Cobertura individual dos requisitos — integração de planejamento 2026-11-16; **linhas**: 81-86, 145, 148-150; **trecho**: RNF-016; Donny Wals — proposto por especialidade; aceite precisa de estados inicial/final, estímulo, negativo e degradação.

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Critérios adicionais não funcionais; **linhas**: 79-85; **trecho**: Live Activities/Updates não devem anunciar como confirmados estados sem evidência.

- **arquivo**: docs/07-contratos/live-presence.md; **secao**: Políticas de projeção; **linhas**: 28-33; **trecho**: state=active não implica socorro confirmado; a projeção não decide elegibilidade; não deve expor campos sensíveis.

- **arquivo**: docs/02-dominio/servico-sos.md; **secao**: Estados independentes e comprováveis; **linhas**: 24-35, 55-57; **trecho**: Registro, transporte e atenção humana são fases independentes; `claimed` exige identidade humana autorizada; offline não garante transmissão.

- **arquivo**: docs/07-contratos/transcricao-sos.md; **secao**: GMP — Eventos SOS / Endpoints conceituais; **linhas**: 42-64; **trecho**: Eventos separados para raised, status.changed, delivery.observed e human.claimed; HTTP 200/202 não prova despacho.

- **arquivo**: docs/05-operacao/testes-app-live-presence.md; **secao**: iOS / Android; **linhas**: 14-24; **trecho**: Matriz pede distinguir registro, entrega técnica e atendimento humano; não é resultado de execução.

- **arquivo**: Apple Developer Documentation; **url**: https://developer.apple.com/documentation/activitykit/displaying-live-data-with-live-activities; **data consulta**: 2026-11-15; **versao**: página rolling; **trecho**: ActivityKit suporta staleDate e atualizações via app/ActivityKit push; a projeção não tem rede/localização própria.

- **arquivo**: Android Developers; **url**: https://developer.android.com/develop/ui/views/notifications/live-update; **data consulta**: 2026-11-15; **versao**: documentação corrente; elegibilidade OEM pode variar; **trecho**: atividade pode ser descartada ou rebaixada pelo usuário/sistema e deve ser em andamento, iniciada pelo usuário e sensível ao tempo.

### aplicabilidade

**aplica**: - projeção iOS ActivityKit

- notificação Live Update/fallback Android

- indicadores de SOS, Guardian e PTT em superfícies móveis

**nao aplica diretamente**: - **plataforma**: servidor Linux; **motivo**: fornece estado/eventos autoritativos, não a apresentação móvel

- **plataforma**: desktop macOS; **motivo**: RNF-016 menciona especificamente Live Activities/Updates móveis; o estado informativo no app desktop segue contrato próprio

**condicao**: A indisponibilidade ou dispensa de Live Activities/Updates requer notificação/tela de fallback que preserve os mesmos níveis de evidência.

### decisoes pendentes

- **decisao**: mapeamento de ACK/evento para rótulo visível; **alternativas**: - rótulos separados por registro/transporte/humano; - rótulo resumido com link aos detalhes; **recomendacao**: resumo só pode afirmar a camada confirmada e deve abrir detalhes com estados separados; **consequencia**: mais transparência exige rótulos concisos; resumo único pode ocultar etapa pendente

- **decisao**: autoridade e ordenação dos eventos de estado; **alternativas**: - ordem local por timestamp; - sequência/versão de evento autenticada pelo contrato durável; **recomendacao**: usar cursor/ordem e idempotência do contrato C10, nunca somente relógio local; **consequencia**: depende do principal de sync e de contrato inexistente/aberto; não criar regra UI concorrente

- **decisao**: critério de stale para PTT e Guardian; **alternativas**: - limite comum; - frescor definido por tipo com fonte de domínio; **recomendacao**: definir por domínio e mostrar idade/fonte quando stale; **consequencia**: limiares quantitativos e origem de localização permanecem pendentes

### dependencias

- contrato Live Presence

- eventos de SOS/ACK de servico-sos.md e transcricao-sos.md

- C10 para ordem/idempotência/reconciliação

- política de autorização e privacidade

- owner Android (Chet)

- revisão semântica SOS (Nancy)

### criterio fechamento documental

Matriz versionada evento/ACK autorizado → estado interno → rótulo de cada target, com fase, destino, ordem, erro e fallback; cenários positivos/negativos/offline/atrasados/duplicados; privacidade em lock screen; owner de cada plataforma e revisão SOS explícitos; métricas sem limiar inventado ficam declaradas abertas.

### futura spec

NOT_ASSIGNED

### caso de teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

### estado

PARCIAL — critério documental proposto; mapeamento, ordenação, limiares e responsabilidade multiplataforma pendentes

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## RNF-017

Origem: [requisitos](../00-produto/requisitos.md). Fonte autoral: requisitos-alex.json; caminhos e hashes no relatório de integração. Contratos e evidências detalhados abaixo.

**Integrador documental confirmado:** Martin Kleppmann. Subescopos e deltas vinculados: [Conciliação kleppmann](#conciliacao-kleppmann). Esta confirmação supera somente atribuição documental histórica, sem aprovar política.

### Pré-condição

Cenário documental com duas ou mais regiões, cliente autorizado, dado sujeito a revogação, conectividade local e estado do AP com fonte/atualidade identificadas. Exige política eficaz e autoridade definidas; se ausentes, registra bloqueio.

### Estímulo

Criar partição inter-regional; durante ela, emitir revogação na autoridade reconhecida e variar disponibilidade local do AP independentemente da conectividade entre regiões; depois restaurar conectividade.

### Esperado

Expor conexão local e sincronização regional separadamente. Cópia isolada sem atualidade de autorização não serve dado sensível. Não declarar revogação global ou entrega sem evidência. Estado do AP usa somente evidência da fonte acordada e não implica acesso regional ou sync concluído. Após reconexão, revalidar autorização/retenção, reconciliar eventos sem saltar lacunas e manter desconhecidos visíveis.

### Invariante

Operação local permitida não autoriza mudança global de ACL/posse/revogação. Grant cuja atualidade não se demonstra não é presumido válido. pending/unknown não equivale a revogação confirmada; cópia de estado desconhecido não serve dado sensível. AP e região são estados distintos.

### Negativo

Sem política efetiva, autoridade ou atualidade, bloquear persistência/replay/divulgação sensível segundo política documental e informar estado/motivo. Sem AP ou sem fonte de status, reportar indisponível/desconhecido sem inferir falha regional. Definir significado de AP antes de fechar o cenário.

### Degradação e recuperação

Durante partição, permitir apenas operações locais explicitamente autorizadas e não avançar escritas fortes sem autoridade. Após reconexão, aplicar revogações antes de servir cópias restauradas, recuperar lacunas via cursor/replay e manter bloqueio se autorização, retenção ou snapshot não puderem ser provados. Sem SLA ou duração definida.

### limiares

Nenhum prazo de tolerância offline, frescor, disponibilidade ou recuperação foi aprovado.

### principal documental

**nome**: Alex Crichton

**estado**: proposto na matriz; parcialmente adequado ao estado offline do cliente

**conflitos ou interfaces**: - Recomendação sem transferência: Martin Kleppmann para sync/revogação

- Leslie Lamport para partição/autoridade regional

- Jouni Malinen para AP Wi-Fi

- Alex para estado local do cliente

### evidencias

- **arquivo**: docs/00-produto/requisitos.md; **secao**: Critérios adicionais não funcionais; **linha**: 85; **trecho**: RNF-017: fluxo offline, revogação e disponibilidade de AP testados sob partição regional.

- **arquivo**: docs/01-arquitetura/replicacao.md; **secao**: Autoridade de escrita e isolamento; **linhas**: 54-55; **trecho**: Regiões offline não podem alterar posse, ACL ou revogações globais sem autoridade; podem registrar eventos locais permitidos.

- **arquivo**: docs/04-seguranca/retencao-revogacao.md; **secao**: Regras para dados offline, retries e replay; **linhas**: 94-99; **trecho**: Falha de rede, emissor, nó de sincronização ou controlador produz estado `unknown`/pendente e bloqueia leitura/divulgação sensível; nenhuma tentativa de best effort amplia acesso.

### aplicabilidade

**produtos**: - Personal

- Teams

- Node

- Platform/SDK quando participa de fluxo offline/revogação

**plataformas**: - Clientes móveis e desktop no escopo integral

- Serviços regionais

- AP Wi-Fi gerenciado quando aplicável

**nao aplicavel**: Para produto sem AP gerenciado, justificar somente a subparte de disponibilidade do AP; fluxo offline, revogação e partição permanecem aplicáveis. Limites de SO são específicos da plataforma e não determinam estado global regional.

### decisoes pendentes

- **decisao**: Definir AP como ponto de acesso Wi-Fi e escolher fonte autoritativa do status.; **alternativas**: - Estado informado pelo SO local; - Estado atestado pelo controlador autorizado; - Ambos com escopo e atualidade separados; **recomendacao**: Especificar fonte, escopo, frescor e incerteza; não inferir status ausente.; **consequencia**: Sem definição, o subcritério de disponibilidade de AP não é verificável.

- **decisao**: Definir operações offline permitidas, atualidade de autorização e comportamento durante revogação isolada.; **alternativas**: - Bloquear leitura sensível sempre que atualidade não for demonstrável; - Permitir apenas dados/ações locais explicitamente cobertos por política e validade aprovadas; **recomendacao**: Falhar fechado para leitura sensível sem prova de atualidade; permitir somente operações locais explicitamente autorizadas.; **consequencia**: Sem decisão de política/tempo, fluxos e testes não podem receber critério fechado.

### dependencias

- Martin Kleppmann: eventos, replay e semântica de revogação

- Leslie Lamport: partição e autoridade

- Jouni Malinen: AP Wi-Fi e evidência de disponibilidade

- Política de retenção/revogação aprovada

- Alex: estado offline do cliente

### criterio fechamento documental

Registrar responsáveis transversais sem transferir atribuição, definir significado/fonte de AP, operações locais e regras de autorização/frescor, estados de partição/reconexão e vínculos a critérios futuros; revisão com principais de sync, partições e Wi-Fi.

### estado

aceite proposto; requisito não funcional; responsável atual parcialmente adequado; implementação não verificada

### futura spec

NOT_ASSIGNED

### caso teste

NOT_ASSIGNED

### observado

NOT_EXECUTED

Futura SPEC: **NOT_ASSIGNED**. Caso de teste: **NOT_ASSIGNED**. Observado: **NOT_EXECUTED**.

## conciliacao-lea

### Conciliação de responsabilidade — Lea Verou

**Resposta:** confirmo a proposta de Alan para integrar documentalmente RF-033, RF-042 e RNF-016 no recorte de UX/jornada. Isso não aprova política ou operação nem substitui responsáveis técnicos por plataforma, SOS ou estado de domínio.

| ID | Decisão | Limite e interfaces |
|---|---|---|
| RF-033 | **Confirmo como integradora documental de UX/cliente.** | A jornada de prioridade e feedback em conexão restrita pertence à interface. Nancy preserva a salvaguarda de não suprimir SOS nem substituir funções oficiais; Alex cobre estado local/energia; Vint conectividade; Chet Android e Donny Apple. Essa divisão não atribui à UX a política de rede ou operação. |
| RF-042 | **Confirmo como integradora da jornada multiplataforma.** | Abrange iOS e Android. Donny mantém subescopo Apple/ActivityKit; Chet mantém Android/Live Updates e compatibilidade; Nancy revisa significado e riscos de estados SOS. Lea integra consistência de UX, conteúdo, fallback e acessibilidade. Donny confirma somente o recorte Apple e recomenda explicitamente Lea+Chet para a abrangência completa. |
| RNF-016 | **Confirmo como integradora de UX da regra de projeção honesta.** | A regra de não apresentar confirmação sem evidência é comum à experiência. Donny cobre Apple, Chet Android e Nancy a semântica SOS/ACK humano. Martin/Graydon/Alex são interfaces necessárias para fonte, ordenação e persistência do estado. Lea não define estados autoritativos nem aprova a semântica de ACK. |

### Evidência e fontes

- `CONCILIACAO-RESPONSABILIDADES.md`, linhas 1–31: a integração é uma proposta de coordenação; as confirmações estavam pendentes e não há designação operacional. Para RF-033, a proposta dá UX a Lea e subescopos a Alex, Vint, Chet/Donny e Nancy. Para RF-042/RNF-016, propõe integração Lea, Apple Donny, Android Chet e salvaguarda SOS Nancy.
- `docs/00-produto/requisitos.md`: L62 RF-033; L76 RF-042; L84 RNF-016. Esses requisitos são direção aprovada, não prova de implementação.
- `contribuicoes/requisitos-nancy.md`, §§ “Cobertura e confirmação” e “RF-033”, linhas 3–8 e 104–114: Nancy registra conflito de escopo de RF-033, recomenda Lea como principal de cliente/UX e mantém sua revisão da invariante SOS.
- `contribuicoes/requisitos-donny.md`, “Escopo e confirmação”, §§ RF-042/RNF-016: Donny confirma apenas Apple e recomenda Lea para jornada, Chet para Android e Nancy para semântica SOS. O vínculo histórico de RF-042 à ADR-052 está documentado; não atribuo ADR a RNF-016.
- `CONTEXTO-E-PAPEIS.md`, linhas 35, 73–84: Lea lidera UX; Chet cobre Android/Kotlin; Donny Apple; Nancy segurança operacional SOS; Alex estado offline; Martin sincronização durável.
- `contribuicoes/chet.md`, §§ “Síntese” e “Responsabilidade documental proposta”: cobre Android e menciona RF-042 entre as interfaces do plano. Não encontrei um arquivo `contribuicoes/requisitos-chet.md` nem confirmação de aceite de Chet por RF nesta rodada.

### Delta de aceite necessário

Não reescrevo os aceites existentes. Ao consolidar:

- **RF-033:** mostrar estado da conexão/serviço e elegibilidade sem declarar transmissão; tornar visíveis redução/pausa de cargas opcionais e recuperação sem duplicar efeitos; não orientar a desligar funções oficiais de emergência nem sugerir controle de apps terceiros. P0–P3 são prioridades documentadas, não limiares ou SLI medidos.
- **RF-042:** especificar a jornada comum e diferenças de capacidade iOS/Android; Live Activity/Live Update elegível versus fallback no app; atraso/stale, dispensa/expiração e retorno ao app; acessibilidade além de cor/movimento e privacidade no lock screen. Não assumir paridade ou elegibilidade uniforme.
- **RNF-016:** ligar evento/evidência ao rótulo de cada superfície; manter estado desconhecido/stale quando faltar evidência; aceitação técnica não vira atendimento humano, entrega final ou resolução; perder a projeção não muda o estado de domínio; fallback/recovery consulta a fonte autoritativa.

Esses deltas integram UX com os contratos já citados nas contribuições de Nancy e Donny e com `aplicativo-operacao-normal.md`, `conectividade-contingencia-satelite.md`, `live-presence.md`, `07-contratos/live-presence.md`, eventos/sync e SOS. Semântica final, ordenação, idempotência, stale e limiares continuam dependentes dos responsáveis de domínio e de decisões abertas. Nenhum parâmetro foi inventado.

### Lacunas, confirmações e não atribuições

- **Confirmados por mim nesta resposta:** RF-033, RF-042 e RNF-016 como integradora documental de UX, com os subescopos acima. Isso ainda requer conciliação de Alan na matriz e não equivale à aprovação do usuário.
- **Subescopo Android:** adequado a Chet conforme distribuição e contexto; Chet tem contribuição geral `chet.md`, mas falta confirmação individual dos aceites RF-042/RNF-016 numa entrega `requisitos-chet.md`. Lacuna administrativa para Alan acompanhar, não razão para omitir Android.
- **Apple:** Donny confirma o próprio escopo Apple, não a integração multiplataforma.
- **SOS:** Nancy mantém revisão de semântica e salvaguardas, não a integração completa da interface.
- **RNF-001:** fora da minha parcela. A origem diz “IPv6-first sem prejudicar acessibilidade” e seu critério enumera IPv6/IPv4/QUIC/TCP/relay, o que sustenta interpretação de alcançabilidade de rede. A palavra pode ser ambígua; não a redefinir silenciosamente como acessibilidade de UI. Se esse sentido também for pretendido, registrar complemento de UI distinto e pedir decisão de produto.

Nenhuma política/rotina operacional, SPEC ou teste foi aprovada/criada. Execução `NOT_EXECUTED`; implementação e suporte real por aparelho não verificados. Esta contribuição não altera matriz, documentos canônicos ou arquivos de outros autores.


## conciliacao-fowler

### Conciliação de responsabilidade — Fowler

Data: 2026-11-16. Estado: confirmação da parcela de integração documental; não é designação operacional, aprovação normativa, SPEC ou validação.

### Escopo conferido

Li `CONCILIACAO-RESPONSABILIDADES.md`, `contribuicoes/requisitos-fowler.md` e as contribuições pertinentes de Graydon, Jack, Vint e Brendan. A conciliação pede manter Fowler como integrador de RF-024 e RNF-008, com subescopos. **Confirmo ambos como integrador documental**: RF-024 cobre a arquitetura da evolução/negociação e fallback; RNF-008 cobre coerência de compatibilidade de protocolo/firmware e rollout. Isso preserva minha contribuição de aceites e não reivindica autoria isolada de wire, firmware, transporte ou operação.

A atribuição original da matriz (matriz-rastreabilidade.md, linhas 110 e 120) ainda era “proposto por especialidade, confirmação por requisito pendente”. Minha confirmação resolve apenas a participação de Fowler; Alan deve consolidar o principal e registrar as interfaces. Nenhum proprietário operacional ou de plantão é designado.

### Responsabilidades de subescopo

- **Graydon / GMP:** contrato de negociação, versões/features, erros e conformidade da API/wire. A contribuição requisitos-graydon.md confirma domínio do contrato GMP; não transfere integração arquitetural.
- **Vint / transporte e rotas:** compatibilidade de caminhos IPv6/IPv4/QUIC/TCP/relay e degradação de conectividade. Sua contribuição restringe essa participação a critérios de rede; não redefine negociação GMP.
- **Jack / firmware:** versão e recuperação dos dispositivos que realmente têm firmware. A contribuição requisitos-jack.md é a interface necessária para RNF-008; firmware não se aplica a peer sem firmware gerenciado.
- **Brendan / rollout/rollback operacional:** sinais, gates e evidência de canary/rollback. A contribuição requisitos-brendan.md trata planejamento operacional, sem executar serviço/CI nesta rodada.

As interfaces são complementares. Se houver sobreposição textual futura, manter uma semântica normativa de versão/perfil e referenciar subcontratos; não duplicar enums ou janelas de compatibilidade em cada camada.

### Delta necessário nos aceites

O aceite RF-024 já cobre negociação compatível, rejeição de major/feature incompatível, fallback permitido e proteção contra downgrade. Para refletir a conciliação, declarar explicitamente a decomposição: (1) negociação e erros GMP por Graydon; (2) elegibilidade e mudança de transporte por Vint; (3) seleção arquitetural/perfis comuns por Fowler. Fallback não deve acionar bridge externa como se fosse transporte equivalente nem alterar semântica GMP; permanece condicionado a perfil autorizado.

O aceite RNF-008 já cobre N/N-1, incompatibilidade, pausa de canary e rollback seguro. Acrescentar que N/N-1 deve ser definido separadamente para protocolo, schema e firmware; Jack valida a dimensão de firmware em planejamento, Brendan define evidência/gates de rollout e Fowler integra a matriz de compatibilidade. “Canary passou” e “rollback seguro” precisam de critérios definidos pelos owners de operação, sem criar valores nesta conciliação.

### Decisões e limites

Permanecem abertas as versões suportadas, definição de N/N-1, feature crítica/opcional, política de downgrade, fallback, critérios de canary/abort/rollback e aplicabilidade do firmware por classe de dispositivo. Recomendo tabela versionada por perfil com dimensões separadas; consequência: sem aprovação dos perfis e critérios operacionais não se afirma interoperabilidade nem se encerra o aceite.

Não confirmo/recuso outros IDs da conciliação. Nenhuma política, jurisdição, topologia, janela ou limiar foi aprovada. SPEC/testes permanecem NOT_ASSIGNED; execução NOT_EXECUTED. Nenhum canônico ou contribuição alheia foi editado.


## conciliacao-jouni

### Conciliação documental — Jouni Malinen

### Confirmações

Confirmo RNF-013 como principal integrador documental proposto. O requisito cobre inventário Wi-Fi privado, não divulgação de scans/senhas e prova auditável de autoridade sobre AP. Esses fluxos são centrais à atribuição Wi-Fi de Jouni em `DISTRIBUICAO.md` e aos aceites existentes RF-035, RF-036 e RF-040. Bruce Schneier permanece revisor de privacidade, segredo, auditoria e controles de publicação. Esta confirmação não transfere nem altera a matriz canônica e não designa autoridade operacional.

Confirmo também a recomendação de responsabilidade para RNF-014. Jouni integra a semântica Wi-Fi de solicitação, associação, autenticação do AP e validação de Internet; Graydon Hoare deve revisar eventos e contrato GMP, com as equipes de conectividade/cliente responsáveis pelos sinais próprios de suas plataformas. Isso concilia os nomes e evidências sem criar enum ou política nova.

### RNF-013: delta de aceite necessário

`requisitos.md:81` contém a frase “não publicar scans Wi-Fi nem senhas por padrão; provação de titularidade auditável”. O aceite de Bruce em `contribuicoes/requisitos-schneier.md`, §§ “RNF-013” e “Conflito de responsável”, linhas 114–132, já propõe estados de prova, negação sem autoridade, segredo fora de logs/respostas/mapa, privacidade offline e recuperação. Não reescrevo esse aceite. Minha confirmação adota seu reparto: Jouni integra o fluxo completo; Bruce revisa privacidade e auditabilidade.

Delta restante para a revisão documental: esclarecer com o dono do requisito se “provação” significa “prova de titularidade”, como sugere o contexto, antes de editar a origem; confirmar se um requisito deve conservar junto o controle de não publicação e o processo de autoridade. Até essa decisão, usar “prova de autoridade” apenas como interpretação provisória no texto de planejamento, sem corrigir silenciosamente o requisito. Depois, manter concordância entre `wifi-autodiscover-autoshare.md` §§3–4, `wifi-compartilhamento-privacidade.md` §§2 e 5, API §2 e `testes-wifi.md` seções de descoberta e titularidade. A prova deve resultar em decisão auditável por referência opaca, nunca em segredo ou prova reutilizável no log.

### RNF-014: crosswalk de termos existentes

Fontes conferidas:

- `docs/00-produto/requisitos.md:82` usa `requested`, `os_associated`, `ap_authenticated` e `internet_verified`.
- `docs/02-dominio/wifi-registro-acessos.md:13–19` define eventos `connect.requested`, `os.associated`, `ap.authenticated`, `ap.session.started`, `ap.usage.reported` e `ap.session.ended`, com fonte e evidência diferentes.
- `docs/07-contratos/wifi-autodiscover-autoshare-api.md:86–88` enumera estados `requested`, `os_deciding`, `os_approval_required`, `associating`, `associated`, `local_only`, `captive_portal`, `internet_verified`, entre outros. O contrato veda inferir estados posteriores.
- `docs/00-produto/requisitos.md:71` e `docs/03-integracoes/wifi-android-ios-roteadores.md:5–19` exigem que AutoConnect respeite a escolha/permissão do SO em Android e aprovação do usuário no iOS.

| Rótulo do RNF-014 | Evidência Wi-Fi existente | Relação proposta para conciliação |
|---|---|---|
| `requested` | Estado API `requested`; evento Access Registry `connect.requested` | Mesmo estágio semântico: solicitação do app. O evento registra a ocorrência; não afirma associação. |
| `os_associated` | Evento `os.associated`; estado API `associated` | A associação é uma observação do app/SO. O domínio pode mapear o evento ao estado API atual `associated`, desde que preserve fonte e nível de evidência. Não criar um novo estado `os_associated` por esta contribuição. |
| `ap_authenticated` | Evento `ap.authenticated`, fonte roteador/RADIUS | Evidência do controlador de autenticação aceita; não é hoje um estado na enumeração API de conexão. Preservar como evento/evidência, sem acrescentar enum até Graydon e Jouni reconciliarem o modelo. |
| `internet_verified` | Estado API `internet_verified` e resposta exemplificativa com booleano | O estado existe, mas contrato ainda não define probe, serviço-alvo, instante/frescor nem o que fazer quando a verificação falha. Não equiparar autenticação do AP a Internet. |

Proposta de ajuste do aceite (planejamento):

> Para uma sessão identificada, registrar `connect.requested` quando o app solicita conexão. Registrar `os.associated` somente quando o app/SO observar associação e projetar o estado existente `associated` com origem e evidência preservadas. Registrar `ap.authenticated` somente após evento autenticado do controlador/RADIUS; não convertê-lo em novo estado de conexão sem decisão de contrato. Usar `internet_verified` somente depois que o contrato definir e executar a verificação de serviço correspondente. Ausência de evidência mantém o resultado desconhecido ou pendente. Solicitação, associação, autenticação e Internet não se inferem entre si.

O texto mantém quatro degraus sem transformar evento em enum. A matriz pode continuar usando os quatro termos do RNF como rótulos de aceite. A tabela de eventos e os estados de API devem ser reconciliados num contrato futuro antes de SPEC. Em particular, `associated` não implica que o controlador autenticou, e `ap.authenticated` não implica que houve Internet utilizável.

**Lacunas que continuam abertas:** semântica e fonte de verificação para `internet_verified`; estado quando captive portal, `local_only` ou serviço indisponível; validade/frescor do resultado; transições e eventos fora de ordem; se a resposta da API precisa expor o nível de evidência; convenção final para termos de estado versus evento. Não proponho enum novo, timeout, probe, limiar ou versão de plataforma.

### Evidência e limite

Li a conciliação, o aceite de RNF-013 escrito por Bruce, meu aceite anterior de RNF-014 e somente os trechos das fontes canônicas acima necessários à decisão. Não reiniciei a revisão dos 60 requisitos, não alterei arquivos de outros autores ou canônicos, e não enviei mensagens cruzadas. A decisão da matriz e a revisão independente continuam com Alan/Edsger. Os contratos permanecem propostas documentais; não foram criados SPECs nem casos executáveis e não houve execução ou validação de produto.


## conciliacao-margaret

### Conciliação de responsabilidade — Margaret Hamilton

**Decisão:** confirmo **RF-028 — Guardian integral** como principal documental de produto para jornada e critérios de aceitação. Nancy Leveson permanece interface/revisora da fronteira entre atraso e SOS; não transfiro a responsabilidade sobre semântica operacional SOS. Bruce Schneier (privacidade), Lea Verou (UX), Paul Ramsey (localização) e responsáveis por plataforma/domínio são interfaces, conforme necessário. Esta confirmação é documental: não nomeia owner operacional, plantão, nem aprova política.

### Base e evidências

- `docs/06-planejamento/matriz-rastreabilidade.md`, seção “Cobertura individual dos requisitos — integração de planejamento 2026-11-16”, linha 126: a proposta de conciliação identifica RF-028 para Margaret e descreve Nancy como revisora da separação overdue/SOS. A linha da cobertura individual associa RF-028 à especialidade de Nancy atualmente; a distribuição propõe Margaret como integradora de produto.
- `docs/00-produto/requisitos.md:57`: RF-028 — “Guardian: convite, prazo, rastreamento autorizado temporário, revogação, check-in e aviso de atraso sem SOS automático”; estado APROVADO, que não comprova implementação (requisitos.md:3).
- `docs/02-dominio/guardian-acompanhamento.md:3–20,22–34,47–51`: especificação de produto; compartilhamento voluntário, estados/convite/escopo/precisão, revogação e Beacon; `overdue` é aviso de check-in e não SOS; critérios de aceite nomeiam fim automático, revogação imediata de novas leituras, autorização e distinção overdue/SOS.
- `docs/07-contratos/guardian-crowdfind.md:1–3,8–15,40–47` (referência já conferida na contribuição de Nancy): contrato lógico conceitual; grants não são permanentes e evento de atraso não equivale a SOS. Sua versão/estado não é contrato executável.
- `Shared/GodMesh/CONTEXTO-E-PAPEIS.md`, §§Responsabilidades, Fronteiras de colaboração e Cobertura transversal consolidada: Margaret lidera requisitos/jornadas e decisões de produto; Nancy trata SOS/perigo operacional; o contexto não designava antes um principal funcional nominal para Guardian.
- `Planejamento-Integral-2026-11-16/contribuicoes/requisitos-nancy.md:15,92–102`: análise fundamentada que identifica conflito de escopo, recomenda Margaret para ciclo de vida Guardian e reserva a Nancy `overdue ≠ SOS`; detalha cenários e lista decisões pendentes. A recomendação agora está confirmada por mim.

**Observação sobre localização da linha:** a proposta de conciliação relaciona RF-028 na tabela de `CONCILIACAO-RESPONSABILIDADES.md`; a atual matriz canônica da cobertura, linha 126, ainda reflete seu próprio principal provisório conforme verificação local. Não trato a conciliação como aplicada à matriz.

### Delta necessário no aceite antes de considerá-lo documentalmente completo

A contribuição de Nancy já fornece um critério comportamental amplo para RF-028. Não o reescrevo. Para a integração de produto, acrescentar/assegurar somente estes elos explícitos no registro:

1. **Atores e confirmação de convite:** separar owner, destinatário convidado e destinatário que aceitou; especificar que nenhum convite pendente/recusado concede leitura e que destinatário vê finalidade, duração, escopo e precisão concedidos antes da aceitação.
2. **Transições de ciclo de vida observáveis:** por evento (agendar/ativar/pausar/retomar/concluir/expirar/revogar), indicar o estado esperado da sessão e do grant de cada destinatário, quem pode iniciá-lo e que estado recebe o destinatário. Precedência para duas alterações concorrentes e múltiplos dispositivos ainda depende de decisão/contrato; não inventar regra.
3. **Limite de dados após encerramento:** confirmar que consultas novas são negadas após expiração/revogação/conclusão em cada caminho coberto e separar isso da retenção/purga de observações já recebidas. O instante/limite de propagação da revogação durante partição, retenção e comportamento de cache permanecem sem valores/contrato aprovados; devem ficar explicitamente pendentes, sem chamar “imediata” uma garantia não definida.
4. **Distinguir estados do participante:** check-in confirmado, `overdue`, localização stale/offline, falha de permissão e SOS explícito devem ter resultados visuais e notificação diferentes; atraso, ausência de conectividade, falta de resposta ou dado antigo nunca iniciam SOS nem alegam ajuda confirmada.
5. **Aplicabilidade por plataforma e produto:** manter jornadas Owner/recipient em Personal e Teams, sem consentimento substituído por administração empresarial. Registrar Node/Platform/SDK apenas onde expõem ou processam a sessão por serviço autorizado. Por cliente/plataforma, declarar capacidades de atualização em background, precisão e notificação; desktop/web podem gerenciar/consultar segundo suas capacidades e não presumir coleta contínua. Ausência de sensor não elimina convite, grant, prazo, revogação e apresentação do estado.

Esses pontos são refinamentos de escopo/observabilidade comportamental, não novas políticas nem autorização para alterar os contratos. O aceite de Nancy cobre atraso, revogação, offline/stale e aplicabilidade geral; continuam abertos parâmetros e regras de concorrência indicados na fonte. Não associei GMD/PI por semelhança nominal nem criei RF/RNF.

### Decisões e pendências que permanecem

- Produto/usuário: papéis e semântica final de convite; defaults de duração, tolerância, check-in e frequência; uso de Guardian em Teams e política de retenção/acesso, com alternativa/recomendação a registrar antes de aprovar.
- Donos de domínio/dados/sync: relógio/fuso e eventos tardios; concorrência entre dispositivos; cache, partição e efetividade/prova de revogação; relação entre grant, precisão e Beacon.
- Plataformas: capacidades reais e permissões de background, atualização e notificação por versão de SO/hardware, com evidência futura; sem generalizar disponibilidade entre iOS, Android, macOS, Windows, Linux e Web.
- Segurança/privacidade/legal: finalidade, audiência, consentimento e retenção por jurisdição; não converter recomendações em aprovação.
- Operação: owner humano, escala ou atendimento não decorrem deste requisito. SOS permanece distinto e não operacional para emergências reais sem procedimentos/acordos/avaliação conforme `servico-sos.md`.

**Estado de fechamento:** RF-028 confirmado para liderança documental de produto, com Nancy como subescopo de risco/SOS; critérios ainda propostos e dependências acima abertas. SPEC e testes futuros permanecem `NOT_ASSIGNED`; execução `NOT_EXECUTED`; implementação não verificada. Não alterei a matriz, contribuição de outro autor, docs canônicos nem executei produto, testes ou infraestrutura.

**Limite da parcela:** RF-020 e RF-027 são atribuições minhas na cobertura de requisitos, mas não são linhas desta conciliação focal; não reabri seus critérios. RNF-001 não é pertinente a RF-028; portanto não redefino “acessibilidade” ou “acessibilidade de rede” nesta resposta.


## conciliacao-chet

### Conciliação Chet Haase — RF-042 e RNF-016

### Confirmação de responsabilidade

Confirmo meu subescopo documental **Android** em `RF-042` e `RNF-016`, sem assumir a integração multiplataforma nem a parcela Apple. Concordo com Lea Verou como principal integradora de UX/jornada nos dois requisitos, Donny Wals no subescopo iOS/ActivityKit, Chet Haase no subescopo Android/Live Updates e Nancy Leveson na revisão dos estados e das salvaguardas de SOS. Alex Crichton continua interface para estado offline/fonte do evento. Esta confirmação não designa operação, plantão, política ou owner funcional.

Não há ID recusado dentro do meu subescopo. Não assumo como principal completo `RF-042` ou `RNF-016`, nem `RF-043`, nem a decisão sobre a semântica de ACK/SOS, a tecnologia Android final ou a ordem de implementação.

### Evidências e confronto dos aceites

- `CONCILIACAO-RESPONSABILIDADES.md`, linhas 15–16: propõe Lea como integradora de RF-042/RNF-016, com Chet Android, Donny Apple e Nancy para semântica SOS. O estado de confirmação de Chet ainda estava pendente.
- `contribuicoes/requisitos-donny.md`, linhas 5–9, 61–82 e 84–89: Donny limita sua confirmação à parcela Apple; afirma que a aceitação de RF-042/RNF-016 deve incluir Chet Android, Lea UX e Nancy para SOS. Seus critérios propõem projeção por evidência, fallback, estado `stale`, lock screen privado, dispensa e validação por plataforma. Aceito esse enquadramento; minha confirmação não altera os critérios Apple.
- `contribuicoes/lea.md`, linhas 24–26 e 43–53: propõe matriz de capacidades e UX por plataforma, sem presumir paridade de APIs, e pede revisão de Chet/Donny. Isso é compatível com a divisão acima. O arquivo não atribui a Chet a integração completa desses requisitos.
- `contribuicoes/requisitos-nancy.md` e `.json`: Nancy recomenda principal de produto para Guardian e de UX para RF-033, com interfaces de plataforma; trata ACK/claim SOS como semântica de risco própria. Não transfiro ao meu escopo propriedade de RF-028, RF-033 ou ACK humano.
- `contribuicoes/chet.md`, linhas 43–61 e 91–104: PI-chet-002 propõe critérios Android para elegibilidade Live Update, início/fim, estado `stale`, dispensa sem repostagem, fallback e validação futura. PI-chet-001 cobre Android background/Guardian e preserva A30-001 em aberto. Esses são aceites propostos, não aprovados nem executados.
- `docs/00-produto/requisitos.md`, linhas 76–77 e 84: RF-042 exige Live Presence iOS/Android com SOS/Guardian/PTT, estados reais e privacidade na tela bloqueada; RNF-016 veda anunciar confirmação sem evidência. Ambos estão aprovados como requisitos, não validados como produto.
- `docs/01-arquitetura/live-presence.md`, linhas 23–29 e 40–51; `docs/05-operacao/testes-app-live-presence.md`, linhas 14–24: especificam projeção, `stale`, privacidade, elegibilidade Android, fallback e cenários de teste. O segundo arquivo é matriz futura, sem resultados executados.

### Delta necessário no aceite integrado

Para compor o aceite único sem duplicar as parcelas, manter o corpo de Donny para iOS e incorporar o subcritério Android de `PI-chet-002`:

1. Aplicar Live Update somente a atividade em andamento, iniciada pelo usuário e sensível ao tempo, com começo e término definidos e elegibilidade confirmada pelo Android/dispositivo.
2. Usar o último evento de domínio autenticado e permitido como fonte da projeção. Atualização ausente ou velha produz `stale`; notificações não avançam estado técnico nem humano de SOS.
3. Respeitar dispensa e preferência de notificação sem repostar indevidamente. Quando inelegível, indisponível ou dispensada, conservar acesso ao estado no app e usar fallback permitido, sem prometer exibição uniforme.
4. Testar futuramente combinações SOS/Guardian/PTT, tela bloqueada, rede ausente, evento atrasado, permissão revogada, encerramento/retomada do processo, dispensa e fallback em modelos/versões Android cobertos. Registrar evento de origem redigido, versão, modelo, elegibilidade, estado esperado/observado e motivo de fallback.
5. Não fixar thresholds de frescor ou latência nesta conciliação. A fonte de estado por tipo, janela de `stale`, elegibilidade mínima e conjunto de aparelhos continuam dependentes de decisão/contrato e devem permanecer abertos até aprovação.

O critério integrado fecha documentalmente quando a matriz registra Lea como integradora de UX, Donny/iOS e Chet/Android como subescopos aceitos, Nancy como revisora da semântica SOS, e cada plataforma possui fonte do estado, elegibilidade, dados permitidos, stale, término, dispensa, fallback e critérios futuros. SPEC e teste permanecem `NOT_ASSIGNED`; execução permanece `NOT_EXECUTED`. Nenhum aceite aqui declara RF-042 ou RNF-016 implementado ou validado.

### Limites

Li a conciliação, as contribuições requisitos de Donny e Nancy, a contribuição UX de Lea, minha contribuição anterior e as fontes canônicas diretamente ligadas a RF-042/RNF-016. Não editei a matriz, os critérios de outro autor nem documentos canônicos. Não criei SPEC, código ou testes; não usei aparelhos. A primeira implementação futura continua conforme a ordem já estabelecida, sem antecipar Android.


## conciliacao-kleppmann

### Conciliação de responsabilidade documental — Martin Kleppmann

Data: 2026-11-16. Confiro a proposta de Alan para integrar **RF-011, RNF-009 e RNF-017**, com os subescopos abaixo. Esta confirmação é de responsabilidade documental transversal; não designa operação, plantão, aprovação normativa nem transfere outros donos. Não reescrevo os aceites da matriz e não altero documentos canônicos ou contribuições anteriores.

### Confirmação por requisito

| ID | Decisão de responsabilidade | Fundamento e limite |
|---|---|---|
| RF-011 | **Confirmo Martin Kleppmann como integrador documental.** Nancy Leveson conserva a semântica de segurança operacional/ACK humano de SOS; Graydon Hoare, contratos de serviço e estados wire; Lea Verou, apresentação/UX. | O requisito combina mensagens, presença, notificações, SOS e ACKs por etapa (`00-produto/requisitos.md:17`). Eventos e sync fornecem o crosswalk comum para durabilidade e estados, mas não absorvem a semântica de domínio SOS, política de notificação ou UX. O próprio parecer de Nancy recomenda Martin para o requisito geral e ela como interface SOS. |
| RNF-009 | **Confirmo Martin Kleppmann como integrador documental.** Nancy mantém interpretação SOS e reconhecimento humano; Graydon mapeia serviço/GMP/wire. | O requisito exige rastreabilidade de “ACK persistido, recebido, confirmado e reconhecido” (`requisitos.md:45`). Isso atravessa registro durável, tentativas e ACK humano. C10 §2 separa `Command.lifecycle`, `DeliveryAttempt.state` e `HumanAcknowledgment.state`; não deve haver equivalência automática entre esses eixos. Nancy identifica explicitamente a amplitude de SOS e comandos e recomenda Martin como principal transversal. |
| RNF-017 | **Confirmo Martin Kleppmann como integrador documental para sync/offline/revogação em partição.** Alex Crichton conserva persistência/estado local e FFI; Leslie Lamport, autoridade/partição; Jouni Malinen, contrato/fonte de disponibilidade do AP; Bruce Schneier/Kim Cameron, política e autorização/revogação. | O texto fonte pede fluxo offline, revogação e disponibilidade de AP sob partição regional (`requisitos.md:85`). A contribuição de Alex descreve eventos, checkpoint, recuperação e a distinção entre autorização e conectividade, e recomenda Martin como interface de sync, sem reivindicar integração exclusiva. AP permanece subescopo: não tratar disponibilidade do AP como prova de serviço regional nem de sync. |

Não identifico conflito que justifique recusar ou transferir esses três IDs. A proposta da conciliação é adequada **desde que os subescopos permaneçam explícitos** e os aceites não tratem sync como responsável único por políticas de SOS, credenciais, AP ou persistência cliente.

### Fontes confrontadas

- `Planejamento-Integral-2026-11-16/CONCILIACAO-RESPONSABILIDADES.md`, tabela RF-011/RNF-009/RNF-017 e seção “Outras interfaces a conciliar”: proposta de Alan e interfaces declaradas.
- `Planejamento-Integral-2026-11-16/contribuicoes/requisitos-nancy.md`, §§Cobertura e confirmação e critérios RF-011/RNF-009: Nancy registra conflito de escopo de SOS+comandos e propõe Martin como principal geral, mantendo semântica SOS/ACK humano.
- `Planejamento-Integral-2026-11-16/contribuicoes/requisitos-alex.md`, §RNF-017: Alex fornece aceite de partição, revogação e AP, separa estado de AP de estado regional e aponta dependências Martin/Leslie/Jouni/Alex.
- `docs/00-produto/requisitos.md:17,45,85`: enunciados e respectivos estados/critério de origem.
- `docs/02-dominio/eventos.md`, “Estados de um comando”, “Ingestão” e “Backpressure”: separa estado lógico, tentativas, ACK e dedupe; Inbox precede checkpoint; retries/backpressure não equivalem a entrega.
- `docs/07-contratos/eventos-sincronizacao.md` §§2, 3, 5, 6, 7 e 9, incluindo §4.1.3: crosswalk proposto de lifecycle/attempt/human acknowledgment, idempotência, cursor/replay, retenção, Inbox/materialização, falhas e integração de dispatch. O contrato continua proposta v0.1; códigos e retenção não são aprovados.
- `docs/01-arquitetura/replicacao.md` §§Fluxo, Snapshot + replay, Deleções e retenção e Autoridade de escrita: ACK é prefixo Inbox, lacunas não avançam checkpoint, replay revalida autorização e políticas ausentes bloqueiam replay persistente.
- `docs/02-dominio/servico-sos.md` §§Estados independentes e encaminhamento: ACK de rádio/MQTT/websocket/push não equivale a endpoint nem a reconhecimento humano; sem claim não há assunção humana comprovada.
- `docs/06-planejamento/matriz-rastreabilidade.md`, seção de cobertura individual, linhas RF-011/RNF-009/RNF-017 e “Registro comportamental e cobertura de cenários”: atribuições ainda propostas; casos futuros não são SPECs ou execução.

### Delta de aceite necessário para a integração da matriz

Este delta é orientação a Alan/Ada para a conciliação sequencial; não substitui nem reescreve os aceites completos já produzidos por Nancy/Alex ou o desdobramento prévio de RF-013/RNF-004.

### RF-011

O aceite integrado deve observar mensagens, presença, notificações e SOS sem forçar uma máquina de estados única para todos: pré-condições incluem principal/tenant e audiência autorizados; estímulo cria/publica operação e injeta confirmação ou falha em cada etapa; esperado distingue persistido, recebido, submetido/aceito, confirmado por endpoint e reconhecido por pessoa quando esses sinais existem. Presença efêmera não ganha durabilidade por estar no mesmo requisito. Caso negativo: ACK de gateway/MQTT/push sem recibo final não vira `delivered` ou ACK humano. Degradação/recuperação: timeout pós-envio preserva identidade e estado desconhecido; reconexão deduplica e atualiza apenas com evidência nova. Política de qual etapa significa “recebido”/“entregue”, TTL e apresentação permanecem dependências de domínio, C07/C09/C08 e UX; sem inventar limiar.

### RNF-009

O aceite integrado deve vincular cada marco ao sujeito que o pode afirmar e à evidência durável correspondente: persistido requer commit/registro durável; recebido exige evidência de receptor/Inbox conforme perfil; confirmado exige recibo de endpoint autenticado quando o transporte oferece isso; reconhecido exige ação humana autorizada. Testar redelivery, resposta perdida, ator/tenant incorreto, protocolo sem ACK fim a fim e SOS sem claim. A invariante é não promover automaticamente evidência de uma camada para a seguinte, nem confundir estado de tentativa com lifecycle do comando ou claim SOS. C10 oferece o modelo lógico, mas vocabulário wire, alias, predicados de sucesso, retenção das evidências e compatibilidade continuam para C07/C08/C09 e decisão normativa quando mudar comportamento.

### RNF-017

O aceite integrado deve controlar uma partição regional na qual autorização/revogação muda na autoridade reconhecida e, independentemente, variar disponibilidade local do AP; depois restaura comunicação. Esperado distingue conectividade regional, disponibilidade do AP e frescor de autorização; cópia isolada sem prova de grant atual não serve conteúdo sensível; após reconexão, revogação é revalidada antes de serving e replay recupera lacunas sem salto. Negativo: AP alcançável não prova região/sync; serviço local não declara revogação global; estado `unknown` não vira `valid` nem `revoked` por inferência. Dependem de decisão: significado de “AP”, fonte autoritativa/frescor de AP, operações locais permitidas, validade/offline grace e retenção. Recomendo preservar estados separados e fail-closed para dados sensíveis quando atualidade do grant não puder ser provada, sem adotar duração de tolerância. Alex completa armazenamento local, Leslie autoridade, Jouni AP, Kim/Bruce autorização/retention.

### Rastreabilidade e estado

- **Confirmados por Martin:** RF-011, RNF-009 e RNF-017 como principal integrador documental, com os subescopos acima.
- **Recusados:** nenhum dos três. Não reivindico autoria exclusiva dos subescopos nem responsabilidade operacional.
- **Lacunas:** aceite comportamental integrado ainda depende de revisão de interfaces e de parâmetros/políticas. A matriz deve manter SPEC/caso `NOT_ASSIGNED` e observado `NOT_EXECUTED`; esta confirmação não altera estado de validação.
- **Limite:** não associei GMD/ADR individuais por inferência; não declarei API, código, plataforma ou implantação testados. A documentação de sync não prova implantação nem disponibilidade operacional.

