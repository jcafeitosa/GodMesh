# Contrato de credenciais CrowdFind

> **Perfil documental v1, proposta para revisão.** Detalha o token rotativo BLE, a autorização do observador e a resolução de avistamentos para as ADR-043/044. O desenho é proposto; nenhuma credencial, chave, API móvel ou comportamento de hardware foi implementado ou homologado.

## Limite de confiança

Um beacon anuncia somente um token opaco de curta duração. O pacote BLE não contém `gmi_id`, GMC, nome, telefone, coordenadas, tenant ou ID de hardware persistente. O token prova apenas que a emissão foi autorizada por uma sessão Guardian; não prova que o portador esteve fisicamente perto do beacon nem identifica uma pessoa para o observador.

Um observador precisa de identidade autenticada e permissão `crowdfind.observe`. A pessoa que emite precisa de sessão Guardian ativa e consentimento independente `guardian.beacon.emit`. A autorização `guardian.location` ou `guardian.invitee.precise` não concede automaticamente permissão para emitir beacon ou observar.

O serviço de localização valida e resolve tokens. Apenas destinatários da sessão com concessão `guardian.precise.read` ou o escopo aprovado para localização coarse recebem uma observação. O observador não recebe identidade, sessão privada, lista de contatos ou autorização de rastreamento. A resolução não sai para tópicos globais, API pública, WebFinger ou bridge.

## Identificadores e token BLE

### Forma no anúncio

**Proposta:** `gmp.beacon.rotating.v1` leva uma versão e um valor aleatório opaco de 128 bits. Representação lógica:

| Campo | Tipo | No ar? | Regra |
|---|---|---:|---|
| `protocol_version` | `uint8` | Sim | Versão do formato; valor ilustrativo `1` |
| `token` | 16 bytes | Sim | Valor gerado por CSPRNG, independente para cada período; 128 bits é proposta, não requisito de padrão BLE |
| `session_ref` | bytes opacos | Não | Índice privado do serviço para a sessão Guardian |
| `token_hash` | 32 bytes SHA-256 | Não | Índice no serviço; não é transmitido |
| `not_before`, `expires_at` | timestamps UTC | Não | Janela de validade registrada no serviço |
| `token_epoch` | inteiro sem sinal | Não | Geração da lista para invalidação/revogação |

O serviço armazena `SHA-256(token)` em vez do token em texto. O beacon recebe uma lista limitada de tokens válidos para a sessão e os armazena em área local protegida pelo SO. Cada item é independente e não deriva publicamente do anterior. O cliente apaga tokens expirados quando possível; o serviço não aceita token fora da validade mesmo se o cliente ainda o anunciar.

128 bits aleatórios tornam busca exaustiva não prática sob as hipóteses usuais de CSPRNG, mas o valor exato, fonte de entropia e formato de anúncio precisam de revisão e homologação. Um atacante ainda pode capturar e retransmitir um token válido. BLE convencional não prova distância nem resiste sozinho a relay em tempo real.

### Validade e rotação

Cada token é válido no intervalo semiaberto `[not_before, expires_at)`. Nenhum token pode durar além da sessão Guardian. O token seguinte deve ser diferente e não revelar relação criptográfica ao portador sem a chave do servidor. A autoridade mantém lookup de hash para sessão e destinatários. Token repetido por observadores distintos pode ser legítimo; reutilização do mesmo `event_id` pelo mesmo principal é duplicata segundo o contrato de eventos e sync.

A duração precisa ser escolhida. `ttl_seconds=900` no exemplo antigo de `gmp.crowdfind.sighting.v1` é ilustrativo, não parâmetro aprovado. Recomenda-se avaliar janelas de até 900 segundos como ponto inicial de discussão. Janelas menores reduzem correlação temporal e o tempo de exposição de token capturado, mas aumentam chamadas de renovação, uso de rádio e falha sob conexão intermitente. Janelas maiores apoiam períodos offline, mas ampliam o tempo de replay e correlação. Nenhum valor habilita produção sem validação móvel e aprovação do limite de revogação.

O serviço pode provisionar uma quantidade finita de tokens para uma sessão ativa. A lista deve vencer antes ou junto da sessão. O cliente não estende validade localmente. Quando a sessão termina, é pausada ou revogada, o cliente para o anúncio quando o SO permitir e o serviço revoga o epoch/lista. Um beacon desconectado pode continuar anunciando token já provisionado até expirar; não se promete revogação remota instantânea. O tempo máximo de exposição depende do `expires_at` configurado.

## Emissão e observação

Os caminhos a seguir são operações lógicas; Graydon Hoare define o transporte e nomes finais no contrato GMP API v1.

| Operação lógica | Chamador, scope e vínculo | Resultado |
|---|---|---|
| Emitir lote de tokens | Pessoa dona da sessão, sessão `active`, prova de posse do device, consentimento vigente e `guardian.beacon.emit` | Lote privado com `batch_id`, tokens, janela, epoch e chave pública de destino; nunca GMC/GMI no anúncio |
| Enviar avistamento | Observer `device` autenticado, `crowdfind.observe`, token observado e prova de posse do device | Aceitação, duplicata, token expirado/revogado ou rejeição; sem identidade do titular para o observer |
| Consultar avistamentos | Titular/contato designado, grant de sessão vigente e escopo de precisão correspondente | Só observações autorizadas, com incerteza, fonte e frescor; sem histórico fora do prazo de política |
| Revogar beacon | Titular da sessão autenticado ou autoridade de sessão sob regra aprovada | Interrompe novos tokens e marca epoch anterior revogado no serviço; acesso a observações antigas segue retenção |

Antes da emissão, a autoridade verifica: session ID existe e pertence ao titular; estado é `active`; sessão não expirou; consentimento de emissão está ativo; device está ligado ao titular; tenant e região permitem a sessão; limites antiabuso estão configurados. Convite de Guardian sem `beacon_opt_in` não emite token.

Antes de aceitar observação, o serviço verifica: principal autenticado e device ativo; scope `crowdfind.observe`; proof of possession do token de acesso, se suportado pelo perfil API; evento dentro de validade; hash pertence a beacon permitido; sessão/epoch não revogado; `observed_at` dentro do limite máximo de chegada tardia configurado; payload respeita limites. A sessão pode receber relatos de vários observadores autorizados. Um observer não pode criar grant para si, nem resolver o token.

### Envelope de avistamento

Campos novos abaixo detalham o exemplo em `07-contratos/guardian-crowdfind.md`. O contrato de sync mantém autoridade sobre IDs, idempotência, replay e cursores; esta página não cria outro mecanismo.

| Campo | Tipo | Obrigatório | Origem e regra |
|---|---|---:|---|
| `type` | string | Sim | `gmp.crowdfind.sighting.v1` |
| `event_id` | UUID/event ID GMP | Sim | Único por tentativa lógica; sem reutilização para conteúdo diferente |
| `beacon_token` | 16 bytes | Sim | Valor lido do anúncio; somente no canal privado observer→resolver |
| `observer_ref` | principal tipado + device ref | Sim | Derivado da sessão autenticada, não texto não confiável do cliente |
| `observed_at` | RFC 3339 UTC | Sim | Hora declarada do sensor; não é autoridade para validade do token |
| `received_at` | RFC 3339 UTC | Sim | Atribuído pelo serviço no recebimento; o cliente não o escolhe |
| `location_source` | enum | Sim | `observer_gnss`, `observer_network`, `none`, `manual` |
| `position_ciphertext` | objeto cifrado ou `null` | Condicional | Presente só com permissão de localização apropriada; nunca usar localização do beacon |
| `uncertainty_m` | número positivo ou `null` | Sim | Incerteza da localização do observador, não do beacon; `null` se desconhecida |
| `confidence` | enum | Sim | `high`, `medium`, `low`, `unknown`; derivação ainda depende de qualidade validada |
| `token_epoch` | inteiro sem sinal | Sim | Epoch retornado na emissão; não é prova de localização |
| `proof_ref` | referência | Sim | Prova de identidade/posse e autorização sob contrato API; nunca a chave privada |

O payload privado identifica no mínimo a observação e seus dados de localização permitidos. O evento não contém GMC/GMI do titular, token em log aberto, coordenada do beacon, SSID, MAC/BSSID ou localização que o observador não autorizou. Se a origem não tem posição confiável, aceitar apenas um registro de proximidade sem coordenada.

**Proposta de proteção do payload:** usar HPKE conforme RFC 9180 para cifrar a posição do observador ao serviço destinatário. Começar a avaliação com `DHKEM(X25519, HKDF-SHA256)`, `HKDF-SHA256` e `AES-128-GCM`. Associar `event_id`, `token_epoch`, `location_source` e identificador de chave como dados autenticados. A lista final depende de compatibilidade, gestão de chave, rotação e revisão de segurança. HPKE protege até o serviço destinatário; não significa E2EE entre pessoas nem resolve retenção em claro após a descriptografia. Se perfil HPKE não for aprovado, o payload permanece indisponível até existir contrato de envelope seguro. Não enviar coordenadas precisas por fallback em texto claro.

## Autorização e resposta

`guardian.beacon.emit`, `crowdfind.observe`, `guardian.location`, `guardian.precise.read` e `incident.location.precise.read` são direitos separados. Um usuário comum da rede não tem permissão de leitura do avistamento só porque retransmitiu token. Contactos recebem observações somente enquanto sua grant de sessão permite. A posição precisa segue a política Guardian/SOS, localização coarse segue ADR-041. A auditoria registra quem consultou, qual sessão, finalidade/caso, horário e decisão, sem incluir token ou coordenada em log de aplicação.

| Resultado lógico | Condição | Ação |
|---|---|---|
| `accepted` | Token e observer válidos, sessão ativa, evento dentro da janela | Guarda e encaminha só à audiência autorizada |
| `duplicate` | Mesmo `event_id` para mesma tentativa | Retorna resultado idempotente, não cria segunda observação |
| `stale` | Token expirado, sessão encerrada, chegada atrasada além do limite ou freshness excedida | Não atualiza localização; não revela titular |
| `revoked` | Epoch/sessão/device/observer revogado | Nega observação e consulta; impede emissão nova |
| `unauthorized` | Consentimento, scope, associação ou audiência ausente | Nega; não distingue token de pessoa não visível |
| `invalid` | Token não emitido, payload inválido, prova ou assinatura incorreta | Nega, registra sinal de abuso sem copiar segredo |
| `authority_unavailable` | Estado atual não pode ser conferido | Não emite token nem mostra posição precisa; mantém separação local/offline definida abaixo |

O envelope HTTP/GMP de cada erro depende do contrato API. Erros de token para cliente sem visibilidade retornam resposta equivalente para `unknown`, `revoked` e titular não visível, para reduzir enumeração.

## Falhas, partição e revogação

- **Falha antes do lote:** não começar anúncio; mostrar `permission_required`, `offline`, `stale` ou `unsupported` conforme evidência do dispositivo.
- **Falha durante anúncio:** o telefone pode continuar com a lista local ainda válida. O cliente deve parar ao fim da lista ou sessão. O serviço rejeita tokens expirados.
- **Revogação online:** autoridade marca sessão/epoch e tokens como revogados; não aceita novos avistamentos e revoga grants de emissão/consulta conforme política. A cessação de rádio depende de comunicação com o aparelho e do SO; mostrar estado pendente se não houver confirmação.
- **Observer offline:** não publicar posição por rota alternativa. A fila local criptografada pode reter evento apenas se o contrato C10 e política de retenção aprovarem prazo e replay. Ao reconectar, o serviço valida `observed_at`, `received_at`, token e estado atual; observação fora da janela retorna `stale`.
- **Servidor de revogação desconhecido/stale:** negar emissão e consulta precisa. Relato enfileirado não equivale a avistamento entregue.
- **Backups, exportação e réplicas:** retenção e bloqueio após revogação dependem do contrato C12. Sem política de cópia, TTL e efeito da revogação aprovados, não declarar exclusão completa.
- **Abuso por relay:** alguém pode retransmitir token BLE válido para uma localização distante. O serviço pode limitar cadência por observer, sinalizar anomalia e exigir evidência de localização do observer, mas nenhuma dessas medidas transforma BLE em prova de distância.

## Chaves e privacidade

O modelo proposto usa tokens aleatórios; não há uma chave secreta de beacon que o observador precise possuir. A autoridade guarda somente hash do token mais a referência privada à sessão e horário de validade. Chaves de assinatura/HPKE do serviço têm `key_id`, finalidade, validade, status e rotação sob manifesto autenticado. A rotação de chave do servidor não alonga validade do lote já emitido. Tokens antigos deixam de ser aceitos ao expirar ou ao revogar epoch.

O serviço de localização é o único componente que resolve token para sessão. Serviço de analytics, logs, administração de tenant e gateways externos recebem apenas campos minimizados. O observador não vê identificador persistente do beacon. Repetição de horário, proximidade ou observações em sequência ainda pode permitir inferência por correlação externa. Não prometer anonimato.

Os prazos de token, ciphertext, observação, trilha de consulta, fila offline, backup e exportação ainda não foram aprovados. A política C12 precisa distinguir bloqueio de leitura após revogação de exclusão física no prazo legalmente permitido. Sem retenção e jurisdição definidas, produção fica bloqueada.

## Exemplos

### Aceito

Uma pessoa inicia sessão Guardian, aceita explicitamente `guardian.beacon.emit` e recebe um lote com expiração limitada. O telefone anuncia bytes aleatórios sem GMI/GMC. Um observer autenticado e opt-in envia token, hora e incerteza da própria localização. O servidor resolve o token, grava hora de recebimento, e entrega observação apenas ao contato com grant ativo.

### Negado: tentativa de descobrir titular

Um usuário sem `guardian.precise.read` envia token capturado para resolução. O endpoint não responde com GMI, GMC, nome, sessão ou status que diferencie token revogado de token desconhecido.

### Negado: consentimento ausente

Uma sessão Guardian está ativa, mas `beacon_opt_in=false`. A chamada de emissão falha. Ter `guardian.location` não concede `guardian.beacon.emit`.

### Negado: token velho durante partição

Um observer conserva um pacote e o envia depois da expiração da janela. O serviço retorna `stale`, mesmo se o `observed_at` informado pelo cliente disser que o pacote foi visto antes. Sem assinatura/relógio confiável e freshness de autoridade, o serviço não inventa horário.

### Limite: relay em tempo real

Um atacante transmite o token vigente por rádio/IP a outro local antes de expirar. A assinatura do serviço continua válida, mas não prova proximidade física. A observação permanece de baixa confiança ou é sinalizada; ela nunca substitui GNSS do titular nem justifica posição precisa do beacon.

## Casos de conformidade documental

Os casos abaixo são critérios futuros e não foram executados.

1. Varredura do payload BLE não encontra GMI, GMC, nome, telefone, coordenada, endereço, tenant ou ID permanente.
2. Dois tokens de uma mesma sessão e dois tokens de sessões distintas não revelam relação por inspeção de bytes; token fora da lista não resolve.
3. Sem sessão ativa, consentimento, vínculo do device e `guardian.beacon.emit`, nenhum lote é emitido.
4. `crowdfind.observe` separado é obrigatório; `guardian.beacon.emit` isolado não permite submeter avistamento.
5. Repetir o mesmo `event_id` não cria observação duplicada; novo `event_id` de outro observer autorizado pode ser aceito dentro da janela.
6. Token expirado, revogado, de epoch antigo ou desconhecido não retorna titular nem atualiza localização.
7. Observer sem grant não lê observação precisa; destinatário de outra sessão não a lê; cross-tenant é negado.
8. O evento não trata localização do detector como coordenada do beacon e preserva `uncertainty_m=null` quando qualidade não é conhecida.
9. Sem autoridade de revogação atual, o serviço não emite nem divulga localização precisa. Um evento offline atrasado excede a janela configurada e é marcado stale.
10. Um evento adulterado, ciphertext inválido ou chave fora de validade não é encaminhado ao titular.
11. Revogar sessão impede emissão nova, invalida tokens ainda conhecidos pelo servidor e encerra consulta sob grant; cliente offline para no fim do lote, sem promessa de cancelamento remoto imediato.
12. Um relay em tempo real não é reportado como prova de localização/proximidade.
13. Retenção, expurgo de réplicas e cópias seguem política aprovada, não o `ttl_seconds` de um exemplo.

## Decisões pendentes e alternativas

| Tema | Recomendação | Alternativa e impacto | Bloqueio |
|---|---|---|---|
| Token | Aleatório opaco por período, 128 bits como proposta, lookup por SHA-256 no serviço | Token derivado de segredo HMAC/HKDF reduz lote enviado, mas aumenta gestão de chave, revogação e risco de correlação após comprometimento | Aprovar tamanho/fonte aleatória e política de chave |
| Janela de rotação | Avaliar máximo de 900 s como ponto inicial não aprovado | Janela menor melhora limite de replay/correlação e aumenta carga/indisponibilidade; maior faz o oposto | Aprovar máximo, antecipação de lote e atraso permitido |
| Cifra de observação | Avaliar HPKE RFC 9180 para serviço de localização | TLS apenas reduz complexidade mas expõe plaintext nos intermediários que terminam TLS | Aprovar suite, ciclo de chaves e destinatário |
| Cache e partição | Negar emissão e consulta precisa sem freshness; aceitar backlog apenas sob C10 e prazo aprovado | Permitir backlog maior melhora funcionamento offline e amplia observação atrasada/revogada | C10, C12 e limite de revogação por classe |
| Tamanho BLE/OS | Formato lógico de 16 bytes mais versão | Service UUID e framing competem pelo limite de anúncio e variam por SO | Homologar versões, aparelhos, API e modo background |
| Resolução de relay | Rate limiting, reputação e sinais de anomalia | Distância física exige capacidade que BLE comum não fornece em todos os aparelhos | Não declarar detecção confiável sem hardware/estudo |
| Retenção | Minimizar tokens, payload, localizações e logs; prazo por categoria | Retenção maior ajuda abuso/auditoria e aumenta impacto de vazamento | País, lei, prazo, cópia/backup e autoridade não definidos |

Não foram escolhidos país, lei aplicável, licença, hardware, ofertas, carga, orçamento, SLO, RPO, RTO, prazo de retenção, atendimento humano nem SLA de revogação. Isto não substitui parecer jurídico nem homologação de aparelhos.

## Fontes oficiais consultadas

Consulta em 15/11/2026. Os textos oficiais limitam-se ao escopo publicado; disponibilidade no hardware e no produto GodMesh não foi testada.

- IETF, [RFC 9180 — Hybrid Public Key Encryption](https://www.rfc-editor.org/rfc/rfc9180.html), fevereiro de 2022. Define HPKE como KEM, KDF e AEAD combinados e suites registradas. Base da proposta de cifrar ao serviço; não define gestão de chave GodMesh nem E2EE a destinatários finais.
- IETF, [RFC 4086 — Randomness Requirements for Security](https://www.rfc-editor.org/rfc/rfc4086.html), junho de 2005. Orienta geração de valores não previsíveis; não fixa o tamanho de token GodMesh proposto.
- Apple, [Core Bluetooth](https://developer.apple.com/documentation/corebluetooth) e [Core Bluetooth Background Processing](https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html), consultados em 15/11/2026. Documentação atual e página de referência arquivada indicam diferenças/limites de background e publicidade; comportamento varia por versão, e a página arquivada não é garantia para versões atuais.
- Android Developers, [BluetoothLeAdvertiser API](https://developer.android.com/reference/android/bluetooth/le/BluetoothLeAdvertiser), [BLE background communication](https://developer.android.com/develop/connectivity/bluetooth/ble/background) e [Bluetooth permissions](https://developer.android.com/develop/connectivity/bluetooth/bt-permissions), consultados em 15/11/2026. Documentam limite de 31 bytes para anúncio nessa API e permissões/estratégias de background descritas. Limite real da combinação GodMesh não foi homologado.
