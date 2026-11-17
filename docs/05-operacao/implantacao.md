# Instalação Linux e implantação — especificação integral (não é instalador executável)

## Alvo de distribuição aprovado

O servidor Rust será distribuído em contêiner Linux, conforme ADR-055. Imagem base, arquiteturas, privilégios, volumes, segredos, limites e ciclo de atualização ainda precisam ser especificados. O monólito modular e os processos isolados preservam suas fronteiras; não se presume um contêiner único para banco, mídia, bridges e aplicação.

Durante o desenvolvimento, PostgreSQL, NATS e demais dependências ficam no OrbStack; backend, frontend e apps executam localmente, conforme instrução do usuário. Esta documentação não comprova configuração ou execução.

## Premissas
Cada full node recebe a mesma distribuição de software, mas dependências e hardware podem variar. Kubernetes/K3s ou Docker Compose podem ser usados conforme a escala. Não pressupor que Docker Compose em 1 VPS garante alta disponibilidade.

## Topologia de laboratório
- 3 máquinas/VMs Linux em mesma região (2 ou 3 com IPv6 público, todas com IPv4 onde disponível).
- 1 a 3 bancos PostgreSQL de teste com PostGIS (estratégia de replicação documentada).
- NATS JetStream e logs de eventos persistentes.
- GodMesh Voice/Media nativo em cada célula regional; Mumble é somente um servidor de interoperabilidade opcional para testes de bridge.
- 1 Bridge FRN Connector com ambiente de testes autorizado.
- ChirpStack/MQTT e gateway LoRaWAN opcionais à fase de integração.

## Bootstrap (desenho de CLI, **ainda não implementada**)
```bash
godmesh-node init --role full-node
godmesh-node join --invite '<TOKEN>'
godmesh-node start
godmesh-node status
godmesh-node peers
godmesh-node services
```

## Arquivo conceitual
```yaml
node:
  role: full-node
  generate_identity_on_first_boot: true
  enrollment_requires_authorization: true
network:
  preferred_family: ipv6
  ipv4_fallback: true
  transport: quic
  tcp_tls_fallback: true
  discovery: [mdns, bootstrap_dns, kad]
replication:
  enabled: true
  policy: tenant_scoped
  snapshot_then_events: true
services:
  api: true
  registry: true
  location: true
  messaging: true
  voice_native: true
  media_edge: true
  radio_connectors: true
security:
  signed_manifests: true
  require_trusted_peers: true
```

## Passos de instalação esperados
1. Testar requisitos: CPU, RAM, disco, IP disponível, DNS, clock, permissões e portas.
2. Instalar binário assinado e serviços/dependências necessárias.
3. Gerar chaves e armazená-las em keystore protegido; não em repositório Git.
4. Obter manifesto da rede confiável e ingressar com token autorizado.
5. Subir banco/filas/serviços locais e garantir healthchecks.
6. Receber snapshot autorizado, aplicar eventos, verificar checksums/checkpoints.
7. Expor endpoint regional após `readiness == true`.
8. Publicar métricas e política de capacidade no balancer.

## Atualizações
Rolling updates com canary regional e migração compatível; em caso de incompatibilidade, nó não participa de replicação. Guardar última versão assinada e capacidade de rollback, sem reintroduzir esquema de banco incompatível.

### Registro documental por release (proposta)

Para cada release futura, manter uma matriz de compatibilidade cliente/servidor/schema por plataforma e arquitetura, identificador da revisão, versão resolvida do toolchain/dependências e digest de imagem quando aplicável, origem e evidência de assinatura. Registrar pré-condições, ordem de atualização, saúde/readiness observadas por serviço/rota, migração expansiva/compatível e janela em que versões anteriores continuam suportadas. O plano inclui condição aprovada de interrupção do canary, artefato anterior recuperável, procedimento de rollback para nós online e offline e tratamento do estado de dados após migração; migração destrutiva ou downgrade de schema sem rollback compatível bloqueia a promoção até decisão aprovada. Isto é um gate documental proposto, não pipeline ou release existente.

O desenvolvimento local continua conforme decisão do usuário: dependências justificadas em OrbStack e backend/frontend/apps no macOS. Check de infraestrutura/dependency smoke não prova build, teste ou readiness do GodMesh. O alvo servidor em contêiner Linux está aprovado como direção, mas imagem base, arquiteturas, ferramentas, runner e matrix de CI/release não estão selecionados. Nenhuma escolha de ferramenta, prazo de suporte, threshold, SLO ou estratégia específica de migração é aprovada por este texto.

## Backups
Regras de retenção por tenant, backups criptografados e restauração periódica de dados e chaves; nunca confundir réplica quente com backup contra corrupção ou exclusão maliciosa.
