# Privacidade, consentimento e prevenção de rastreamento no GodMesh CrowdFind

> **ADR-044 · diretriz de segurança proposta.** Beacon não é um rastreador secreto. Qualquer compartilhamento é autorizado, visível, com propósito, duração e revogação.

## Permissões independentes
1. `guardian.location` — localização da sessão Guardian, com destinatários e precisão definidos.
2. `guardian.beacon.emit` — permitir publicidade BLE temporária durante sessão ativa.
3. `crowdfind.observe` — permitir que o aparelho reconheça beacons consentidos e publique relatos protegidos.
4. `guardian.invitee.precise` — contato autorizado a ver localização precisa no intervalo da sessão.
5. `incident.location.precise.read` — acesso estritamente vinculado ao incidente SOS, para destinatário competente.

Nenhuma permissão implica automaticamente outra. A interface exibe quando o Beacon está ativo e disponibiliza pausa, revogação e encerramento.

## Minimização e ameaça
- Beacon transmite **somente** token efêmero; não transmite coordenadas, nome, GMC nem telefone.
- O observador não consegue identificar diretamente a pessoa observada a partir do token.
- Tokens são intransferíveis entre sessões, com escopo e prazo curtos; evitar replay, clonagem e rastreamento prolongado.
- Rotatividade e criptografia não são garantias de anonimato perfeito; sinal físico, horário, padrões de movimento e observação repetida ainda podem criar risco de correlação.
- Evitar relatórios da localização precisa dos **observadores** além do necessário; enviar relatos criptografados, reduzir retenção e impedir consultas arbitrárias.
- Não publicar no chat comum localização precisa nem listas de redes Wi-Fi detectadas; usar regra >=50 km quando o usuário optar por compartilhar região.
- Tratamento diferenciado de SOS e Guardian: localização precisa apenas para contatos/atendentes autenticados e escopados.
- O sistema deve permitir desativação e esquecimento conforme políticas e obrigações aplicáveis; cópias offline de destinatários são um limite que precisa ser comunicado.

## Efeito documental da revogação

Aplicar a [proposta de Retenção e revogação](retencao-revogacao.md): cessar novas emissões e consultas a partir da autoridade de revogação, invalidar resolução de tokens, cancelar relatos ainda em filas controladas pelo serviço e impedir que replays ou réplicas offline restabeleçam a autorização. O contrato de sincronização define o evento, cursor e evidência de propagação; este documento não cria uma segunda máquina de estado.

Relatos recebidos antes da revogação continuam cifrados e inacessíveis ao fluxo de localização enquanto uma decisão de descarte não estiver determinada; não se retornam em novas consultas. A proposta recomenda apagar o conteúdo sensível segundo a política aplicável e conservar somente recibo de auditoria minimizado. Se houver retenção legal formalmente autorizada, separar o conteúdo sob acesso bloqueado, registrar escopo/autoridade/prazo da exceção e limitar consultas ao processo autorizado. Não há prazo aprovado nesta versão.

Um dispositivo que esteve offline pode não conhecer a revogação. Até consultar a autoridade atual, não publica relatos pendentes nem resolve tokens com consentimento possivelmente obsoleto. Ao sincronizar, a autoridade rejeita conteúdo fora da janela de autorização; o cliente elimina filas localmente quando receber a revogação. Cópias já exportadas ou abertas pelo destinatário não podem ser apagadas remotamente; informar esse limite antes do consentimento, aplicar expiração local quando suportada e não representar isso como remoção confirmada.

**Casos de conformidade documentais:** (a) token resolvido antes da revogação sob grant válido: acesso é registrado; (b) consulta, replay ou emissão depois da revogação: negar; (c) relato em fila com origem anterior, recebido depois: não publicar sem validação da autorização no instante de origem e da política atual; caso não haja prova confiável, descartar; (d) destinatário desconectado que mantém cópia: marcar como cópia fora do controle remoto, sem alegar exclusão; (e) auditoria preservada: não incluir coordenadas, token reutilizável ou payload do sighting.

## Proteções do produto
- Indicador claro de emissão e participação colaborativa; opt-in próprio para observação.
- Nenhum convite silencioso para rastrear outra pessoa; prova de posse/autorização antes de associar um dispositivo.
- Auditoria de consultas de localização e alertas de uso anômalo; rate limits e bloqueio de acesso abusivo.
- Recursos para detecção de rastreamento indesejado em futuras SmartTags físicas deverão ser projetados segundo padrões vigentes, sem tentativa de mascarar seu funcionamento.
- Não acionar alertas por inferência automática de desaparecimento; atraso de check-in não é SOS.

## Validação
- App desativado/sessão expirada para anúncio e observação; testes de interrupção do SO.
- Sem GMI/GMC/coords em BLE, logs do detector, notificações públicas e mensagens comuns.
- Resolução do token só pela parte com permissão, com negativa após revogação.
- Perfil infantil e regras por organização exigem revisão de proteção de dados antes de implantação.

## Referências internas
- [Guardian](../02-dominio/guardian-acompanhamento.md)
- [Localização](politica-localizacao.md)
- [CrowdFind](../03-integracoes/crowdfind-beacon.md)
