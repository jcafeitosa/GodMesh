# Registro de aprovação — gate técnico de 17/11/2026

> **O que este documento registra:** decisão do CTO sobre o gate técnico de cada SPEC, com parecer de crítico independente sobre o **hash da versão corrente**. Não registra implementação, teste executado nem homologação.
> **O que ele não registra:** parecer do revisor nominado (Bruce Schneier, Erich Gamma, Vint Cerf, Ada Lovelace, Edsger Dijkstra, James Bach, Leslie Lamport, Donny Wals, Michael Stonebraker) sobre esta versão — isso continua pendente e é pré-condição de aprovação formal de produto, não de início de implementação sob SDD+TDD.

## Base da decisão

- 4 rodadas de crítica independente: C3 (testabilidade), C4 (adversarial), gate v2, gate v3 e gate v4 sobre as 8 SPECs corrigidas.
- Correções aplicadas: 113 gaps classificados, 15 defeitos documentais, ADR-056…068, 24 SPECs com casos por decisão e por requisito.
- Todo caso permanece `NOT_EXECUTED`; todo ID formal de teste permanece `NOT_ASSIGNED`.

## Decisão

Aprovo as 24 SPECs **para início de implementação sob SDD+TDD**, com as ressalvas abaixo. A aprovação é do gate técnico; o parecer do revisor nominado por versão segue como condição para declarar a SPEC formalmente aprovada em produto.

| SPEC | Versão | Hash aprovado | Decisão | Ressalva |
|---|---|---|---|---|
| 001 | DRAFT-0.8 | `101dc00e5689ae55d64eb95a683d326e06add54f04dde896969240e433c6e8d0` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 002 | DRAFT-0.9 | `98ce3a947b44b2a103e8498105c752ffc1b41373e68cd078591c4a562b5e369d` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 003 | DRAFT-0.5 | `a5e85551ff71f0da38ccfaa6e20a4f8ab30952a7aec18d30703501ffd9133c15` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 004 | DRAFT-0.6 | `ce350188aaf38da269804782b617eefc9b6398912fdcd8f1c14a1eb62d128b6f` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 005 | DRAFT-0.9 | `888b02621693e3759746e73638e8296df36fb781dbf05ac288d5739485dd5b3c` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 006 | DRAFT-0.10 | `8a6703aae83cad826e8a175dd88349ec1e30aacd0d296ac8f01656013a1073b5` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 007 | DRAFT-0.8 | `c713fa4ad8a9580c0224658da0490706b3dcad0e103ba02442920ae490ad614e` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 008 | DRAFT-0.8 | `2279bbbc7bd9843b5206203ee36cb312f9667f50ca9d430bd0708fa3847868a2` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 009 | DRAFT-0.9 | `162add243cbd5ef779dc4d4672a611f0c4dbfde568a547104ce1f2b05ccf19b3` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 010 | DRAFT-0.7 | `2e38e9aa61adb01cd3e9d7d755359cce70832acb98653d9df3072674fbf5d873` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 011 | DRAFT-0.10 | `c7684dfffb7d09a8fea4cdcd72a773bbe6f6b6bbb6e73b456ea1c46c093b8d8f` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 012 | DRAFT-0.12 | `6d4b4584fcd17f40432b5a1ec9898930c1972d767a40bef4392d1058ae5bf1ea` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 013 | DRAFT-0.7 | `2a0af707d04f40950be96e694da72f79efa23b71a7ac8f3d26e75685b6418478` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 014 | DRAFT-0.9 | `d37160a32ddd04d74417b4d15000bb2dc0b3acff2bc1acbc33dffa1efe30fbbb` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 015 | DRAFT-0.7 | `18c1ab4a8be8a8669129e04663c73039be66118bb54ff7bb046d518ec7619cea` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 016 | DRAFT-0.9 | `0378d4b790c465676875055221b75cea01c6d8f21b4b9dc31a8da4eac4b341b1` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 017 | DRAFT-0.5 | `982d5096af36ee2e73c0bf87871fbc1d17724666b9e746f3ba33b6efe9336658` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 018 | DRAFT-0.6 | `be889e2a5405d270a28aa5b19123141b974b76511a80f87528c6e0f783e92b33` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 019 | DRAFT-0.11 | `617d048197e0cc7b5a1ced8aaf9e688703c1a4cc651ad3c9743934277edb87b5` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 020 | DRAFT-0.5 | `6e90fe613044c61fdc35a910148117d5542446f74b90cb41163a9289ce900180` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 021 | DRAFT-0.7 | `8d4b173bdb473acb98198570031160f4f96625ba8bef25ac9258e24b59c4d7ce` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 022 | DRAFT-0.6 | `510961f2978f3ab7c563260f18da114affb305ce130e0665e2b88eb7f00b2713` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 023 | DRAFT-0.8 | `deedcbeea57272be8eb3254e426e8068fe9103267a219e1b2b6a83560d6159c0` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |
| 024 | DRAFT-0.6 | `c49f6a830211898f562228113417cb6f670b5b34acc0b672b2aa38f67be73cfa` | APROVADA no gate técnico | parecer do revisor nominado para esta versão: pendente |

## Ressalvas que acompanham a aprovação

1. **Parâmetros não medidos**: janela N/N-1, limites de frescor, RPO/RTO, tetos de capacidade e custos seguem desconhecidos; nenhum número foi inventado.
2. **Jurídico**: retenção (ADR-065), residência BR/CO (ADR-066) e obrigações de RF/emergência exigem confirmação por jurisdição antes de operação comercial.
3. **Hardware**: F1/F2 (contador protegido, tempo confiável) não foram verificados em nenhuma plataforma; até lá, SOS offline é negado por desenho (ADR-064).
4. **Tauri mobile**: plugins nativos são requisito de entrega de PTT em segundo plano, BLE, localização e push; sem eles, RF-005/RF-028/RF-030/RF-042 não são entregáveis.
5. **Implementação**: começa pelo Programa A (protocolo, identidade, sync) com teste que falha antes do código; nada de tela antes de contrato.

## Verificação desta aprovação

- `shasum -a 256 docs/08-specs/*.md` deve bater com a coluna de hash acima; divergência invalida a aprovação para aquela SPEC.
- Qualquer alteração posterior exige nova versão, novo hash e nova revisão; aprovação não se herda entre versões.
