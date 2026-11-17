# Plano de testes e critérios de aceitação

## Laboratório de validação da arquitetura integral
3 nós Linux, dual-stack quando possível, 2 ou mais instâncias completas na mesma região; clientes simulados e reais. FRN em ambiente de testes autorizado; Mumble próprio.

## Rede
- [ ] Persistência do `peer_id` após reinicialização e troca de IP.
- [ ] Conexão preferencial IPv6 quando alcançável.
- [ ] Fallback IPv4 com IPv6 indisponível e sem downgrade de segurança.
- [ ] Conexão via relay autorizado quando peers não têm caminho direto.
- [ ] Descoberta LAN e WAN após perda de peers de bootstrap conhecidos.
- [ ] Negociação de versões incompatíveis falha de forma segura.

## Federação/replicação
- [ ] Nó novo só entra após autorização.
- [ ] Snapshot é completo no escopo autorizado e verificado.
- [ ] Queda de nó não perde eventos confirmados localmente.
- [ ] Reconexão recupera backlog com checkpoints.
- [ ] Eventos duplicados não geram registros duplicados.
- [ ] Exclusões, revogações e regras de retenção têm comportamento testado.
- [ ] Partição sem quórum bloqueia apenas operações que o exigem.

## Capacidade
- [ ] Várias instâncias da mesma região recebem novas conexões.
- [ ] Um nó saturado é removido/reduzido da escolha após histerese.
- [ ] O serviço não entra em oscilação quando a carga muda rapidamente.
- [ ] Uma nova máquina entra no pool somente após readiness.
- [ ] Falha regional move novas sessões para outro destino permitido.

## Convergência
- [ ] Smartphone tem voz, mensagens e localização compatíveis.
- [ ] Tag BLE de teste não recebe interface de voz fictícia.
- [ ] Rastreador LoRaWAN publica observação via ChirpStack.
- [ ] Downlink publicado não aparece como entregue sem confirmação apropriada.
- [ ] GodMesh Native PTT funciona com grupos/ACL, cliente web e cliente nativo sem servidor Mumble.
- [ ] Mumble Bridge conecta a servidor autorizado e pode ser desligada sem afetar GodMesh Voice.
- [ ] FRN conecta e recebe de servidor autorizado; bridge não entra em loop.
- [ ] Apenas uma instância controla TX externo e lease revogado impede TX tardio.

## Privacidade
- [ ] Nenhuma observação de um tenant aparece para usuário não autorizado.
- [ ] Revogar compartilhamento impede novas leituras e propaga políticas.
- [ ] Logs não armazenam tokens/segredos nem localização além do necessário.
- [ ] Rede de radioamador não recebe tráfego empresarial não autorizado.

## Critérios de aceitação do sistema integral
O sistema completo exige validações independentes de identidade, protocolo GMP, voz nativa, federação, balanceamento, replicação, localização, universal gateway, bridges, dispositivos, segurança, privacidade e operação global. Cada domínio só poderá ser declarado concluído após testes reproduzíveis e observabilidade; testes parciais não equivalem à conclusão da plataforma.

## Transcrição multimodal e SOS
- Ao pressionar PTT, cada fala autorizada recebe `talk_session_id` e transcrição parcial/final com autor/GMC ou fonte externa escopada; silêncio/ruído/voz não identificável não são atribuídos arbitrariamente.
- Falha/saturação do ASR não interrompe áudio; canais sem consentimento e canais E2EE incompatíveis não capturam mídia.
- Para SOS: testar ACK local, replicação, gateway, endpoint e reconhecimento humano como evidências separadas; nenhum evento vira `claimed` automaticamente.
- Testar modo simulado, falha de região, LoRaWAN Classe A, gateway offline, localização desatualizada, escalonamento, cancelamento e bridge externa não autorizada.

## Testes dedicados de mobile, Guardian e satélite
A matriz específica de plataforma, consentimento, BLE e contingência está em [Testes móveis, Guardian, CrowdFind e satélite](testes-mobile-guardian.md). A plataforma não pode alegar cobertura satelital, transcrição ou funcionamento de SOS sem ensaios controlados e aprovação das integrações reais.
