<div align="center">
  <img src="assets/godmesh-cover.svg" alt="GodMesh — comunicação nativa e conectividade federada" width="100%" />

  <h1>GodMesh</h1>
  <p><strong>Comunicação nativa. Conectividade federada.</strong></p>
  <p>Especificação de uma infraestrutura integral para pessoas, equipes, ativos e dispositivos.</p>
  <p>
    <a href="docs/README.md">Documentação</a> ·
    <a href="docs/00-produto/visao.md">Visão do produto</a> ·
    <a href="docs/01-arquitetura/arquitetura-referencia.md">Arquitetura</a> ·
    <a href="docs/00-produto/requisitos.md">Requisitos</a>
  </p>
</div>

---

## Estado do projeto

GodMesh está em fase de especificação e planejamento. Este repositório publica documentação do produto e contratos propostos; **não contém uma implementação completa**, e as propostas aqui descritas não comprovam validação em produção. Veja a [documentação](docs/README.md) antes de interpretar recomendações como compromissos implementados.

O objetivo é desenvolver um sistema integral, sem reduzir o escopo final a um MVP: protocolo de aplicação e voz próprios sobre padrões auditáveis, federação distribuída e integrações externas isoladas por bridges/adaptadores opcionais.

## Produtos planejados

- **GodMesh Personal** — comunicação, dispositivos próprios e compartilhamento consentido.
- **GodMesh Teams** — organizações, equipes, ativos, despacho, auditoria e políticas.
- **GodMesh Node** — serviços distribuídos e capacidades regionais/edge.
- **GodMesh Platform & SDK** — APIs, SDKs, adaptadores e automações.

## Princípios arquiteturais

- Núcleo GodMesh nativo; serviços externos são opcionais e não são dependências do caminho GodMesh↔GodMesh.
- GMP e GodMesh Voice definem semântica própria de aplicação; não se propõe reinventar criptografia, codecs ou transportes auditados.
- Federação e replicação respeitam autoridade, finalidade, consentimento, tenant e residência de dados.
- Estado offline, entrega, confirmação técnica e reconhecimento humano são distintos e apresentados honestamente.
- Localização e recursos de emergência seguem escopo autorizado e não prometem entrega universal.

## Estrutura

| Caminho | Conteúdo |
|---|---|
| [`docs/00-produto/`](docs/00-produto/) | Visão integral, glossário e requisitos |
| [`docs/01-arquitetura/`](docs/01-arquitetura/) | Arquitetura distribuída, protocolo GMP, voz, federação e clientes |
| [`docs/02-dominio/`](docs/02-dominio/) | Identidades, ativos, eventos, SOS, localização e Wi-Fi |
| [`docs/03-integracoes/`](docs/03-integracoes/) | Bridges, gateways, rádio e adaptadores externos |
| [`docs/04-seguranca/`](docs/04-seguranca/) | Segurança, privacidade, consentimento e retenção |
| [`docs/05-operacao/`](docs/05-operacao/) | Implantação proposta, observabilidade e validação planejada |
| [`docs/07-contratos/`](docs/07-contratos/) | Contratos de API, GMP e sincronização propostos |
| [`AGENTS.md`](AGENTS.md) | Orientações de contribuição e trabalho assistido por agentes |

O índice completo e as regras de precedência estão em [`docs/README.md`](docs/README.md).

## Primeiros passos

Este snapshot é documental; não há aplicação executável ou comando de build/teste anunciado neste repositório. Para conhecer o produto, comece pela [visão integral](docs/00-produto/visao.md), depois leia a [arquitetura de referência](docs/01-arquitetura/arquitetura-referencia.md) e os [requisitos](docs/00-produto/requisitos.md).

## Contribuições

Contribuições para a documentação devem indicar a fonte, distinguir decisões aprovadas de propostas e preservar os limites de segurança e estado real. Leia [`AGENTS.md`](AGENTS.md) e as regras em [`docs/README.md`](docs/README.md). Mudanças de código só devem começar após os gates de planejamento e SPECs aprovadas definidos pelo projeto.

## Licença

O conteúdo publicado sob este repositório é disponibilizado sob a [licença MIT](LICENSE). A licença do repositório não resolve automaticamente licenciamento de futuros serviços, marcas, conteúdos de terceiros ou dependências externas.
