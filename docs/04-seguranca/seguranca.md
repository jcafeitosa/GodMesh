# Segurança, privacidade, isolamento e governança

## Princípios
- **Zero trust entre nós:** descoberta não é confiança; exigir identidade criptográfica e autorização explícita por organização/serviço.
- **Least privilege:** recursos externos só com credenciais e permissões mínimas.
- **Privacy by design:** posição, áudio e metadados são sensíveis; retenção e compartilhamento precisam de finalidade e controles.
- **Separar plano de controle, dados e mídia.**

## Identidades e autorização
**Usuários são identificados pelo GMI (UUIDv7 / URN), não por e-mail, número de telefone, IP ou indicativo.** Endereços `acct:` são aliases descobertos via WebFinger. Uma conta aparece como GodMesh verificada somente após validação criptográfica/autoridade do domínio e credenciais de federação. Ver [Identidade global](../02-dominio/identidade-global.md).

Usuários: OpenID Connect/OAuth2 com sessões de curta validade, MFA para operadores. Nós: pares de chaves persistentes, identificação libp2p e atestação/convites; serviços: tokens/identidades de workload. Dispositivos: enrolamento/associação ao proprietário e autenticação de gateway.

Permissões: RBAC+ABAC por tenant, recurso, papel, objetivo e escopo (`location.read`, `ptt.transmit`, `radio.bridge`, `device.command`). **Nunca** assumir que possuir DevEUI, endereço BLE ou indicativo é prova de propriedade.

## Isolamento de tenants
- `tenant_id` obrigatório em dados operacionais e eventos replicáveis.
- Políticas de acesso no backend e, onde viável, PostgreSQL RLS.
- Chaves e secrets isolados por tenant/connector; criptografia em repouso e em trânsito.
- Consulta cross-tenant apenas com compartilhamento explícito e revogável.

## Localização e prevenção de abuso
- Incluir origem e confiabilidade da posição; permitir ver quem recebe localização e quando.
- Compartilhamento voluntário e revogável; jamais cadastrar tags de terceiros para rastreamento oculto.
- Avisos de localização recente/expirada, retenção mínima, exportação e exclusão conforme aplicabilidade jurídica.
- Logs não devem conter trajetos completos nem conteúdo de voz desnecessariamente.

## Federação
- Manifestos e updates assinados; certificados com validade e revogação.
- ACL por stream, tenant, região e classe de dados.
- Defesa contra peers maliciosos, sybil, replay, floods e anúncios falsos.
- Não aceitar replicação universal sem contrato/escopo. Segregação de domínios e chaves.
- Partição: evitar split-brain para propriedade/ACL por quórum/leader leases com fencing.

## Administração
A interface Ice do Mumble exige controle de rede estrito. FRN mantém credenciais por servidor; gateways são serviços de terceiros. Mapear dependências e supervisão, permitir desligar uma bridge instantaneamente. Backup criptografado, testes de restauração e rotação de segredos são mandatórios antes de produção.

## Incidentes e SOS
O sistema pode encaminhar mensagens e alertas, mas não substitui serviços públicos de emergência nem garante entrega em falhas de rádio/energia. Exibir estados verificáveis (`queued` ≠ `delivered`), escalonamento com consentimento e botão de cancelamento/prevenção de acionamentos acidentais.

## Questões legais futuras
Privacidade e transferência internacional (LGPD/GDPR/leis locais), políticas de dados pessoais, menores de idade, retenção, comunicação eletrônica e regras nacionais para uso de espectro. Exigir revisão jurídica antes do lançamento.

## Fronteiras do GodMesh Native e das bridges
Identidade/ACL/criptografia nativas não devem delegar autorização a Mumble ou FRN; bridges exigem autorização **nas duas pontas**. Uma bridge de transcodificação normalmente termina a proteção de mídia; apresentar aviso explícito aos participantes e não alegar E2EE universal. Chaves de dispositivos e localização são confidenciais por tenant, sem anúncios P2P públicos de coordenadas.

## Transcrição e SOS — proteção adicional
- Transcrição de áudio requer política de canal, aviso/consentimento quando aplicável, minimização, isolamento de modelo e retenção separada de áudio/texto; serviço externo de ASR é opcional e consentido.
- Canal E2EE não entrega áudio claro ao servidor para STT sem consentimento explícito e tratamento correto das garantias criptográficas; bridges não preservam E2EE por padrão.
- SOS compartilha localização, identidade e dados sensíveis apenas com destinatários autorizados; logs de acesso, revogação, modo teste, prevenção de abuso e confirmação humana verificável.
- SOS via GodMesh não equivale a atendimento público de emergência; disponibilizar caminho claro para discagem local de emergência com ação do usuário.

## Localização em dois níveis — ADR-041

O chat normal (incluindo PTT, mensagens e transcrição) **nunca divulga posição GPS precisa**; somente região ampla autorizada, com incerteza nominal **mínima de 50 km de raio**. O incidente SOS explicitamente acionado pode divulgar a localização **mais precisa disponível** exclusivamente a responsáveis autenticados que possuam `incident.location.precise.read`, com acesso temporário e auditado. Ver [Política de localização](politica-localizacao.md). A precisão aproximada não garante anonimato absoluto.

## Retenção, revogação e cópias

Finalidade, autorização, retenção e destino de cada cópia são controles distintos. O contrato proposto em [Retenção e revogação](retencao-revogacao.md) descreve a matriz por classe/cópia, bloqueio após revogação e tratamento de réplicas, filas, backups e exportações. Nenhum prazo ou jurisdição foi aprovado: políticas sem parâmetros completos não podem ser declaradas efetivas nem justificar retenção indefinida. Uma revogação não apaga silenciosamente trilha mínima de auditoria, e retenção legal exige autoridade, escopo e revisão definidos; isso não autoriza manter o conteúdo sensível indiscriminadamente.
