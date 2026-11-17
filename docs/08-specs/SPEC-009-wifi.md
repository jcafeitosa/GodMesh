# SPEC-009 — Wi-Fi AutoDiscover, AutoShare e AutoConnect

**Versão:** DRAFT-0.6
**Estado:** DRAFT / OPEN
**Autor:** Jouni Malinen  
**Revisor independente designado:** Richard Bejtlich  
**Data:** 2026-11-17

## Mudanças desta versão

- DRAFT-0.4 — casos por requisito e casos P desbloqueados (C3-1): nova subseção `### Casos por requisito (P/N/B/F)` ao final, com uma linha por requisito declarado (RF-035 a RF-041, RNF-013, RNF-014) e as quatro colunas preenchidas ou `n/a` justificado; casos P cujo esperado dependia de valor não decidido — janela N/N-1 de `WIFI-API-001` e prazo/gatilho da política de retenção de auditoria — reescritos para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID; caso de borda `C009-061-B` (ADR-061) acrescentado. Nenhum número, prazo ou enum é criado; nada é aprovado ou executado.
- DRAFT-0.3 — casos por decisão (resposta ao veto C3-1): nova seção “Casos por decisão (ADR-056…063)” com uma linha por decisão aplicável, ao menos um caso negativo e um de falha/recuperação por decisão, esperado observável e todo caso `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`; parâmetro `OPEN` resulta em negar/informar desconhecido. Nada é aprovado, alterado em decisão ou executado.
- ADR-060: cada perna que termine proteção declara `confidentiality=terminated`; TLS/DTLS/SRTP não são E2EE.
- ADR-058: toda resposta declara `ack_class` entre as seis classes normativas; ACK de bridge/adaptador não eleva estado a entrega ao dispositivo ou confirmação humana.
- Estado documental: DRAFT-0.3 / OPEN; revisão independente do delta pendente.

> Esta SPEC organiza contratos e critérios documentais para revisão. Não é aprovada, implementada ou validada. Os cenários são planejados e não foram executados. Pontos OPEN impedem considerar o documento pronto para implementação.

## 1. Escopo e rastreabilidade

Esta SPEC descreve descoberta e inventário privado, comprovação de administração, compartilhamento consentido, provisionamento, associação, autenticação em AP/controlador, verificação de conectividade, posição de AP, auditoria, idempotência e revogação.

| Requisito | Escopo nesta SPEC | Situação na fonte |
|---|---|---|
| RF-035 | Descoberta e inventário privado | APROVADO |
| RF-036 | AutoShare com autoridade, consentimento, política e revogação | APROVADO |
| RF-037 | Solicitação de associação e estados de acesso distintos | APROVADO |
| RF-038 | Posição do AP, origem, qualidade e histórico | APROVADO |
| RF-039 | Publicação exata do AP por opt-in específico | APROVADO |
| RF-040 | Registro de sessão e auditoria de acesso | APROVADO |
| RF-041 | APIs/GMP, escopo e idempotência | ESPECIFICAR |
| RNF-013 | Privacidade, autorização, minimização e auditoria Wi-Fi | Critério de origem; vínculo quantitativo/ADR não demonstrado |
| RNF-014 | Evidência e distinção entre solicitação, associação, autenticação e Internet | Critério de origem; método/frescor ainda OPEN |

Fontes dos IDs: `docs/00-produto/requisitos.md:69–75,81–82`; matriz: `docs/06-planejamento/matriz-rastreabilidade.md` seção “Rastreabilidade adicional (aplicativo e Wi-Fi)” e linhas RF-035–041/RNF-013/014. Aceites da matriz são propostas documentais. `NOT_ASSIGNED` e `NOT_EXECUTED` permanecem para casos e execução.

Fora do escopo: operar AP/controlador, emitir ou ler credenciais reais, acessar senhas salvas, implementar clientes/serviços, executar varredura, teste de rede ou provisionamento, determinar políticas legais/regionais, fixar fornecedor ou aprovar gasto.

## 2. Fontes, precedência e baseline

A arquitetura de referência e ADRs prevalecem, conforme `docs/README.md`, regras de precedência 1 e 7–10. Fontes Wi-Fi relevantes:

- `docs/01-arquitetura/wifi-autodiscover-autoshare.md`, GMWIFI-ARC-001 v1.0, 2026-11-15. Arquitetura aprovada; implementação e homologação pendentes. §§1.2 (invariantes), 3–5, 8–10.
- `docs/03-integracoes/wifi-android-ios-roteadores.md`, matriz de capacidades e limites por plataforma/controlador.
- `docs/02-dominio/wifi-modelo-dados.md`, especialmente versões, outbox, deduplicação, tombstones e posição de AP.
- `docs/02-dominio/wifi-registro-acessos.md`, papéis e leitura de auditoria.
- `docs/04-seguranca/wifi-compartilhamento-privacidade.md`, §§2–6, retenção e revogação por classe.
- `docs/07-contratos/wifi-autodiscover-autoshare-api.md`, WIFI-API-001 v1.0, 2026-11-15, proposta não implementada.
- `docs/05-operacao/testes-wifi.md`; `docs/06-planejamento/decisoes.md`; `docs/06-planejamento/pendencias.md`.

Contribuição de autoria lida: `SPECS-EM-ANDAMENTO/contribuicao-jouni-malinen.md`, SHA-256 `da081830214f2fa2f1e184b8d9c1ac6374190fa897bb133968d462de2a975532`, captura verificada em 2026-11-16. Pareceres: `contribuicao-richard-bejtlich.md`, SHA-256 `9ec8d93457c8a6802438667c089ee08dcf34e70e141953889f7a5e314bfc0945`; Richard revisou §§12–14 e considera os achados defensivos substancialmente descritos, mas parciais por parâmetros/owners/políticas OPEN. `contribuicao-dan-kaminsky.md`, SHA-256 `eabc96a2689bb60ec74949f8b1114a195cb1a247ddf1280b6832615ee946f609`; Dan recomenda revalidar autorização/escopo antes de revelar recibo de retry pós-revogação. Essas revisões não aprovam a SPEC.

## 3. Produtos, plataformas e limites de aplicação

A SPEC se aplica a clientes e serviços que implementem seus fluxos, sem alegar suporte idêntico em todas as plataformas. Servidor Linux processa APIs, políticas, catálogo e eventos segundo autoridade/escopo. Clientes móveis usam somente capacidades publicadas pelo respectivo SO. Controlador/AP só participa por integração autenticada e homologada. macOS desktop, Windows, Linux desktop e web podem administrar, consultar ou operar contratos remotos conforme permissões e decisões de produto; descoberta/associação direta depende de capacidades confirmadas da plataforma e não é presumida.

| Ambiente | Aplicabilidade documentada | Limite/OPEN |
|---|---|---|
| Servidor Linux | API, política, registro, mapa, eventos e integrações de controladores autorizados | implantação, região, capacidades de cada controlador e autoridade precisam ser definidas |
| macOS arm64/x86_64 | Cliente de desktop pode consultar e administrar por serviço; Core WLAN é referenciado para gerenciamento Wi-Fi | modo de associação, permissões e suporte concreto precisam de fonte oficial/versionada e decisão de produto |
| iOS | Solicitar configuração/join via Network Extension quando autorizado; obter apenas observações permitidas | sem API pública geral para listar APs próximos ou senhas/redes de terceiros; consentimento do usuário; callback não prova Internet |
| Android | Solicitar rede/sugestão e observar callbacks/capacidades permitidos pelo SO | SO decide sobre sugestões; scans e background limitados por versão/permissões; callback não prova Internet |
| Windows e Linux desktop | Interfaces remotas e funções administrativas podem ser aplicáveis | API local, permissão, backend e compatibilidade ainda não especificados por versão |
| Web | Consulta a serviços autorizados e superfícies administrativas conforme produto | navegador não é fonte de autoridade nem fornece operação Wi-Fi geral; N/A para associação direta enquanto capacidade não for comprovada |

Fontes oficiais consultadas em 2026-11-16: Apple, [Wi-Fi configuration](https://developer.apple.com/documentation/networkextension/wi-fi-configuration) e [NEHotspotConfigurationManager](https://developer.apple.com/documentation/networkextension/nehotspotconfigurationmanager); Android, [WifiNetworkSpecifier](https://developer.android.com/reference/android/net/wifi/WifiNetworkSpecifier) e [WifiManager](https://developer.android.com/reference/android/net/wifi/WifiManager). A documentação Apple diz que configuração/join exige autorização do usuário e diferencia iOS Network Extension de Core WLAN para macOS. Android descreve `WifiNetworkSpecifier` como solicitação de rede e `WifiNetworkSuggestion` como sugestão sujeita à decisão do SO. Essas APIs não comprovam, por si, autenticação no controlador ou acesso geral à Internet. Sem documentação de fabricante, suporte de roteador é OPEN.

## 4. Atores, autoridade e consentimento

- Usuário/dispositivo observa redes disponibilizadas pelo SO ou por controlador autorizado. A observação não prova propriedade nem autoridade.
- Administrador do AP comprova autoridade pelo mecanismo aprovado e autoriza separadamente compartilhamento de acesso e publicação da posição exata.
- Serviço de identidade/política autentica identidade, tenant, papel, escopo, consentimento, prazo e revogação antes de cada efeito ou consulta protegida.
- Controlador autenticado pode emitir evidência de autenticação/sessão dentro de sua capacidade documentada. Cliente não pode declarar `controller_attested`.
- Serviço de mapa só entrega projeção autorizada. A posição exata pertence ao AP físico, não ao telefone ou à pessoa.
- Operador de segurança lê eventos restritos somente mediante função e auditoria; o público não lê sessões, visitantes ou identificadores pessoais.

SSIDs/BSSIDs, posição, senha conhecida, proximidade e conexão prévia não provam controle. Descoberta local não cria publicação, grant, credencial, sessão ou consentimento. Credenciais principais da casa não são compartilhadas por padrão. Priorizar credenciais individuais/revogáveis ou rede convidada quando suportadas; política por fabricante está OPEN.

## 5. Entidades e dados

Entidades lógicas: rede (`network_id`), AP físico (`ap_id`), controlador, observação privada, compartilhamento (`share_id`), consentimento/versionamento, grant (`grant_id`), credencial referenciada em cofre/controlador, solicitação/sessão (`connection_session_id`), evidência de conectividade, publicação cartográfica, evento de auditoria e cópia controlada. `gmi_id` é identidade interna; GMC é indicativo, não credencial.

Cada objeto operacional precisa de tenant, finalidade, autoridade, versão, origem e instante conforme sua natureza. Coordenada exata, credencial, inventário privado e auditoria seguem escopos e classes de retenção distintos. Segredo não deve ser incluído em payload público, evento genérico, log ou recibo de idempotência. A duração e jurisdição de retenção estão OPEN; classe sem política aprovada não está pronta para persistência/replicação operacional.

## 6. Contratos e comportamento

### 6.1 Descoberta/inventário (RF-035)

O cliente armazena observações locais como inventário privado, com origem e frescor. Sincronização remota exige consentimento e escopo privado explícito. Não publicar observação, exibir como AP comunitário ou promover a prova de autoridade. Sem permissão, API ou observação atual, retornar capacidade/estado limitado ou `unknown`, sem inventar ausência de redes.

### 6.2 Prova e compartilhamento (RF-036, RNF-013)

Criar ou ativar compartilhamento requer identidade e tenant autorizados, prova vigente de administração, consentimento específico para a ação, política de público/escopo, validade e controlador elegível. Falha de prova, tenant, consentimento ou escopo nega sem efeito. Controlador offline não prova revogação aplicada nem autoridade atualizada.

AutoShare não herda consentimento automaticamente para redes observadas pelo celular. Herança de política só pode abranger SSIDs gerenciados por controlador explicitamente autorizado e redes de convidados conforme política; a extensão exata fica OPEN por perfil/fabricante.

### 6.3 AutoConnect e evidência (RF-037, RNF-014)

Fluxo mínimo: autorização/grant vigente → solicitação de associação → eventual aprovação/decisão do SO → observação de associação → evidência autenticada do controlador, se disponível → evidência de Internet segundo método OPEN. Cada transição registra fonte, instante, recurso/sessão e versão. Não inferir estado posterior do anterior.

`requested`, aprovação do SO, associação observada, autenticação do AP/controlador e `internet_verified` são fatos distintos. A modelagem final de enum/nome permanece OPEN por divergência entre documentação atual (`ap_authenticated`, `controller_attested`, `associated`, `connected`, `validated`). Um payload de proposta deve carregar status, origem, instante, alvo lógico e validade de evidência de Internet, sem converter observação expirada em fato atual.

`internet_verified` significa somente que a fonte declarada alcançou o alvo/perfil de serviço definido para aquela sessão naquele instante. Não significa que todos os destinos, apps ou serviços estão acessíveis. Método, origem aceita, alvo, frescor, validade, retenção, falsos positivos e comportamento sob portal cativo ainda exigem decisão. Falha de consulta, troca de rede, sessão diferente, timeout ou evidência expirada remove a atualidade do positivo. `unknown` não significa ausência provada de Internet.

### 6.4 Localização e mapa (RF-038, RF-039)

A posição é vinculada ao AP físico e conserva fonte, qualidade informada/estimada, instante, responsável e histórico, sem fabricar precisão. Cadastro de AP não publica posição. Publicação exata requer prova atual de autoridade, opt-in específico, autenticação recente, escopo, auditoria e consentimento versionado. Sem autorização, fornecer apenas projeção coarse/hidden se aprovada; não expor posição exata, BSSID bruto, GMI, visitantes ou inventário privado.

Revogar opt-in bloqueia novas respostas exatas na autoridade que aceitou a revogação e em cópias que já a receberam. Eventos/tombstones monotônicos têm precedência sobre versões antigas fora de ordem. Réplicas offline ficam pendentes e não servem dado antigo como atual. A declaração de remoção só cobre cópias enumeradas/confirmadas segundo política. Backup/exportação fora do mecanismo exige procedimento de expurgo/restauração. Não prometer remoção global.

### 6.5 Sessão, auditoria e minimização (RF-040)

Cliente registra solicitação, não autenticação do AP. Evento de cliente tem nível `client_claim` ou equivalente ainda por conciliar. Somente controlador autenticado emite evidência com força `controller_attested`. Contadores/duração não medidos permanecem nulos/desconhecidos. Não registrar histórico de navegação.

Participante acessa suas próprias sessões; administrador consulta apenas AP sob sua administração e metadado mínimo; operador de segurança consulta eventos restritos mediante função e auditoria; mapa público não expõe sessões/pessoas. Toda consulta administrativa gera trilha. Definição final de função/ABAC continua com SPEC de identidade e política.

Eventos de segurança propostos incluem negação de publicação/grant, pedido e resultado de revogação, falha de controlador, mudança de precisão, emissão de grant e consulta de auditoria. Campos candidatos: tenant, AP/recurso pseudonimizado, operação, resultado, origem, timestamp e correlator. Remover senha/PSK, token de prova, BSSID público, posição exata e tráfego antes de buffer persistente, exportação, indexação ou painel. Falha de redactor bloqueia o envio do campo sensível; falha de fila/cobertura torna integridade `unknown`, não prova de ausência de incidente. Política de fail-closed, classes obrigatórias, anti-sampling, preservação, acesso e retenção estão OPEN.

Correlator exige namespace/tenant e finalidade; não é credencial, não concede acesso e não é reutilizável entre tenants/finalidades por padrão. Validade, rotação, autoridade emissora, audiência e retenção da leitura continuam OPEN. Resolver correlator para ler evento exige validar papel/tenant/finalidade e auditar a própria consulta. Proibido adicionar papel novo implicitamente.

### 6.6 API, idempotência e erros (RF-041)

WIFI-API-001 fornece endpoints de proposta, não contrato final. RF-041 exige especificar autorização server-side, tenant, função, escopo temporal, consentimento, prova, estado do controlador, versão, idempotência, concorrência, erros, eventos e compatibilidade.

Para operação mutável: mesma chave e corpo equivalente não repete efeito; chave igual com corpo divergente conflita; versão esperada obsoleta não sobrescreve; retry após timeout recupera resultado sem duplicar efeito. Antes de devolver recibo/detalhe de resultado em qualquer retry, inclusive após revogação, revalidar identidade, tenant, papel, escopo temporal e autorização/revogação atual. Se autorização expirou ou foi revogada, não repetir efeito nem revelar recibo/payload protegido; devolver somente erro mínimo permitido pela política. Idempotência não cria direito duradouro de leitura.

Escopo da chave, equivalência do corpo, retenção da chave/recibo, resposta mínima pós-revogação, proteção contra enumeração e comportamento em estado de revogação desatualizado são OPEN. A autoridade offline não pode emitir autorização a partir de estado obsoleto. Deduplicação de evento por `event_id` e versão por `share_id`/`grant_id` precisam conciliar com a SPEC de eventos/sincronização.

Erros já propostos para revisão: `WIFI_PERMISSION_REQUIRED`, `WIFI_OS_UNSUPPORTED`, `WIFI_OWNER_UNVERIFIED`, `WIFI_AP_NOT_REACHABLE`, `WIFI_APPROVAL_REQUIRED`, `WIFI_SHARE_EXPIRED`, `WIFI_GRANT_REVOKED`, `WIFI_CONTROLLER_OFFLINE`, `WIFI_ACCESS_LIMIT_REACHED`, `WIFI_NOT_IN_RANGE`, `WIFI_INTERNET_UNVERIFIED`, `WIFI_LOCATION_PUBLICATION_DENIED`. Compatibilidade semântica, status HTTP/GMP e extensão do conjunto permanecem OPEN; esta lista não é enum aprovado.

### 6.7 Confidencialidade e ACK (ADR-060, ADR-058)

Toda perna de comunicação em que cliente, servidor, controlador, bridge ou adaptador termina a proteção declara `confidentiality=terminated` na API e na apresentação correspondente. TLS, DTLS ou SRTP protegem apenas a respectiva perna e não podem ser descritos como E2EE. Quando houver bridge/adaptador de áudio ou telemetria, o perfil identifica o terminador e os dados que o servidor passa a poder ver; em integração Wi-Fi, a aplicabilidade concreta é desconhecida — decisão pendente.

Toda resposta da API inclui `ack_class` obrigatório e evidência/escopo compatíveis com a classe: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`. ACK de bridge/adaptador não pode promover estado a `delivered_to_device` nem `acknowledged_human`; essas classes exigem prova própria do destino/dispositivo ou da pessoa. Não se infere classe superior por tempo, ACK inferior ou ausência de erro.

## 7. Revogação, falhas e recuperação

Revogação identifica objeto/escopo e versão. A autoridade que aceita a operação bloqueia novas emissões/respostas dentro de seu alcance conhecido e emite evento/tombstone. Cada controlador, réplica ou cache controlado confirma aplicação separadamente. Sem confirmação, manter `pending`/`unknown`. Controlador offline não equivale a `revoked_applied`. PSK já compartilhada requer ação/rotação confirmada no controlador; revogação do share não a torna inválida automaticamente.

Ações defensivas candidatas para cross-tenant, credencial exposta, publicação indevida ou partição: negar novas consultas/grants no escopo autorizado, preservar evidência sanitizada, iniciar revogação e escalonar ao owner. Autoridade organizacional, função nominal, plantão, tempo, fallback, ação global e procedimento de recuperação não estão definidos. Não executar contenção ou rotação automática com base nesta SPEC.

A coleta de sinais diferencia evento observado de falha de coleta, atraso, cobertura insuficiente ou controlador inalcançável. Fonte, população/denominador, janela, frescor, destino do alerta e escalonamento devem ser especificados. Sem baseline aprovado, não fixar limiar nem alegar cobertura suficiente.

## 8. Critérios de aceite planejados

Critérios documentais para futura revisão, não resultados de teste:

1. Descoberta sem permissão ou com dado obsoleto não cria registro público nem inventa estado.
2. SSID/BSSID/conexão/senha conhecida sem prova administrativa não permite AutoShare.
3. Consentimento ausente, expirado ou fora do escopo nega publicação/grant sem efeito.
4. Pedido do app e aprovação do SO não afirmam associação; associação não afirma autenticação; autenticação não afirma Internet.
5. Ausência de controlador ou observação de Internet produz `unknown`/pendente, salvo evidência específica que sustente outro estado.
6. Publicação exata sem opt-in específico e autoridade vigente é negada. Mapa público não retorna BSSID bruto, credencial, GMI, visitante, sessão ou inventário privado.
7. Operação repetida não duplica efeito; chave reutilizada com corpo diferente ou versão obsoleta conflita.
8. Retry pós-revogação revalida autorização atual, não repete efeito e não revela recibo protegido. Retry pré-revogação autorizado recupera resultado anterior sem duplicar efeito. Um principal de outro tenant não resolve o recibo; nó com versão de autorização/revogação desatualizada não concede leitura nem efeito.
9. Revogação bloqueia respostas na autoridade atual, tombstone vence evento antigo e réplica offline não serve dado obsoleto como atual.
10. Consulta administrativa é escopada por papel/tenant e gera trilha; leitura cross-tenant sem compartilhamento explícito é negada.
11. Falha de redaction não exporta campo sensível; falha de pipeline não é tratada como ausência de incidente.
12. SOS/PTT/Guardian ativos não são degradados por troca automática de Wi-Fi sem política e rota autorizadas.

## 9. Casos planejados (não executados)

Todos os IDs abaixo são rótulos internos deste rascunho, não casos cadastrados no sistema. A matriz oficial e o índice continuam `NOT_ASSIGNED`; observado fica `NOT_EXECUTED`.

| Rótulo | Pré-condição/estímulo | Resultado verificável planejado | Negativo/degradação | Requisitos |
|---|---|---|---|---|
| P-WIFI-01 | SO fornece observação; sincronização privada consentida | Inventário conserva origem/hora e escopo privado | Sem permissão, dado não vira inventário público | RF-035 |
| P-WIFI-02 | Administrador e prova válidos; consentimento para share | Ativa apenas política aprovada | SSID conhecido, tenant divergente ou consentimento expirado nega | RF-036, RNF-013 |
| P-WIFI-03 | Solicitação enviada ao SO | Mantém pedido e callback como fases distintas | Sem callback não afirma associação | RF-037, RNF-014 |
| P-WIFI-04 | Verificador e alvo configurados alcançáveis | Registra fonte, alvo lógico, instante e validade | Mudança de rede, falha ou evidência vencida resulta desconhecida/limitada | RF-037, RNF-014 |
| P-WIFI-05 | Evento autenticado do controlador | Classifica sessão conforme atestação | Relato do app continua `client_claim` | RF-040, RNF-013 |
| P-WIFI-06 | AP comprovado e opt-in exato vigente | Publica posição do AP no escopo autorizado | Visitante, opt-out ou prova ausente não publica | RF-038, RF-039 |
| P-WIFI-07 | Retry idêntico após timeout | Um único efeito e mesmo resultado lógico enquanto autorização vigente | Corpo divergente/versão antiga conflita | RF-041 |
| P-WIFI-08 | Retry idêntico depois de revogação | Nenhum efeito duplicado; autorização revalidada; recibo protegido não exposto | Recibo/payload não é revelado após revogação; principal cross-tenant não o resolve; nó com versão de autoridade obsoleta nega ou retorna estado mínimo sem efeito | RF-036, RF-041, RNF-013 |
| P-WIFI-09 | Revogação com réplica desconectada | Autoridade bloqueia resposta e guarda pendência; tombstone vence ao reconectar | Sem ACK não declara remoção global; PSK requer ação separada | RF-036, RF-039, RF-041 |
| P-WIFI-10 | Consulta participante/admin/operador/público | Cada ator recebe somente escopo permitido e leitura administrativa fica auditada | Cross-tenant sem compartilhamento nega | RF-040, RNF-013 |
| P-WIFI-11 | Auditoria e telemetria recebem evento de segurança | Sanitiza antes do buffer/exportação e registra cobertura/fonte | Falha do redactor bloqueia campo sensível; falha de pipeline vira unknown | RNF-013 |
| P-WIFI-12 | Uso de correlator autorizado e tentativa não autorizada | Leitura valida finalidade/audiência e audita a consulta | Tenant/função/finalidade incorretos não resolvem correlator | RNF-013, RNF-014 |
| P-WIFI-13 | SOS/PTT ativo e pedido de mudança de rede | Política mantém serviço prioritário sem migração não autorizada | Falta de rota/capacidade é mostrada sem garantia inventada | RF-037 e contratos SOS/voz |

## 10. Decisões OPEN e responsáveis

| OPEN | Alternativas e consequência | Dono proposto | Condição de fechamento |
|---|---|---|---|
| ADR-060/058: aplicabilidade de terminador, visibilidade e ACK em perfis Wi-Fi | bridge/adaptador identificado por perfil; sem inferência global | Jouni + Gregor; Richard revisa | Perfis aplicáveis identificam terminador/conteúdo visível e cada resposta fornece ACK classificado com evidência própria |
| B05: evidência `internet_verified` | teste do cliente mede caminho local; serviço mede alvo escolhido; controlador fornece evidência de sua capacidade; fontes podem coexistir se rotuladas | Jouni + Graydon; Donny/Chet/Lea, operação e segurança | Aprovar método, alvo lógico, fonte, validade/frescor, troca de rede, falha, unknown, retenção e consentimento |
| B07: mapeamento e idempotência | chave por operação/recurso/grant; dedupe de evento separado da idempotência da API; operação atômica local vs estágio/cópia distribuído | Jouni + Graydon; Kleppmann/Michael/Leslie para distribuição | Definir escopo/retensão da chave, concorrência, versões, conflitos, retry pós-revogação e recibo mínimo |
| Estados/eventos | manter eventos tipados por fase vs consolidar enum; nomes atuais divergem | Jouni + Graydon; matriz/Ada | Crosswalk aprovado entre arquitetura, domínio, API/GMP e consumidores; sem semântica inferida |
| Autoridade/consentimento | controlador, titular ou autoridade regional podem ter poderes distintos | Kim + Bruce + administrador/usuário | Prova, escopo, delegação, validade, contestação e negativas definidos |
| Retenção/jurisdição | política parametrizada por classe e oferta hospedada/autogerida; jurisdição não foi escolhida | Bruce + Michael + autoridade competente | Finalidade, gatilho, duração, descarte, cópias/backup, auditoria e região aprovados |
| Controladores/RADIUS/EAP | integrações diferem; capacidade pode faltar | Jouni + Graydon + Kim; fornecedor ainda não selecionado | Fonte primária por modelo/versão, prova, sessão, rotação, revogação e falha documentadas |
| Telemetria e resposta | evento por domínio; cobertura sem limiar; contenção limitada por tenant/AP/região | Jouni + Brendan; Bruce/Richard revisam; Alan encaminha autoridade | Schema, redaction, anti-sampling, audiência, denominador, janela, owner/plantão, autorização, confirmação e recuperação definidos |
| Correlator | aleatório por evento ou pseudônimo derivado; resolução central ou por serviço | Jouni + Brendan; Bruce/Michael/Alan decidem política | Namespace, finalidade, autoridade emissora, validade/rotação, audiência, trilha de leitura e falhas aprovados |
| Falha de auditoria | falhar fechado para leitura/operação sensível ou permitir exceção explícita | Bruce + Michael + Alan | Ação por tipo de operação, responsável, exceção, proteção de campo sensível e recuperação documentados |
| Aplicabilidade desktop/web | função administrativa/consulta remota versus associação direta | produto/plataforma + Jouni | Permissões e capacidade verificadas por sistema/versão; N/A justificado individualmente |
| SOS/PTT e Wi-Fi | preservar rota atual vs migrar quando autorizado e seguro | Radia/Vint/Nancy + Jouni | Prioridade/empates, fallback e evidência ligados a contratos aplicáveis |

## 11. Cobertura, revisões e estado

Cobertura semântica atual: RF-035 a RF-041 e RNF-013/RNF-014 têm escopo descrito e cenários propostos. RNF-013/014 mantêm dependências quantitativas/ADR; RF-041 continua a especificar. Integração com Android/iOS detalha apenas limites documentados; Windows/Linux desktop/web e fabricantes permanecem parciais. Gatilhos de incidente, autoridade, retenção, regiões, enum/erro final, validade de evidências e perfis comerciais seguem OPEN.

Richard Bejtlich revisou a versão da contribuição e considerou os achados de telemetria, contenção, redaction e correlator endereçados substancialmente no desenho, porém parciais por políticas, owners, retenção, validade/rotação e parâmetros OPEN. Dan Kaminsky identificou a necessidade de revalidar escopo/autorização e proteger recibo em retry pós-revogação; esse delta está incluído como proposta. Revisões referem-se às versões/hashes na seção 2, não a este arquivo canônico. Revisão desta SPEC ainda pendente.

**Estado final deste rascunho:** DRAFT-0.1 / OPEN. Não aprovado. Nenhuma API de fabricante, instalação, rede, hardware, serviço, código, banco, SQL ou teste de produto foi executado ou validado. Aprovação do usuário e gates de SPEC antecedem implementação.

## Casos por decisão (ADR-056…063)

Casos planejados, sem ID formal: `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED` em todas as linhas. `Tipo`: **P** positivo, **N** negativo, **B** borda, **F** falha/recuperação. Cada decisão aplicável tem ao menos um caso N e um F. Onde o parâmetro é `OPEN`, o esperado é negar ou informar desconhecido — nunca número, prazo, código de erro ou enum inventado. Estes casos complementam §8 e §9 e não os substituem.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| C009-056-P | ADR-056 | Cliente e servidor de API Wi-Fi com perfis N e N-1; **janela N/N-1 de `WIFI-API-001` não declarada** (`OPEN`) | Criar um compartilhamento | Esperado reformulado com parâmetro ausente: operação recusada com `param_not_configured`; nenhum `share_id` criado e compatibilidade exibida como desconhecida. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-056-N | ADR-056 | Cliente cujo perfil está fora da janela de suporte declarada | Enviar proposta de compartilhamento | Rejeição antes de qualquer efeito: nenhum `share_id`, nenhum `grant_id`, nenhuma linha de auditoria de efeito; incompatibilidade declarada sem downgrade silencioso | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-056-B | ADR-056 | Corpo de proposta com campo adicional desconhecido do servidor N-1 | Enviar e reler a proposta | Campo desconhecido preservado; número de campo não reutilizado; campo removido marcado `reserved` | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-056-F | ADR-056 | Janela N/N-1 de `WIFI-API-001` não declarada para o perfil (`OPEN`) | Operar cliente e servidor de versões distintas | Operação recusada com motivo de perfil não suportado; estado desconhecido informado; nenhuma aceitação por “melhor esforço” | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-057-P | ADR-057 | Evento de revogação de opt-in em partição conhecida e `authority_epoch` vigente | Consultar e aplicar o evento com cursor versionado da tupla completa | Tombstone aplicado na posição correta; cursor devolvido com versão e vínculo a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-057-N | ADR-057 | Evento ou cursor de sincronização sem `partition` | Aplicar o evento na réplica | Evento rejeitado com erro explícito; nenhuma aplicação parcial; revogação não considerada aplicada; nenhuma tradução de cursor | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-057-B | ADR-057 | Duas partições com eventos de mesma `seq`; tombstone monotônico e evento antigo chegam fora de ordem | Aplicar ambos | Eventos tratados por partição; tombstone vence a versão antiga; nenhum colapso por `seq` repetida | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-057-F | ADR-057 | Réplica offline reconecta após troca de `authority_epoch` e reapresenta cursor antigo | Replay do cursor na réplica | Rejeição fechada por epoch incomparável; réplica não serve dado obsoleto como atual; estado `pending`/`unknown` e pedido de checkpoint novo | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-058-P | ADR-058 | Cliente envia solicitação de associação ao SO e recebe callback de aceitação | Consolidar a etapa | Resposta com `ack_class=transport_accepted` e recibo daquele salto; `requested` e `associated` permanecem estados distintos; nenhum texto de associado | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-058-N | ADR-058 | ACK de bridge/adaptador ou de controlador | Apresentar como `delivered_to_device` ou `acknowledged_human` | UI não promove: exibe a classe comprovada e a etapa aberta separadas; nenhuma exibição de entrega ao dispositivo ou reconhecimento humano | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-058-B | ADR-058 | Resposta de API Wi-Fi sem `ack_class` | Consumir a resposta | Resposta inválida; estado da etapa `unknown`; nenhum efeito exibido como confirmado (código de erro final `OPEN`) | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-058-F | ADR-058 | Callback/recibo perdido após a submissão ao SO ou ao serviço | Reconectar e reconsultar | Estado `unknown`; associação não afirmada; repetição com a mesma chave idempotente não duplica efeito | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-059-P | ADR-059 | Dispositivo offline com inventário local já autorizado | Consultar inventário privado local | Inventário exibido com origem e frescor; nenhum `share_id` ou `grant_id` criado; nenhuma mutação federada enfileirada | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-059-N | ADR-059 | Dispositivo offline com grant assinado disponível | Pedir AutoShare, emissão de grant ou publicação de posição | Operação negada por não estar entre as ações enumeradas; sem efeito, sem fila de mutação federada; motivo registrado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-059-N2 | ADR-059 | Pedido de criação/alteração de grant ou mudança de identidade offline | Executar o pedido | Negado sem efeito; nenhum grant novo emitido, nenhuma identidade alterada; estado permanece o anterior | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-059-F | ADR-059 | Controlador offline e estado de autorização `stale`/`unknown`; frescor do perfil `OPEN` | Emitir grant ou confirmar revogação | Emissão negada; estado `pending`/`unknown`; nenhuma exibição de `revoked_applied`; nenhum prazo ou validade inventados | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-059-B | ADR-059 | Grant offline de sessão anterior com `epoch` comparável mas fresco indeterminável | Tentar operação condicional | Operação negada por frescor desconhecido; grant não renovado nem promovido; motivo explícito | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-060-P | ADR-060 | Consulta administrativa somente na perna protegida cliente↔servidor, sem bridge/adaptador de áudio | Consultar dado administrativo na API e na UI | Campo de confidencialidade presente e fiel ao perfil; nenhuma alegação de E2EE; TLS da perna não descrito como ponta a ponta | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-060-N | ADR-060 | Bridge/adaptador de áudio ou telemetria habilitado no perfil | Ler estado da integração na API e na UI | `confidentiality=terminated` presente com terminador identificado e campos que o servidor passa a ver; proibido exibir E2EE ou anonimato | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-060-B | ADR-060 | Perfil de integração sem terminador definido (`OPEN` em §10) | Consultar o perfil | Terminador declarado desconhecido e artefato tratado como `terminated`; nenhuma alegação de proteção ponta a ponta | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-060-F | ADR-060 | Cópia de evento de auditoria sem metadado de confidencialidade após replicação | Consultar a cópia | Cópia tratada como `terminated` com ausência declarada; nenhuma herança de proteção por cache ou versão anterior | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-061-P | ADR-061 | Serviço Wi-Fi com transação única de domínio e `outbox` | Criar compartilhamento | `share_id` e evento de outbox na mesma transação; publicação após commit; uma única linha de auditoria de efeito | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-061-N | ADR-061 | Outbox com evento de compartilhamento/revogação pendente; controlador configurado | Executar replay do outbox | Projeção reconstruída sem reemitir comando ao controlador ou rotação de PSK; nenhum efeito externo repetido | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-061-N2 | ADR-061 | Recepção de evento do controlador em Inbox/ledger separado da projeção | Consolidar o evento | ACK máximo `persisted_local`; `materialized` não emitido; UI não exibe sessão projetada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-061-B | ADR-061 | Duplicata do mesmo `event_id` de compartilhamento ou revogação no outbox | Executar replay do outbox | Uma única linha de auditoria de efeito por evento; duplicata descartada pela chave idempotente e registrada; nenhum comando repetido ao controlador | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-061-F | ADR-061 | Controlador offline ou crash entre commit e publicação | Reconectar e reconciliar | Estado `pending`/`unknown` até evidência; commit recuperado publicado uma única vez; nenhum efeito duplicado nem `revoked_applied` presumido | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-062-P | ADR-062 | Política com finalidade, classe e cópia declaradas para a auditoria Wi-Fi, com prazo por classe da ADR-065, jurisdição BR/CO e gatilho não decididos** (`OPEN`) | Gravar evento de auditoria | Esperado reformulado com parâmetro ausente: evento gravado com a política identificada e sua versão; prazo por classe conforme ADR-065, jurisdição BR/CO e gatilho reportados como `param_not_configured`, sem expiração automática anunciada; nenhum campo sensível fora da classe autorizada | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-062-N | ADR-062 | Nenhuma política para a cópia de auditoria/telemetria | Gravar evento de segurança | Persistência negada com erro explícito; nenhum evento em buffer, exportação, índice ou painel; falha informada, nunca “retenção temporária” sem prazo | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-062-B | ADR-062 | Posição exata do AP e inventário privado em classes distintas, com política apenas para a classe coarse | Tentar gravar a posição exata | Campo vedado negado por falta de política própria; apenas a classe autorizada é gravada; degradação informada | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-062-F | ADR-062 | `legal hold` e descarte aplicados a cópias enumeradas; uma cópia não confirma descarte | Consultar e tentar restaurar cada cópia | Hold exige autoridade, escopo e revisão com liberação auditada; cópia sem confirmação fica `unknown`/`pending`; nenhuma remoção global declarada | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-063-P | ADR-063 | Cliente com SBOM por release, assinatura válida e `generation` igual ou superior ao piso | Instalar e operar a API Wi-Fi | Instalação aceita; `generation` e digest registrados; operação autorizada executada com artefato verificado | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-063-N | ADR-063 | Versão anterior do cliente, corretamente assinada, abaixo do piso | Instalar sobre o piso atual | Rejeição por `generation` inferior ao piso; binário não substituído; nenhuma operação Wi-Fi com a versão rebaixada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-063-N2 | ADR-063 | Artefato de cliente ou firmware de controlador adulterado | Instalar/atualizar | Rejeição por assinatura/digest antes de qualquer execução; nenhuma credencial ou configuração Wi-Fi processada pelo artefato | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-063-F | ADR-063 | Chave de assinatura comprometida com lista de revogação publicada | Atualizar com artefato assinado pela chave revogada | Lista verificada e artefato rejeitado; atualização bloqueada até reemissão; evento auditável registrado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C009-063-B | ADR-063 | Firmware de controlador sem `generation` ou SBOM declarados (`OPEN` — fornecedor não selecionado) | Tentar operação de escrita no controlador | Artefato tratado como não verificável: escrita negada e estado degradado informado; nenhuma exceção silenciosa | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §8 e §9 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-035 | P-WIFI-01 | C009-RF035-N1 (novo) | C009-RF035-B1 (novo) | C009-059-P (inventário local offline, sem mutação) | Inventário privado; sincronização remota exige consentimento e escopo explícitos |
| RF-036 | P-WIFI-02 | C009-056-N, C009-059-N | C009-062-B | C009-059-F, C009-061-F | Prova de administração, consentimento e revogação; PSK já compartilhada exige rotação confirmada |
| RF-037 | P-WIFI-03 | C009-RF037-N1 (novo) | C009-RF037-B1 (novo) | P-WIFI-09 | Fases `requested`, associação, autenticação e `internet_verified` nunca se inferem |
| RF-038 | P-WIFI-06 | C009-RF038-N1 (novo) | C009-RF038-B1 (novo) | C009-060-F | Posição pertence ao AP físico; origem, qualidade, responsável e histórico |
| RF-039 | P-WIFI-06 | C009-062-B | C009-RF039-B1 (novo) | C009-057-F | Opt-in específico e autoridade vigente; mapa público não expõe BSSID, credencial, GMI ou visitante |
| RF-040 | P-WIFI-05, P-WIFI-10 | C009-RF040-N1 (novo) | C009-057-B | C009-061-N2 | `client_claim` e `controller_attested` não se convertem; contadores não medidos ficam nulos |
| RF-041 | C009-056-P (reformulado) | C009-056-N, C009-058-N | C009-056-B | C009-058-F, C009-061-F | Escopo da chave, retenção do recibo e resposta mínima pós-revogação seguem `OPEN` (B07) |
| RNF-013 | P-WIFI-11, C009-062-P (reformulado) | C009-062-N | C009-057-B | C009-060-F | Redaction falha bloqueia o campo sensível; falha de pipeline vira `unknown` |
| RNF-014 | P-WIFI-04 | C009-058-N | C009-058-B | C009-058-F | Método, alvo, frescor e validade de `internet_verified` permanecem `OPEN` (B05) |

Casos novos, descritos em uma linha: **C009-RF035-N1** — descoberta sem permissão ou com observação expirada: nenhum registro público, `unknown` declarado, sem inventar ausência de redes. **C009-RF035-B1** — observação local sem frescor confiável: exibida como limitada, não promovida a inventário compartilhável. **C009-RF037-N1** — callback do SO ausente: `requested` permanece distinto de associação e nenhuma autenticação é afirmada. **C009-RF037-B1** — associação observada com autenticação de controlador indisponível: estados separados, sem inferir `internet_verified`. **C009-RF038-N1** — AP sem coordenada confiável: ausência explícita, nenhuma precisão inventada. **C009-RF038-B1** — posição de AP alterada: histórico preservado com origem e responsável, versão antiga não apresentada como atual. **C009-RF039-B1** — opt-in revogado em réplica offline: réplica não serve posição exata como atual e declara pendência, sem alegar remoção global. **C009-RF040-N1** — consulta administrativa fora do tenant ou do AP administrado: negada e auditada, sem revelar sessão ou visitante.
