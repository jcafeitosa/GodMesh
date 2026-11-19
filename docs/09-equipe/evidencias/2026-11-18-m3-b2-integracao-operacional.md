# M3-B2 — integração operacional do piso de configuração

A resposta do usuário para continuar após a solicitação de autorização libera a [SPEC M3-B2 v1.2](../../08-specs/federacao/m3-b2-integracao-operacional.md), o adaptador `tokio-rustls` e o nível 1 para `POST /v1/events`. O texto recebeu parecer independente favorável no SHA-256 `475b476ee0eb7566372ba7bd843910530a0b34202f2083e0f137a8ae5b0a4711`. A base `f646842` inclui o [aceite M3-B1](2026-11-18-m3-b1-servico-duravel.md). M4 permanece condicionada ao aceite operacional.

1. **Transporte.** Reutilizar a verificação de chaves Ed25519 do QUIC em TLS 1.3 dedicado, com ALPN HTTP/1.1, pin no cliente e verificação mTLS no servidor. Testar ausência de certificado, pin divergente, saturação e prazos de handshake/cabeçalhos.
2. **API e pull.** Ligar publicação, seleção condicional e resolução à transação B1. Revalidar a identidade autenticada na aceitação do pull. Testar status, ETag, limites, perda de resposta, revogação concorrente e orçamento de tentativas.
3. **Runtime e aplicação.** Validar a política antes dos listeners. Ativar no relógio da B1 e aplicar o piso transacional no QUIC e ingest sob o guard M1C. Testar mudança em sessão aberta, vencimento e recuperação por HTTPS.
4. **Aceite.** Executar testes de regressão e revisão independente. Subir três processos com chaves e bancos próprios e verificar no fio a ativação de piso 2, o bloqueio do QUIC e do ingest e a recuperação por um bundle superior.

A documentação oficial registra `tokio-rustls` 0.26.6 estável, MIT ou Apache-2.0 e Rust mínimo 1.71. Os requisitos Tokio 1 e Rustls 0.23.27 ou superior na série 0.23 são compatíveis com 1.54.1 e 0.23.46 já resolvidos. O adaptador liga o I/O assíncrono ao Rustls existente. Desabilitar features padrão e manter Ring. A implementação deve conferir a resolução e a árvore de features. Consulta de 18/11/2026. [Manifesto publicado](https://docs.rs/crate/tokio-rustls/0.26.6/source/Cargo.toml) · [Avisos RustSec](https://rustsec.org/packages/tokio-rustls.html)

Status no registro da aprovação. Testes de implementação pendentes.
