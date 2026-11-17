# SPEC-011 — Localização geoespacial

**Versão:** DRAFT-0.5  
**Data:** 2026-11-17  
**Estado:** DRAFT / OPEN. Não aprovada.  
**Autor documental:** Paul Ramsey  
**Revisor independente designado:** Bruce Schneier.  
**Revisão:** o parecer citado abaixo refere-se a uma versão anterior; a revisão independente de DRAFT-0.2 está pendente.  
**Escopo:** geoespacial, frescor e privacidade espacial; integra com Guardian, SOS, Wi-Fi/AP, identidade/autorização, dados, sincronização e plataformas.  
**Contribuição de origem:** `SPECS-EM-ANDAMENTO/contribuicao-paul-ramsey.md`, SHA-256 `2cf50403f30be73f0c90f444ec2f385f7ef551fe6a4fb484e77bf001b2dc128e` (versão lida em 16/11/2026).  
**Parecer de privacidade:** `contribuicao-bruce-schneier.md`, SHA-256 `8a18dac0c45c4ad365112619399cfdf1f14c4c28dfb2932e98b00ec971923488` (versão lida em 16/11/2026). Achados de Bruce aceitos como lacunas documentais parciais; nenhum deles é declarado fechado.

## Mudanças desta versão

- **DRAFT-0.5 — critério observável no lugar de “conforme política”:** `S011-RNF003-P1` deixa de dizer “dado entregue conforme política” e passa a exigir, no esperado, o `policy_id/version` aplicado e a retenção declarada da classe; leitura sem política vigente da cópia responde `RETENTION_POLICY_MISSING` e é negada. Nenhum valor numérico é criado.

- **ADR-057:** cursores/streams de observação passam a usar posição canônica com `partition` e `authority_epoch` distintos; cursor sem partição ou com epoch incomparável é rejeitado.
- **ADR-058:** ACK de ingestão declara classe por fronteira (local, replicado à autoridade ou materializado); ACK de ingestão nunca significa entrega a dispositivo/destinatário nem reconhecimento humano.
- **ADR-060:** bridge que recebe localização termina a confidencialidade; a resposta/UI declara `confidentiality=terminated` e a perda de precisão da projeção. Não se alega anonimato nem E2EE para localização.
- **ADR-062:** retenção fail-closed por finalidade, classe, cópia, jurisdição, prazo e gatilho; sem política aprovada, não persistir histórico, e declarar descarte por cópia.
- **casos por requisito e casos P desbloqueados (C3-1):** nova subseção `### Casos por requisito (P/N/B/F)`, com uma linha por requisito declarado (RF-009, RF-029, RF-038, RF-039, RF-021, RNF-003, RNF-015, RNF-017, RF-026) e as quatro colunas preenchidas ou `n/a` justificado; casos P cujo esperado dependia de valor não decidido — janela N/N-1 do perfil `geo-stream`, limite de frescor do grant offline e prazo/gatilho da política ADR-062 — reescritos para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID; caso de borda `S011-061-B1` (ADR-061) acrescentado. Nenhum número é criado; nada é aprovado ou executado.
- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.

Esta versão continua DRAFT/OPEN; as mudanças documentais não constituem aprovação, implementação ou validação.

## 1. Requisitos e cobertura

| Requisito | Cobertura nesta versão | Estado |
|---|---|---|
| RF-009 | observações, consultas, histórico, geofence, frescor e estados incertos | Parcial; regra de geofence e retenção OPEN |
| RF-029 | fontes, combinação, incerteza, frescor e limites de sensores | Parcial; fusão, suporte e critérios por plataforma OPEN |
| RF-038 | localização de ponto de acesso físico, proveniência e histórico | Parcial; integração e esquema pertencem também à SPEC-009 |
| RF-039 | autorização e divulgação de posição exata de AP no mapa | Interface e política referenciadas; fluxo e aceite completos OPEN |
| RF-021, RNF-003, RNF-015, RNF-017 | autorização, isolamento, retenção e estado offline/revogação | Interfaces transversais; propriedade de Kim/Bruce/Michael/Martin e critérios completos OPEN |
| RF-026 | localização opcional em SOS | Integração dependente da SPEC-006; não especifica o fluxo SOS |

Os requisitos acima são identificadores existentes em `docs/00-produto/requisitos.md` e `docs/06-planejamento/matriz-rastreabilidade.md`. A tabela registra cobertura pretendida, não conformidade demonstrada. Os casos abaixo são planos, sem IDs atribuídos; observado: NOT_EXECUTED.

## 2. Produtos, plataformas e atores

Aplicável ao servidor Linux e serviços que autorizam, armazenam, projetam ou consultam localização; clientes macOS universal, iOS, Windows, Linux desktop, Android e web quando coletam, mostram ou compartilham localização. Capacidades de GNSS, IMU, Wi-Fi RTT, BLE e UWB variam por dispositivo, SO, permissão e disponibilidade. Cliente declara `unsupported`, `unavailable`, `denied` ou equivalente quando a fonte não pode operar; nenhum requisito presume sensor universal.

Atores: usuário/ativo observado; dispositivo e adaptador de fonte; estimador/fusionador; serviço de localização; destinatário autorizado; administrador de AP; autoridade de identidade e consentimento; destinatário SOS designado. Adaptador e fonte não concedem autorização. Identidade, grants e autorização seguem contrato da SPEC-002; eventos e replicação seguem SPEC-004; persistência e retenção seguem SPEC-005; produto Guardian e SOS seguem SPEC-012/SPEC-006.

## 3. Contrato de observação e estimativa

Uma **observação** é uma medição ou indicação produzida por fonte identificável. Uma **estimativa** é uma posição ou área calculada a partir de uma ou mais observações. O consumidor deve conseguir distinguir as duas e identificar as fontes usadas na estimativa. Esta distinção é contrato proposto; o modelo e as APIs existentes não comprovam um esquema implementado.

Quando observações forem expostas em stream ou consultadas com cursor, a posição de evento é a tupla canônica `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` (ADR-057). `authority_epoch`, `dispatch_epoch`, `event_sequence` e cursor são tipos distintos, não intercambiáveis. O cursor é opaco, versionado e vinculado a principal, escopo, filtros, perfil e retenção. Cursor sem partição ou de epoch incomparável é rejeitado, nunca traduzido.

O registro espacial deve preservar, quando disponíveis, identidade da entidade/ativo, origem e método, `observed_at`/instante medido, `received_at`, coordenada ou representação espacial, qualidade, incerteza declarada pela fonte ou distribuição, nível de confiança e estado da permissão. Não substituir falta de dado por coordenada. Não tratar hora de recebimento como hora da medição. Assinatura ou integridade de origem não provam frescor, precisão ou consentimento.

Toda estimativa que combina fontes registra a linhagem e incerteza resultante conforme regra aprovada. Se não for possível declarar fonte, qualidade ou frescor necessários, apresentar estado não disponível/indeterminado; não promover RSSI de Wi-Fi/BLE a distância, ponto ou orientação. Sighting de beacon sem posição própria do observador indica proximidade, não localização do beacon. PDR e integração inercial acumulam incerteza; estimativa offline não se apresenta como ponto atual confirmado.

### Ingestão, ACK e persistência

Cada resposta de API de ingestão declara `ack_class`, conforme ADR-058. `transport_accepted` atesta apenas aceitação de transporte; `persisted_local`, commit durável nesta cópia; `replicated_to_authority`, commit pela autoridade da partição/tenant; `materialized`, efeito materializado no escopo indicado. As classes exigem evidência própria e não se inferem umas das outras. Nenhuma dessas classes significa `delivered_to_device` ou `acknowledged_human`, nem entrega a destinatário. Se a confirmação se perder, o resultado é `unknown`, não sucesso presumido.

Observação, histórico, evento/resultado de geofence e derivados de localização só podem ser persistidos com política aprovada identificada pela tupla `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)` (ADR-062). Sem política aplicável, falhar fechado: não persistir histórico nem derivado; limitar a operação ao mínimo necessário para o efeito imediato autorizado e declarar degradação. A política deve identificar cada cópia relevante, inclusive caches, índices, réplicas, exportações e backups, e o descarte deve ser declarado por cópia, sem alegação de remoção global. Auditoria tem política e prazo próprios; hold só com autoridade, escopo e revisão. Prazos, jurisdições e gatilhos concretos permanecem OPEN e não são inferidos nesta SPEC.

### Estados mínimos de localização

| Estado | Significado e comportamento |
|---|---|
| `available` | Há estimativa e metadados suficientes para o uso e a autorização aplicáveis; isso não equivale a precisão garantida. |
| `stale` | A medição excedeu a regra de frescor aprovada para o uso; preservar a hora e não apresentá-la como atual. Limiares seguem OPEN. |
| `unknown` | Fonte, validade, autorização ou estado não foram confirmados. Não converter em `available`. |
| `unavailable` | Nenhuma fonte utilizável ou estimativa foi produzida; não criar ponto. |
| `unsupported` | Plataforma, sensor ou API não oferece a capacidade necessária; indicar a capacidade ausente. |
| `denied` | Permissão ou consentimento não concedido ou revogado; respeitar a decisão e não coletar/compartilhar sob esse grant. |

Esses nomes são proposta de contrato para conciliação. A SPEC-002 define estados de validação de credencial; não os renomear nem confundir com os estados de localização acima.

## 4. Geofence e trajetória

Uma avaliação de geofence retorna `inside`, `outside` ou `indeterminate`. Retornar `indeterminate` quando a área de incerteza cruza o limite, a posição é `stale`/`unknown`, a fonte está indisponível ou os dados não sustentam uma classificação. Somente observação elegível, autorização vigente e regra espacial aprovada podem produzir transição categórica. Uma amostra atrasada ou repetida não deve aparentar evento atual novo. Localização atrasada nunca implica automaticamente abandono, lesão ou SOS.

O sistema registra a geometria, referência espacial/CRS, versão da regra, instante da amostra, estimativa, incerteza e resultado suficientes para reproduzir a decisão documentalmente, sujeitos a minimização e retenção. A codificação de CRS, geometria permitida, topologia, tolerância e transformação ainda não está fechada.

Eventos de geofence são dados espaciais sensíveis. Destino, audiência, finalidade, conteúdo e frequência devem ser autorizados e limitados; a combinação de horários, proveniência e transições pode revelar trajetória mesmo quando a coordenada foi generalizada. Coalescência versus emissão por amostra, deduplicação, janela temporal, histerese e limites de repetição/cadência permanecem OPEN. Não introduzir parâmetros numéricos sem evidência e decisão.

## 5. Projeções e autorização por destino

A observação precisa e as projeções para destinos devem ser separadas por política. Ocultar coordenadas somente na UI não protege payloads, APIs, caches, índices, logs, exportações, replicação, bridges ou backups. Uma futura matriz por destino deve especificar representação, campos temporais/proveniência, cadência/estabilidade, acesso, retenção e disposição por cópia.

Para chat, PTT, timeline, mensagens e bridges externas, `politica-localizacao.md` §§1–3 estabelece somente localização aproximada quando permitida, com área ou região de incerteza espacial nominal mínima de 50 km de raio; não compartilhar coordenada original, precisão GNSS, geohash de alta resolução ou metadados que reconstruam o ponto. O raio não garante anonimato. A regra exige região que contenha o ponto e estabilidade/frequência configuradas, mas valores e método final permanecem OPEN quando não cobertos pela decisão existente.

O raio nominal de 50 km é limite de generalização, não anonimização nem garantia contra identificação por correlação, contexto, metadados ou consultas sucessivas. Repetir ou combinar consultas não pode aumentar a precisão sem limite: a projeção deve controlar cadência, estabilidade e composição de resultados, evitando deslocamentos sucessivos que permitam estreitar a posição. O limite quantitativo de consultas e o mecanismo de composição permanecem OPEN; até serem decididos, não se presume que repetição seja segura.

A localização precisa de SOS fica segregada e só é acessível sob autorização específica por incidente, finalidade, destinatário e política de retenção, conforme `politica-localizacao.md` §4. Não inferir divulgação ou coleta excepcional de SOS a partir de permissão geral de localização. A SPEC-006 decide sequência e estados SOS. ACK técnico não prova atendimento humano.

Localização de AP físico é distinta de localização pessoal. ADR-049 e ADR-054 permitem registro geográfico preciso do AP quando medido e publicação exata somente por opt-in específico, com política própria. RF-038 exige origem, qualidade, responsável, data e histórico. Não aplicar automaticamente a regra pessoal de 50 km ao AP; também não expor dados civis de administrador ou visitantes. A SPEC-009 e o requisito RF-039 definem os detalhes Wi-Fi e de mapa.

Se uma bridge receber qualquer localização, a perna que termina a proteção declara `confidentiality=terminated` na API e na UI, conforme ADR-060. Declarar também a transformação e a perda de precisão aplicada; se nenhuma degradação foi feita, registrar explicitamente que não houve perda de precisão. Não alegar E2EE nem anonimato para localização transferida à bridge. A bridge não recebe posição precisa de SOS por padrão; qualquer exceção depende de integração formal, autorização e destinatário específico.

## 6. Autorização, partição e revogação

Cada leitura, histórico, projeção, transferência e replicação requer autorização compatível com principal, tenant/escopo, finalidade, precisão, destino e validade. Consentimento de coleta não equivale a autorização de leitura; permissão do sensor não é grant de compartilhamento; proveniência não prova consentimento.

`identidade-autorizacao.md` §§115–118 e 238–247 documenta negação de localização precisa e sync privado quando a credencial está `unknown` ou `stale`; operação local em cache depende de grant assinado ainda válido, epoch conhecido e política de dados. Isso não decide por si a aquisição local nova. Negar aquisição precisa sem frescor é proposta conservadora ainda OPEN; não presumir exceção para SOS. Política precisa distinguir (1) aquisição nova, (2) leitura local de cópia recebida, (3) divulgação a outro principal e (4) replicação.

Ao revogar consentimento/grant, bloquear novas operações dentro do escopo confirmado. Não afirmar remoção global por bloqueio local, evento emitido ou expiração lógica. Registrar separadamente estado de bloqueio e disposição por cópia, incluindo cache, índice, exportação e backup, segundo SPEC-005 e ADR-062; declarar descarte para cada cópia identificada, sem inferir que descarte de uma cópia remove as demais. Em partição, estado de autorização sem confirmação permanece desconhecido; qualquer uso offline permitido depende de política, grant, escopo e validade explicitamente aprovados. Limites de frescor, autoridade/epoch, janela de revogação e conteúdo já recebido seguem OPEN.

## 7. Ponto de acesso: contrato espacial

O identificador geográfico pertence à entidade do AP físico, não ao SSID. Registrar coordenada somente quando disponível e sem inventar precisão; preservar origem, qualidade, instante, responsável pelo cadastro/publicação quando aplicável e histórico de alterações. Sem coordenada confiável, representar ausência explícita. A consulta de posição exata requer opt-in específico e autorização do mapa. Identificadores brutos de redes e dados de visitantes ficam sujeitos à SPEC-009 e à política Wi-Fi. Este contrato depende da conciliação com RF-039, ADR-049/054 e revisões de Jouni/Bruce.

## 8. Falhas e recuperação

- Sem permissão, grant ou consentimento: não adquirir ou divulgar dentro do escopo negado; mostrar `denied` e preservar funções que não dependem dessa capacidade.
- Fonte indisponível ou não suportada: devolver estado correspondente, sem coordenada sintética; usar última posição apenas identificada como antiga e sujeita a autorização.
- Fonte conflitante ou estimativa sem incerteza interpretável: manter estado `unknown`/`unavailable` segundo decisão de contrato; não escolher silenciosamente a fonte mais precisa.
- Relógio/frescor não confiável: preservar ambos os tempos observáveis e marcar frescor desconhecido; não rebatizar o dado como atual. Regra de relógio continua OPEN.
- Partição: não alegar atualidade ou revogação global; revalidar autorização ao reconectar antes de nova divulgação/replicação sensível.
- Restauração: revalidar política e autorização por cópia antes de disponibilizar dados restaurados; a prova de restore pertence à SPEC-005.
- Erro de projeção: falhar fechado para a divulgação de precisão maior que a autorizada; manter observação segregada. Procedimento, logs e resposta operacional pertencem à SPEC-018/020.

## 9. Critérios de aceite planejados

1. Para fonte única, fusão, conflito, fonte sem suporte e ausência de posição, a resposta preserva fonte(s), instante medido/recebido, qualidade, incerteza e estado; não produz direção/ponto a partir de RSSI.
2. Para medição antiga ou fonte sem atualidade confirmada, consulta rotula `stale`/`unknown` conforme regra aprovada e não a apresenta como atual.
3. Para geofence com incerteza cruzando o limite, posição desconhecida ou dados esparsos sem suporte, retorna `indeterminate`; não emite entrada/saída categórica. Transição atrasada/replay não cria estado atual novo.
4. Para projeção pessoal em destino coarse, nenhuma superfície controlada expõe coordenada precisa ou metadado de reconstrução; região divulgada contém ponto verdadeiro e respeita limite nominal aprovado. Consultas repetidas/correlação são cobertas no plano sem alegar anonimato.
5. Para estado de autorização `unknown`/`stale`, leitura de localização precisa e sync privado são negados conforme contrato de identidade. Aquisição local offline permanece sem critério de PASS até política decidida.
6. Para AP sem localização confiável, mantém ausência explícita; com localização, vincula ao AP físico e preserva fonte/qualidade/tempo/histórico. Consulta exata depende de opt-in/autorização de RF-039.
7. Para revogação e cópias, impede nova operação no destino confirmado e diferencia bloqueio de acesso, expiração lógica e disposição efetivamente confirmada por cópia.
8. Para SOS, apenas destinatário e grant por incidente podem consultar posição precisa; fonte e idade são expostas ao destinatário autorizado. O fluxo completo aguarda SPEC-006.
9. Para ingestão, cada resposta declara `ack_class` e evidencia a fronteira local, autoridade ou materialização; nenhuma dessas classes é apresentada como entrega a dispositivo/destinatário ou ACK humano. Perda de confirmação resulta `unknown`.
10. Para cada classe/cópia de observação, histórico, geofence e derivados, escrita sem política aprovada da tupla ADR-062 falha fechada; descarte é declarado por cópia. Uma bridge que recebe localização declara `confidentiality=terminated` e a perda de precisão, sem alegar anonimato ou E2EE.
11. Para streams/cursors, duas partições com sequência igual não colidem; cursor sem partição ou de epoch incomparável é rejeitado. Consultas coarse repetidas não podem estreitar a posição sem limite.

Estes são critérios planejados, não roteiros executados. IDs formais de casos: NOT_ASSIGNED. Testes de produto: NOT_EXECUTED.

## 10. Casos planejados, sem IDs

- Observação GNSS recente, medida e recebida em instantes distintos; verificar metadados sem inferir precisão superior à fonte.
- Fusão GNSS/IMU e perda do GNSS durante percurso offline; verificar incerteza crescente e último ponto confirmado datado.
- RSSI Wi-Fi/BLE forte sem GNSS do observador; verificar que não resulta em ponto, direção ou posição de beacon.
- Fonte `unsupported`, permissão negada, fonte conflitante, relógio incerto e ausência de medição; verificar estados e fallback.
- Geofence nos lados do limite com área de incerteza cruzando-o, amostra esparsa, atraso e replay; verificar `indeterminate`/ausência de transição categórica.
- Tentativas de leitura por principal, tenant, finalidade, precisão ou destinatário não autorizados; verificar negativa sem revelar localização.
- Projeção coarse em API, chat/PTT, timeline, pesquisa/índice, cache, logs, exportação, bridge, federação e restauração; correlacionar consultas repetidas e verificar campos/representação por destino.
- Consentimento/grant revogado antes de nova divulgação, durante partição e após reconexão; verificar estados por ação/destino e que não há alegação de exclusão remota.
- Tentativa de persistir observação, histórico, evento/resultado de geofence e derivados sem política de retenção; verificar falha fechada. Com política, verificar gatilho e declaração de descarte separada para cada cópia, sem inferir remoção de cópias restantes.
- Ingestão com confirmação em cada fronteira (`persisted_local`, `replicated_to_authority`, `materialized`), confirmação perdida, e tentativa de exibir entrega/ACK humano com base no ACK técnico; verificar classe e escopo, resultado `unknown` quando cabível e ausência de escalada de ACK.
- Stream de observações com mesma sequência em partições distintas; cursor sem partição, epoch incompatível e replay após mudança de epoch; verificar namespace canônico e rejeição conforme ADR-057.
- Bridge que recebe projeção coarse e, em cenário formalmente autorizado, localização precisa; verificar `confidentiality=terminated`, precisão original e transformada/perda declaradas, e nenhuma alegação de anonimato ou E2EE.
- Consultas repetidas e correlacionadas à projeção coarse, inclusive em fronteiras e instantes sucessivos; verificar que composição não permite aumentar precisão sem limite. O critério numérico e o mecanismo permanecem OPEN.
- AP físico sem posição; posição proveniente de fonte com qualidade; prova/opt-in ausente ou revogado; alteração de posição; consulta do mapa por administrador autorizado, visitante e usuário sem direito.
- SOS com grant específico e sem grant; destinatário autorizado e não autorizado; posição antiga/ausente; verificar origem/idade e ausência de divulgação a superfícies comuns.

Aplicabilidade exata de cada caso, versões de SO, dispositivos e ambientes de teste serão detalhados por responsáveis de produto/plataforma. Sem especificação de sensor ou aparelho concreto nesta versão.

## 11. Decisões OPEN e dependências

| ID | Questão/alternativas | Responsável proposto | Condição de fechamento |
|---|---|---|---|
| GEO-OPEN-01 | Modelo geométrico, CRS, geometria de geofence e transformações válidas | Paul + Gregor | Contrato espacial consistente com fontes/referências e aceites cobrindo bordas; revisão independente |
| GEO-OPEN-02 | Limites de frescor por finalidade/fonte, relógio e dados fora de ordem | Kim + produto Guardian + Gregor | Evidência e política aprovadas; critério de stale/unknown reproduzível sem número inventado |
| GEO-OPEN-03 | Geofence: histerese, deduplicação, repetição, coalescência/cadência e notificação | Guardian/Lea + Bruce + Paul | Alternativas, impacto de inferência, valores e comportamento autorizados pelo usuário/produto |
| GEO-OPEN-04 | Fusão, conflito, propagação/calibração de incerteza e significado de confiança | Gregor + Jack + responsáveis de plataforma + Paul | Contrato por fonte e plataforma, casos e evidência de validação futura definida |
| GEO-OPEN-05 | Matriz de projeção por API, armazenamento, cópia e destino; frequência/estabilidade; limite e mecanismo contra aumento de precisão por consulta repetida | Bruce + Kim + Martin/Michael + Gregor | Destinos e cópias enumerados, limite/retenção e controle de composição aprovados e revisão de privacidade |
| GEO-OPEN-06 | Aquisição offline sob credencial/frescor desconhecido, inclusive fronteira SOS | Kim + Nancy + Alan; Bruce/Paul consultados | Decisão explícita por operação/destino, grant/validade e compatibilidade com contratos; não presumir exceção SOS |
| GEO-OPEN-07 | Política ADR-062 por finalidade, classe, cópia, jurisdição, prazo e gatilho; descarte de observações, histórico, geofence e derivados por cópia, incluindo backups, índices e exportações | Michael + Bruce + Kim | Política aprovada por combinação aplicável; prazos/jurisdições/gatilhos decididos, cópias enumeradas e evidência de disposição/restore definida; sem política, persistência bloqueada |
| GEO-OPEN-08 | AP: proveniência, prova de administrador, opt-in, qualidade e mapa exato | Jouni + Kim + Bruce + Paul | Conciliação RF-038/RF-039 e ADR-049/054, sem conflitar com privacidade Wi-Fi |
| GEO-OPEN-09 | Escopo de requisitos e aceites formais por plataforma/produto | Ada + Margaret + responsáveis de plataforma | Índice central atribui SPEC/casos, confirma principais e cobre Personal/Teams/Node/Platform e plataformas aplicáveis |
| GEO-OPEN-10 | Interop do contrato com GMP/eventos/sync, posição/cursor ADR-057, ACK de ingestão ADR-058, confidencialidade de bridge ADR-060, erro/API e consentimento | Graydon + Martin + Gregor + Kim | Contratos revisados e versionados; classes/evidências, campo de confidencialidade e namespace/cursor coerentes; referências existentes verificadas; sem inventar endpoint |

Os IDs GEO-OPEN identificam lacunas locais desta SPEC, não requisitos RF/RNF, tickets ou decisões do produto. Cada responsável deve confirmar a atribuição. A lista não afirma que planejamento integral foi fechado.

## 12. Fontes e limites

- `docs/README.md`: hierarquia documental; arquitetura de referência e ADRs prevalecem.
- `docs/00-produto/requisitos.md`: RF-009 (linha 15), RF-029 (linha 58), RF-038 (linha 72), RF-039 (linha 73), RF-021, RF-026 e RNF-003/015/017.
- `docs/06-planejamento/matriz-rastreabilidade.md`: integração da cobertura dos requisitos, aceites propostos e estado NOT_ASSIGNED/NOT_EXECUTED.
- `docs/06-planejamento/decisoes.md`: ADR-049 e ADR-054 aprovadas; ADR-055 para plataformas, com seleções não validadas preservadas como propostas.
- `docs/02-dominio/modelo-dados.md` §§ Tabelas centrais, Origem e precisão da localização; descreve observações e dados propostos, não implementação.
- `docs/02-dominio/wifi-modelo-dados.md` §§ Inventário local e Governança de dados no mapa; não aplicar 50 km de localização pessoal automaticamente ao AP.
- `docs/01-arquitetura/guardian-localizacao-trajetoria.md` §§ Regras de qualidade, Privacidade por destino, Processamento e consumo, API conceitual; API é conceitual.
- `docs/04-seguranca/politica-localizacao.md` §§1–5; política de região coarse, separação SOS, consentimento, retenção e critérios planejados.
- `docs/07-contratos/identidade-autorizacao.md` §§115–118, 145, 173, 219, 232, 238–247; negativas de localização precisa/sync em estado `unknown`/`stale`; aquisição nova local não resolvida por essas regras.
- `docs/06-planejamento/pendencias.md`: pendências de energia, restrições do SO e decisões operacionais; sem preencher limiares ausentes.
- `SPECS-EM-ANDAMENTO/contribuicao-paul-ramsey.md`, hash no cabeçalho; `contribuicao-bruce-schneier.md`, hash no cabeçalho. Bruce §§8–9 identifica quatro lacunas; §15 re-revisa os deltas. Ambos são pareceres documentais, sem prova de implementação.

Não verifiquei execução, código, banco, APIs existentes, hardware, comportamento de sistema operacional, testes de produto, custos, conformidade legal ou remoção de dados. A SPEC permanece parcial até resolver OPENs e receber revisão. DRAFT não significa aprovado; aprovação normativa, implementação e validação permanecem estados distintos.

## Casos por decisão (ADR-056…063)

Casos documentais planejados; nenhum parâmetro ausente foi preenchido. ID formal: `NOT_ASSIGNED`. `Estado` observado em todos: `NOT_EXECUTED`. Quando um parâmetro estiver OPEN, o esperado é negar ou informar desconhecido, nunca um número inventado. Nenhum caso constitui aprovação, implementação ou evidência.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S011-056-P1 | ADR-056 | Perfil de protocolo `geo-stream` sem janela declarada (OPEN); cliente N e cliente N-1 | Ingestão de observação por cada cliente | Esperado reformulado com parâmetro ausente: resposta com `param_not_configured`, `required_profile` vazio e compatibilidade exibida como desconhecida; nenhuma ingestão aceita como veredito. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED |
| S011-056-N1 | ADR-056 | Perfil sem feature crítica comum com o emissor (ex.: posição canônica de evento) | Ingestão por cliente incompatível | Rejeição antes do efeito: zero linhas de observação, zero evento em outbox, resposta com código de incompatibilidade e `required_profile` preenchido; nenhum downgrade silencioso | N | NOT_EXECUTED |
| S011-056-B1 | ADR-056 | Registro de observação antigo com campo numérico desconhecido para o binário novo | Leitura e regravação do registro | Campo desconhecido preservado no round-trip; número de campo não alterado nem reutilizado; registro removido aparece como `reserved` | B | NOT_EXECUTED |
| S011-056-F1 | ADR-056 | Janela numérica de suporte OPEN; consulta de perfil durante ingestão falha | Ingestão com autoridade de perfil indisponível | Resultado `outcome_unknown`; nada materializado; nenhuma aceitação presumida; cliente mantém observação pendente local | F | NOT_EXECUTED |
| S011-057-P1 | ADR-057 | Stream com duas partições ativas e mesma `seq` | Consulta paginada por cursor válido com `partition` | Posições `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` distintas; eventos retornados em ordem canônica sem colisão | P | NOT_EXECUTED |
| S011-057-N1 | ADR-057 | Cursor legado sem `partition` ou com `authority_epoch` incomparável | Reuso do cursor em consulta de observações | Rejeição com erro de cursor (sem partição / epoch incomparável); zero eventos retornados; cursor não é traduzido | N | NOT_EXECUTED |
| S011-057-F1 | ADR-057 | `authority_epoch` mudou entre cursor emitido e replay | Replay do cursor após troca de epoch | Falha fechada; nenhum evento reentregue como atual; reconciliação só por operação explícita e autenticada | F | NOT_EXECUTED |
| S011-057-B1 | ADR-057 | Cursor no fim do stream, sem novos eventos | Avanço de uma página | Lista vazia com cursor preservado; nenhum evento fabricado; `dispatch_epoch` e `event_sequence` não são gravados como `authority_epoch` | B | NOT_EXECUTED |
| S011-058-P1 | ADR-058 | Ingestão com commit durável local e depois commit na autoridade da partição | Envio de observação até cada fronteira | `ack_class=persisted_local` com `last_contiguous_seq` preenchido; depois `ack_class=replicated_to_authority`; nenhuma das classes vira `delivered_to_device` | P | NOT_EXECUTED |
| S011-058-N1 | ADR-058 | Resposta com `ack_class=persisted_local` | Tentativa de renderizar/derivar entrega a destinatário ou ACK humano | Escalada rejeitada; superfície mostra apenas a classe recebida; nenhum campo `delivered`/`acknowledged_human` é preenchido por ausência de erro | N | NOT_EXECUTED |
| S011-058-F1 | ADR-058 | Commit local ocorreu, confirmação se perdeu por timeout | Consulta de resultado após timeout | Resultado `unknown`/`outcome_unknown`; observação permanece pendente; nenhum estado de sucesso por omissão | F | NOT_EXECUTED |
| S011-058-B1 | ADR-058 | Resposta de ingestão sem campo `ack_class` | Consumo por cliente/UI | Tratada como `unknown`; bloqueada para exibição de entrega; nenhuma classe inferida por tempo | B | NOT_EXECUTED |
| S011-059-P1 | ADR-059 | Offline; grant assinado válido para "registrar observação de localização local"; epoch comparável, sem revogação conhecida; **limite de frescor não decidido** (OPEN) | Registro de observação local | Esperado reformulado com parâmetro ausente: frescor reportado como `param_not_configured`; observação mantida como pendente local com `authorization=unknown`, sem `authorization=valid` presumido; nenhuma divulgação a outro principal e nenhum prazo inventado. `S011-059-B1` cobre a negativa quando a política exigir frescor decidido | P | NOT_EXECUTED |
| S011-059-N1 | ADR-059 | Offline; pedido de divulgar localização precisa ou de mutação federada | Solicitação da operação não enumerada | Negado com estado `denied`; zero entradas na outbox; nenhuma exceção para SOS presumida | N | NOT_EXECUTED |
| S011-059-F1 | ADR-059 | Grant `stale`, `unknown`, relógio incerto ou revogação conhecida recebida após snapshot | Nova tentativa de operação condicional | Negação com `authorization=unknown`/`stale`; operação não enfileirada; nenhum número de frescor suposto | F | NOT_EXECUTED |
| S011-059-F2 | ADR-059 | Restore de snapshot antigo contendo grant ainda na validade, ou grant com `epoch` anterior | Uso do grant restaurado | Anti-rollback detectado: grant negado, contador monotônico não regride, operação recusada | F | NOT_EXECUTED |
| S011-059-B1 | ADR-059 | Limite numérico de frescor OPEN | Operação condicional avaliada | Negação com `freshness_limit=not_decided` informado; nenhum valor inventado; comportamento fail-closed registrado | B | NOT_EXECUTED |
| S011-060-P1 | ADR-060 | Bridge autorizada recebe projeção coarse de localização | Consulta da resposta e da UI da perna | `confidentiality=terminated` presente na resposta e na UI, com transformação aplicada e perda de precisão declaradas | P | NOT_EXECUTED |
| S011-060-N1 | ADR-060 | Canal E2EE entre endpoints nativos; servidor sem chave | Tentativa de leitura de conteúdo de localização no servidor e alegação de E2EE sobre artefato com perna terminadora | Servidor não decifra; alegação de E2EE rejeitada pela API; artefato exibido como `terminated` | N | NOT_EXECUTED |
| S011-060-F1 | ADR-060 | Cópia de derivado de localização sem metadado `confidentiality` | Consumo do derivado | Tratado como `confidentiality=terminated`; ausência não suprida por cache, versão anterior ou ausência de erro | F | NOT_EXECUTED |
| S011-060-B1 | ADR-060 | Bridge recebe localização sem degradação aplicada | Publicação da projeção | `confidentiality=terminated` e declaração explícita de perda zero de precisão; nenhuma alegação de anonimato | B | NOT_EXECUTED |
| S011-061-P1 | ADR-061 | Serviço de localização com transação única de domínio e `outbox` na mesma transação | Ingestão de observação e commit | Estado da observação e evento de outbox commitados juntos; publicação assíncrona após commit; projeção aplicada com idempotência | P | NOT_EXECUTED |
| S011-061-N1 | ADR-061 | Replay de evento para reconstruir projeção | Reprocessamento de evento já consumido | Nenhum efeito externo reemitido (bridge, rádio, notificação); contador de efeitos externos inalterado | N | NOT_EXECUTED |
| S011-061-B1 | ADR-061 | Duplicata do mesmo `event_id` de observação/geofence no outbox | Replay do outbox | Uma única linha de observação e um único evento de geofence; duplicata descartada pela chave idempotente e registrada; nenhuma transição de geofence emitida duas vezes | B | NOT_EXECUTED |
| S011-061-F1 | ADR-061 | Crash entre commit e publish | Reinício do serviço com outbox pendente | Outbox reprocessada após retomada; observação não duplicada pela chave idempotente; nenhuma perda silenciosa | F | NOT_EXECUTED |
| S011-061-F2 | ADR-061 | Autoridade de escrita `stale`/`unknown` ou fence inválido | Mutação forte de observação/histórico | Bloqueio com erro explícito de autoridade; nenhuma escrita aceita; leitura permanece com estado incerto declarado | F | NOT_EXECUTED |
| S011-062-P1 | ADR-062 | Política com `(finalidade, classe, cópia)` declarados, mas **jurisdição, prazo e gatilho não decididos** (OPEN) | Gravação de observação e derivado de geofence | Esperado reformulado com parâmetro ausente: gravação aceita com `retention_policy_ref` presente; prazo, jurisdição e gatilho reportados como `param_not_configured`; nenhuma expiração automática anunciada e nenhuma degradação apresentada como conformidade | P | NOT_EXECUTED |
| S011-062-N1 | ADR-062 | Classe/cópia sem política aprovada | Tentativa de persistir histórico ou derivado | Falha fechada com erro explícito de política ausente; zero linhas persistidas; operação limitada ao efeito imediato e degradação declarada | N | NOT_EXECUTED |
| S011-062-F1 | ADR-062 | Política expirada; cópias enumeradas incluem cache, índice, réplica e backup | Gatilho de descarte acionado | Descarte registrado por cópia com estado individual; cópia sem confirmação permanece `pending`; nenhuma alegação de remoção global | F | NOT_EXECUTED |
| S011-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão registrados sobre dado de auditoria | Consulta de acesso comum ao dado retido | Retenção estendida sob prazo próprio; acesso comum não reativado; revisão do hold registrada | B | NOT_EXECUTED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §9, §10 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-009 | S011-RF009-P1 (novo) | S011-RF009-N1 (novo) | S011-057-B1 | S011-058-F1 | Observação, estimativa, histórico e geofence preservam fonte, instante e incerteza; regra de frescor e retenção seguem `OPEN` |
| RF-029 | S011-RF029-P1 (novo) | S011-RF029-N1 (novo) | S011-059-B1 | S011-RF029-F1 (novo) | Fusão declara linhagem e incerteza; RSSI não vira distância, ponto ou direção |
| RF-038 | S011-RF038-P1 (novo) | S011-RF038-N1 (novo) | S011-056-B1 | S011-062-F1 | Posição pertence ao AP físico, com origem, qualidade, responsável e histórico |
| RF-039 | S011-RF039-P1 (novo) | S011-RF039-N1 (novo) | S011-060-B1 | S011-062-F1 | Opt-in específico e autoridade vigente; interface com SPEC-009/RF-039 |
| RF-021 | S011-059-P1 (reformulado) | S011-059-N1 | S011-059-B1 | S011-059-F1, S011-059-F2 | Aquisição, leitura local, divulgação e replicação são autorizações distintas |
| RNF-003 | S011-RNF003-P1 (novo) | S011-RNF003-N1 (novo) | S011-062-B1 | S011-061-F2 | Isolamento por tenant e escopo; negativa não revela a existência do dado |
| RNF-015 | S011-RNF015-P1 (novo) | S011-RNF015-N1 (novo) | S011-062-B1 | S011-060-F1 | Dados administrativos de AP e visitantes protegidos por função/tenant e retenção |
| RNF-017 | S011-RNF017-P1 (novo) | S011-059-N1 | S011-059-B1 | S011-059-F1, S011-061-F1 | Partição e revogação: estado datado, reconciliação antes de nova divulgação |
| RF-026 | S011-RF026-P1 (novo) | S011-RF026-N1 (novo) | S011-059-B1 | S011-058-F1 | Apenas interface SOS; sequência e estados pertencem à SPEC-006 e ACK técnico não prova atendimento |

Casos novos, descritos em uma linha: **S011-RF009-P1** — observação recente com fonte, qualidade e instante medido: estado `available` exibido com esses metadados, sem alegar precisão superior à fonte. **S011-RF009-N1** — geofence com área de incerteza cruzando o limite: `indeterminate`, nenhuma transição categórica e nenhum evento novo por replay. **S011-RF029-P1** — fusão GNSS/IMU: estimativa com linhagem das fontes e incerteza resultante declarada. **S011-RF029-N1** — RSSI de Wi-Fi/BLE forte sem posição própria do observador: proximidade apenas; nenhum ponto, direção ou posição do beacon. **S011-RF029-F1** — perda de GNSS offline durante percurso: último ponto confirmado datado, incerteza crescente declarada, nenhuma posição atual presumida. **S011-RF038-P1** — AP com posição medida e cadastrada: coordenada vinculada ao AP físico com origem, responsável e histórico. **S011-RF038-N1** — AP sem coordenada confiável: ausência explícita, sem precisão inventada. **S011-RF039-P1** — opt-in exato vigente e autoridade atual: mapa retorna posição do AP no escopo autorizado. **S011-RF039-N1** — opt-in ausente ou revogado: projeção coarse/hidden apenas se aprovada; posição exata, BSSID bruto e visitante não expostos. **S011-RNF003-P1** — leitura por destinatário autorizado com finalidade, precisão e escopo compatíveis: dado entregue com `policy_id/version` da ADR-065 aplicado e classe/cópia/retention declarados na resposta; sem política vigente para essa cópia, a leitura responde `RETENTION_POLICY_MISSING` e é negada. **S011-RNF003-N1** — leitura por principal, tenant, finalidade ou precisão não autorizados: negada sem revelar localização nem existência. **S011-RNF015-P1** — consulta administrativa de AP autorizada: metadado mínimo e trilha de auditoria registrada. **S011-RNF015-N1** — visitante ou operador fora do AP administrado: leitura negada e auditada. **S011-RNF017-P1** — partição regional com fila local dentro de grant vigente: pendência declarada e dados datados, sem afirmar entrega. **S011-RF026-P1** — incidente SOS com grant específico: destinatário autorizado consulta posição precisa com fonte e idade. **S011-RF026-N1** — chat, timeline ou bridge sem grant de incidente: posição precisa de SOS não divulgada.

**Sem caso por falta de decisão ou de escopo:** ADR-063 não tem caso local nesta SPEC — a distribuição assinada é de artefatos de servidor/cliente/firmware e pertence a SPEC-017/018/019 e às SPECs de cliente (015/016); esta SPEC não define artefato distribuído próprio. ADR-056 depende da janela numérica, que permanece decisão de produto: o caso S011-056-F1 cobre apenas o comportamento de desconhecido. Frescor (ADR-059) e prazos/jurisdições (ADR-062) seguem OPEN e por isso não têm caso positivo com valor.
