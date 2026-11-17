# Mumble e Free Radio Network — papel oficial de interoperabilidade

> **Histórico substituído:** a hipótese anterior de usar Mumble como motor principal foi **superada** pela arquitetura GodMesh Native. Este documento agora descreve somente integrações externas opcionais.

## Regra obrigatória

Mumble e FRN são dois **primeiros destinos de interoperabilidade**, não componentes necessários à instalação, inicialização, expansão regional ou funcionamento cotidiano do GodMesh. O cliente GodMesh não precisa instalar Mumble/FRN nem possuir conta nessas redes para utilizar comunicação nativa.

GodMesh ↔ GodMesh usa o **GMP e GodMesh Voice nativos**. Mumble e FRN são sistemas terceiros; sua indisponibilidade, licenciamento ou mudanças não podem interromper canais nativos.

## Mumble Bridge
- Cliente Mumble compatível com sinalização e mídia; **Ice é administração**, não canal de áudio.
- Relacionar usuários e canais explicitamente, verificar ACL e autorização dos dois sistemas.
- Credenciais isoladas e canal autorizado, nunca conectar a qualquer servidor encontrado sem permissão.
- Manter ponte *off* por padrão; impedir loops e separação de identidades.

## FRN Bridge
- Cliente compatível com protocolo FRN documentado e versão efetivamente suportada pelo servidor de destino.
- Autenticação/credenciais e autorização do operador para receber ou transmitir.
- Modo RX para testes; habilitação TX somente quando permitido, com lease exclusivo e timeouts.
- Documentação FRN é antiga; validar formato/qualidade de áudio e manutenção contra servidores reais.

## Descoberta de servidores externos
Catálogo curado, DNS/diretórios quando publicados e LAN com consentimento. Encontrar um serviço não permite utilizá-lo. O GodMesh regional seleciona conector responsável e lease, evitando duplicar sessões em rede externa.

## Segurança e legalidade
Proibir por padrão ponte entre canais privados/empresariais e frequências de radioamador. Validar regras da jurisdição e termos de redes externas. Bridges que decodificam áudio não preservam E2EE em toda a rota.

## Fonte de verdade
Consulte [Voice Bridge SDK](bridges-voz.md), [Mídia nativa](../01-arquitetura/midia-nativa.md) e [Uso das redes](../04-seguranca/uso-das-redes.md).
