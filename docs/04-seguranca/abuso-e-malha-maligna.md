# Abuso por usuário e por nó malicioso — modelo de ameaça e controles

> **Status:** proposta de segurança do CTO (18/11/2026), derivada de ADR-030/044/046/071/072 e dos invariantes da rede-alvo. Não substitui a política de segurança normativa; controles marcados **existe** estão implementados e testados, os demais são backlog com dono.
> **Pergunta:** como impedir que usuários maliciosos usem nossos servidores, e que nós de terceiros usem a federação para fins maliciosos.

## 1. Premissa

Não existe "rede segura". Existe **superfície de abuso com custo**: cada ator paga um preço (identidade, autorização, cota, auditoria, revogação) para causar dano, e o dano possível é limitado por escopo. A defesa é em camadas que não dependem uma da outra.

## 2. Atores e o que cada um pode tentar

| Ator | Objetivo típico | Primeira defesa |
|---|---|---|
| Usuário autenticado abusivo | rastrear pessoa, SOS falso, flood de canal | consentimento, cota, correlação, revogação de grant |
| Tenant hostil (empresa) | vigiar funcionário, exportar dado | escopo por finalidade, minimização, auditoria, retenção |
| Atacante sem conta | exaurir recurso, enumerar identidade | autenticação obrigatória, anti-enumeração, rate limit, DDoS |
| **Operador de nó de terceiro** | scraping, Sybil, relay para lavar abuso | ingresso assinado, escopo por nó, quarentena, revogação |
| Nó comprometido | forjar evento/config, exfiltrar | assinatura por evento, epoch/fencing, detecção de anomalia |
| Cliente falso/clonado | replay de grant, dispositivo roubado | alta marca, anti-rollback, vínculo de dispositivo |

## 3. Portão de entrada da malha (nó de terceiro)

**Existe:** identidade Ed25519 de nó, manifesto assinado, `join_request` que exige assinatura válida e `issued_at` na janela, aprovação com escopo `federation:admin`, `trust_state` com transições fechadas, banco sem coluna de segredo.

**Backlog obrigatório antes de abrir federação a terceiros:**

| Controle | Regra |
|---|---|
| Aprovação assinada por nó autorizado (M1b) | aprovação humana sozinha não basta (emenda ADR-071) |
| Quórum para mudança de confiança | cluster com `control-voter` exige 3/5 |
| Escopo por nó, com validade | nó não recebe classe/tenant que não lhe foi concedido; expira e renova |
| Estado de **provação** | nó novo entra `probation`: sem tráfego de usuário por janela declarada, só sync e health |
| Limite por operador | N nós por operador/contrato — Sybil exige custo real |
| Revogação assinada com epoch | revogação anterior não reativa acesso |
| Sem auto-ingresso | federação não tem cadastro público; ingresso é convite/aprovação |
| Quarentena automática | anomalia (epoch errado, assinatura inválida repetida, fan-out anômalo) suspende o nó antes de revisão humana |

## 4. Controles dentro dos nossos servidores

**Existe:** credencial obrigatória com escopo por rota (401/403 testados), tenant derivado do grant, isolamento entre tenants (404 para incidente alheio), transição atômica de incidente, trilha `sos_ack` append-only, retenção com FK, posição de evento com tupla canônica, log com `token_id` e sem token.

**Backlog, em ordem de risco:**

| # | Controle | Por que |
|---|---|---|
| 1 | **Cotas por grant/tenant**: requisições/s, eventos/s, tamanho de payload, conexões por principal | hoje um grant válido pode inundar o servidor |
| 2 | **Cota reservada de SOS** e admissão por classe | impedir que abuso consuma o orçamento de emergência |
| 3 | **Antiabuso de SOS**: dedupe por incidente, limite de aberturas por principal/janela, sinalização de repetição | SOS falso em massa satura atendimento |
| 4 | **Anti-stalking de localização**: limite de consultas por unidade de tempo, detecção de correlação entre sessões, coarse por padrão | 50 km não impede inferência por consulta repetida |
| 5 | **RLS no Postgres** por tenant como defesa em profundidade | bug de código não deve virar vazamento entre tenants |
| 6 | **Enumeração**: erros e latências indistinguíveis, paginação limitada, sem lookup por alias fora de escopo | hoje 404 já ajuda, falta teste de canal lateral |
| 7 | **Convites e canais**: limite de convite por principal, bloqueio entre usuários, denúncia | assédio e spam de convite |
| 8 | **Quarentena de tenant**: congelar escrita, preservar leitura, registrar motivo | resposta a incidente sem desligar região |

## 5. Nó malicioso dentro da malha

| Ataque | Controle necessário |
|---|---|
| Ingerir evento falso em nome de outro nó/tenant | **evento assinado pelo nó de origem** + verificação de escopo `(node_id, tenant, stream)` + epoch |
| Replay de evento antigo | `seq`/`epoch` monotônicos + janela + quarentena de epoch incomparável (já há base) |
| Scraping de dado fora de escopo | consulta com escopo por nó; negar e auditar; alarme em padrão de varredura |
| Relay para lavar abuso | relay autorizado com quota e log; origem preservada ponta a ponta |
| Envenenar configuração | bundle assinado com quórum; nó receptor recusa bundle sem assinatura válida (M3) |
| Fingir ponte de rádio/identidade externa | UDNL preserva identidade externa; nunca promove a identidade GodMesh (ADR-037) |
| Saturação por fan-out | limite de conexões por peer, backpressure por classe, corte de peer abusivo |

**Regra dura:** evento sem assinatura de nó autorizado **não entra no log durável**. Sem isso, um nó de terceiro comprometido contamina o histórico — e histórico contaminado é pior que histórico ausente.

## 6. Detecção e resposta

- **Auditoria mínima por evento de segurança**: emissão/revogação de grant, aprovação/rejeição de nó, negação de escopo, falha de assinatura, mudança de retenção. Sem conteúdo de usuário.
- **Sinais de anomalia**: taxa de 401/403 por `token_id`, taxa de SOS por principal, consultas de localização por sessão, fan-out de eventos por nó, idade de lease, divergência de epoch entre peers.
- **Kill switch por escopo**: revogar grant (segundos), revogar nó (próxima busca + lease), suspender tenant (escrita congelada), desabilitar bridge (sem afetar canal nativo).
- **Resposta a abuso**: procedimento escrito com prazos, notificação ao titular quando aplicável, preservação de evidência e revisão humana antes de banimento definitivo.

## 7. O que NÃO resolve (e não deve ser vendido como solução)

- mTLS entre nós não impede nó autorizado de mentir sobre o conteúdo que assina.
- 50 km não é anonimato; é redução de precisão.
- E2EE não impede abuso de metadados (quem fala com quem, quando).
- Revogação não apaga dado já entregue a terceiro.
- Cota não impede abuso lento e distribuído; isso exige correlação e revisão humana.

## 8. Fatias propostas

| Fatia | Entrega | Depende de |
|---|---|---|
| S-sec-1 | cotas por grant/tenant + limites de payload + admissão por classe | nada |
| S-sec-2 | evento assinado pelo nó + escopo `(node, tenant, stream)` no ingest | M1 identidade |
| S-sec-3 | provação de nó + quarentena automática + revogação assinada | M1b aprovação assinada |
| S-sec-4 | antiabuso de SOS e de localização (limites + correlação) | S-sec-1 |
| S-sec-5 | RLS por tenant + teste negativo de vazamento | nada |
| S-sec-6 | bundle de config assinado com quórum (M3 da rede) | M2 transporte |
