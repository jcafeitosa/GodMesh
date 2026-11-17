# Segurança e privacidade — GodMesh Wi-Fi Share

> **WIFI-SEC-001 · Versão 1.0 · 2026-11-15 · Política de produto, sujeita a avaliação jurídica por país.**

## 1. Quatro conjuntos de dados separados

1. **Observação privada:** SSID/BSSID observado, qualidade e instante no aparelho; rede de terceiros não vira ponto público.
2. **Cadastro de AP administrado:** coordenadas precisas e evidência de origem, administrador e controle, histórico e provas de posse; acesso restrito.
3. **Projeção do mapa:** dados escolhidos pelo responsável, incluindo **posição precisa se opt-in explícito**; sem GMI, identificação civil ou credenciais.
4. **Auditoria de acesso:** uso de Wi-Fi por grant, sessão e controlador, com acesso restrito aos envolvidos autorizados; não é histórico de navegação.

## 2. Prova de autoridade e publicação

- Ter a senha de uma rede salva, observar o BSSID ou conseguir conectá-la **não** prova direito de compartilhá-la.
- O responsável pareia o controlador, demonstra competência administrativa ou apresenta delegação verificável.
- `auto_share=enabled` representa preferência e **não publica redes de terceiros**. Publicação só começa após `owner_verified` e concessão específica de visibilidade, incluindo publicação geográfica exata separada.
- O responsável pode restringir público, convidados, membros, horário, quota, região e duração; revogação bloqueia novos grants e solicita remoção no roteador.
- Na impossibilidade de revogar credenciais PSK já compartilhadas, informar `revocation_pending_router` até comprovar aplicação/rotação.

## 3. Localização exata dos APs

- Guardar o ponto real em coluna protegida PostGIS com `coordinate_source`, `accuracy_m`, `observed_at`, `verified_at`, `registered_by` e `sharing_authorized_by`.
- O proprietário escolhe `map_precision=exact_opt_in`, `coarse` ou `hidden` por AP. **“Exato” significa melhor coordenada disponível, com incerteza informada**, não garantia de medição de precisão centimétrica.
- O mapa só retorna o ponto preciso se houver autorização **explícita de divulgação**; residências e redes privadas começam como `coarse`/`hidden`.
- A regra de 50 km aplica-se à **localização pessoal no chat**, não a um ponto de acesso voluntariamente publicado; não inferir uma da outra.
- Não divulgar BSSID bruto, endereço de cobrança, log de visita ou identidade do administrador para quem consulta mapa.
- Evitar exposição de residencial por dedução: localização exata é opção destacada com prévia e confirmação específica.

## 4. Registro de visitantes e auditoria

- Os visitantes recebem informação clara sobre quais metadados serão registrados. Administrador do AP acessa apenas metadados operacionais mínimos de **seu próprio AP**, preferencialmente GMC pseudônimo, status, duração e volume quando medido.
- GMI e correlações internas não são mostrados publicamente. Logs técnicos sensíveis são reservados a funções administrativas justificadas.
- Nenhum histórico de URLs, conteúdo de tráfego, conversas, pesquisas, GPS contínuo ou rastreamento Guardian será coletado por padrão para administrar Wi-Fi.
- Definir retenção por categoria, motivo de tratamento, controle de exportação, pedidos de exclusão e auditoria de acesso às próprias auditorias.
- IP/MAC, quando realmente necessários, devem ser tratados como dados sensíveis operacionais de retenção reduzida; endereços MAC aleatórios não devem ser considerados identidade segura.

### Retenção e revogação por classe

A matriz e as regras comuns estão em [Retenção e revogação](retencao-revogacao.md). Aplicam-se separadamente a inventário privado, credencial/grant, sessão, identificador IP/MAC, coordenada exata do AP, projeção pública, auditoria e backup/exportação. A autorização de publicação exata revogada bloqueia imediatamente novas respostas exatas e invalida caches controlados assim que recebem a revogação; a retirada de uma projeção em réplicas é confirmada pela semântica de sincronização comum. Uma senha PSK já entregue não pode ser considerada revogada até que a rotação/ação do controlador seja confirmada, conforme `revocation_pending_router`.

Nenhum TTL está escolhido. Enquanto a política da classe estiver sem jurisdição, gatilho ou duração aprovados, a classe não está pronta para persistência operacional, replicação, exportação ou anúncio de readiness. A perda de contato com o controlador resulta em estado desconhecido, não em revogação confirmada. Auditoria guarda somente metadados necessários à prestação de contas e segue prazo próprio, sem copiar segredos ou localização de visitantes.

## 5. Proteção de credenciais

- Priorizar credenciais individuais revogáveis, redes convidadas, RADIUS/EAP-TLS/Passpoint/PPSK quando existentes.
- Tokens curtos, TLS, armazenamento protegido no aparelho e cofre de segredos/controlador para emissão; **não** armazenar a senha principal da casa em tabelas do mapa.
- Proteções contra replay, enumeração de SSIDs privados, tentativas abusivas de conexão e consulta de sessões de terceiros.
- Auditoria das operações de `share.publish`, `share.revoke`, `grant.issue`, `grant.revoke`, `map_precision.change` e `session.audit.read`.

## 6. Testes obrigatórios

- Rede vista na varredura não aparece no mapa sem autorização do dono.
- Mudar visibilidade para `exact_opt_in` exige prova da autoridade, confirmação e registro de auditoria.
- Um observador público não consegue listar visitantes, GMI, IP, MAC ou segredos de AP.
- Consentimento/revogação propagam-se por células regionais, inclusive quando um nó remoto reconecta.
- Grant vencido não permite emissão de novas credenciais; controlador offline é `unknown`, não `revoked_applied`.

[Registro de acessos](../02-dominio/wifi-registro-acessos.md) · [Modelo Wi-Fi](../02-dominio/wifi-modelo-dados.md) · [Testes](../05-operacao/testes-wifi.md).
