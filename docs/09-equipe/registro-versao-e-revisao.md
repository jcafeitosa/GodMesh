# Registro de versão e revisão — SPECs e ADRs

**Worker:** W2 — Normativo e ADRs (thread `thr_6fji6bxva8`, execução interrompida por truncamento; registro produzido pelo CTO em 17/11/2026 com verificação mecânica). **Método:** extração por regex dos cabeçalhos, `shasum -a 256` dos arquivos atuais e confronto com o snapshot publicado em `SPECS.md`. **Nada aqui aprova SPEC, decisão ou implementação.**

## 1. SPECs — versão declarada, autoria, revisão e hash atual

Tabela regenerada em 17/11/2026 para 24 arquivos (SPEC-001…024). Hash identifica conteúdo, não revisão.

| Arquivo | Versão | Data | Autor | Revisor designado | SHA-256 atual |
|---|---|---|---|---|---|
| SPEC-001-arquitetura-federacao.md | DRAFT-0.5 | 2026-11-17 | Martin Fowler | n/d | `c2ea11384d07a540364553cb0472d02e23086aeb1474ee8fed2386b1a41a7e7e` |
| SPEC-002-identidade-autorizacao.md | DRAFT-0.6 | 2026-11-17 | Kim Cameron (pod P2 — Identidade) | Bruce Schneier. Revisão da contribuição anterior não equ | `db059aa886f945b3e0a772b3071407d9cb9a95653a1633857eb759bc12beb6ca` |
| SPEC-003-gmp-api.md | DRAFT-0.4 | 2026-11-17 | Graydon Hoare | Erich Gamma, revisão de DRAFT-0.4 pendente | `e99c9bffe049d2ded19b2d36b40d3666ddf2d5d660c6dea81dbc5132ca7860a9` |
| SPEC-004-eventos-sincronizacao.md | DRAFT-0.5 | 2026-11-17 | Martin Kleppmann | Leslie Lamport, revisão desta versão pendente. | `a860892cdb6e98852760b323841fc676f9ac7b5d7bef9e029e0ba75287d9afcd` |
| SPEC-005-dados-persistencia.md | DRAFT-0.5 | 2026-11-17 | Michael Stonebraker. | Bruce Momjian | `6bbe8eb40ce43ec2583cfaeb87644d16474ee7287e8dfbea92f0efa1a72edb31` |
| SPEC-006-sos-assistencia.md | DRAFT-0.6 | 2026-11-17 | Nancy Leveson | Edsger Dijkstra | `ae3837d55b34b0324038e0bf9f4f813536e980a3c28bc0a056b0a64da1083ee7` |
| SPEC-007-voz-ptt-midia.md | DRAFT-0.4 | 2026-11-17 | Radia Perlman | Vint Cerf | `135fc321e18b926a6cae835da7fb80a19f5e98afcf7e77622486fa8668c61c95` |
| SPEC-008-transcricao-asr.md | DRAFT-0.3 | 2026-11-17 | Daniel Povey | Bruce Schneier | `df7344da2bb9c6ef3b91148244040c663719c6f41ce8c28ec04bfbf694df7ede` |
| SPEC-009-wifi.md | DRAFT-0.3 | 2026-11-17 | Jouni Malinen | Richard Bejtlich | `11e7c6ba64c351e6ee92f59e538d3f3f4926b9c92c57d5aba1f1dd34fb3fe043` |
| SPEC-010-integracoes-bridges.md | DRAFT-0.4 | 2026-11-17 | Gregor Hohpe | Erich Gamma (revisão pendente desta versão/hash). | `3a127dd2de567343c5ca76d092c2c6fedb88310f735f51ecb7f0f33445aae2a5` |
| SPEC-011-localizacao-geoespacial.md | DRAFT-0.3 | 2026-11-17 | Paul Ramsey | Bruce Schneier. | `69e6448b20f904fa3e501a474ed4db678ae89d2e5c79ce6fd036e657ce7503c4` |
| SPEC-012-produto-guardian.md | DRAFT-0.7 | 2026-11-17 | Margaret Hamilton | Ada Lovelace, revisão do delta desta versão solicitada | `18115111fd97b4e3db787d37de008f4e62166478548c0a933d475bac1d6f413d` |
| SPEC-013-experiencia-plataformas.md | DRAFT-0.3 | 2026-11-17 | Lea Verou | James Bach (pendente desta versão/hash) | `b5cfe065ad48cb09534c291c7e772b20a1b450f2fd3df0d48ae9c38b5e7059f8` |
| SPEC-014-cliente-offline-ffi.md | DRAFT-0.3 | 2026-11-17 | Alex Crichton | Donny Wals | `afad20ae2b86928e0b9a8eae3baf4059ebe474f0b03433d2b678102c4e7f57b6` |
| SPEC-015-android-kotlin.md | DRAFT-0.3 | 2026-11-17 | Chet Haase. | Alex Crichton, fronteira Android/offline/FFI. | `f1e7c5032d2bdd2471a3f127a81dc91bca53bfd31165da0a3f454bf66f68241b` |
| SPEC-016-ios-swift.md | DRAFT-0.5 | 2026-11-17 | Donny Wals. | Alex Crichton, fronteira Rust–Swift e cliente offline. | `cb8e4e66e70ac51fe03a56a108aae3febbdd30ae67f5f4d27b4e4cc30c44c1c3` |
| SPEC-017-dispositivos-firmware.md | DRAFT-0.3 | 2026-11-17 | Jack Ganssle | Vint Cerf | `7eeab4e384248a58411cf5c97f5271baa92a44fd8b0fa9a7cbd77f896dda4a7e` |
| SPEC-018-operacao-observabilidade.md | DRAFT-0.4 | 2026-11-17 | Brendan Gregg • | James Bach revisou DRAFT-0.1 (versão anterior) | `67d5e3edc79438898efd037fb7ab7993fa5bb1a94b7351998f9dab59fa717605` |
| SPEC-019-implantacao-capacidade.md | DRAFT-0.5 | 2026-11-17 | Grady Booch. | Leslie Lamport revisou DRAFT-0.2 (versão anterior) | `6fee450ec96a0634e4b23a7e03934ad52685c410a0db3ef4a530d5d07272a50d` |
| SPEC-020-seguranca-retencao.md | DRAFT-0.3 | 2026-11-17 | Bruce Schneier | Dan Kaminsky e Raphael Mudge | `69da2fa2165750f9c5c9320f4f1ba31999ec3e1567d980ebf42e8f6d9f37e0c8` |
| SPEC-021-postgresql-recuperacao.md | DRAFT-0.3 | 2026-11-17 | Bruce Momjian. | Michael Stonebraker | `036303c01225cfeac4dadda2b226c009978c2f9aee04861277839b88414e17b5` |
| SPEC-022-produto-organizacoes.md | DRAFT-0.2 | 2026-11-17 | Margaret Hamilton (pod P5 — Emergência, produt | Ada Lovelace — revisão desta versão | `d6ec72e3e3a634a762daec26fee58775434d6f27458da155c58f842587fe6053` |
| SPEC-023-capacidades-conectividade.md | DRAFT-0.2 | 2026-11-17 | Gregor Hohpe | Vint Cerf — pendente | `0f4d23edb0023f07cd7089c59797b2c40d8a1224dd212362cc048ccbad1f9a6a` |
| SPEC-024-crowdfind-beacon.md | DRAFT-0.2 | 2026-11-17 | Jack Ganssle | Bruce Schneier, pendente. | `940c2d1f9c73e27b072dd1609eff265875d30dc151c322cd02c978a9315ebadf` |

## 2. ADRs — status declarado e cobertura por SPEC

Extraído de `docs/06-planejamento/decisoes.md`. Coluna "SPEC que cita" lista menções textuais `ADR-NNN` nos arquivos de `docs/08-specs/`.

| ADR | Status declarado | SPEC que cita |
|---|---|---|
| ADR-001…ADR-003 | Aprovada | nenhuma |
| ADR-004 | Aprovada | SPEC-012, SPEC-019 |
| ADR-005 | Aprovada | nenhuma |
| ADR-006 | Aprovada | SPEC-001 |
| ADR-007 | Em validação | nenhuma |
| ADR-008…ADR-013 | Aprovada | nenhuma |
| ADR-014 | Aprovada | SPEC-007 |
| ADR-015 | SUPERADA por ADR-021/022 | nenhuma |
| ADR-016…ADR-026 | Aprovada | nenhuma |
| ADR-027 | Aprovada | SPEC-018 |
| ADR-028…ADR-033 | Aprovada | nenhuma |
| ADR-034…ADR-036 | Aprovada (035/036 como direção) | SPEC-002 |
| ADR-037 | Aprovada como direção | SPEC-010 |
| ADR-038 | Aprovada como direção | SPEC-008 |
| ADR-039 | Aprovada como direção | SPEC-006 |
| ADR-040 | Aprovada como direção | SPEC-008 |
| ADR-041 | Aprovada como direção | SPEC-008, SPEC-010, SPEC-016 |
| ADR-042 | Aprovada como arquitetura | SPEC-016, SPEC-018 |
| ADR-043, ADR-044 | Aprovada como arquitetura | nenhuma |
| ADR-045 | Aprovada como arquitetura | SPEC-016 |
| ADR-046 | Aprovada como regra de produto | nenhuma |
| ADR-047 | Aprovada; implementação pendente | SPEC-013, SPEC-014, SPEC-015, SPEC-016 |
| ADR-048…ADR-051 | Aprovada | nenhuma |
| ADR-052 | Aprovada | SPEC-013, SPEC-016 |
| ADR-053 | Aprovada | SPEC-007, SPEC-008, SPEC-013, SPEC-014 |
| ADR-054 | Aprovada | SPEC-011 |
| ADR-055 | Plataformas aprovadas; recomendações propostas | SPEC-011, SPEC-012, SPEC-013, SPEC-014, SPEC-015, SPEC-016, SPEC-019 |

**Achado verificável:** 54 ADRs registradas; apenas 20 são citadas textualmente por alguma SPEC. ADR-043 (CrowdFind/Beacon) e ADR-044 (privacidade CrowdFind) não são citadas por SPEC alguma, embora RF-030 dependa delas — coerente com a ausência de SPEC canônica de CrowdFind apontada por W1/C1. ADR-007 ("QUIC/libp2p como base candidata — validar") permanece "Em validação" e não é citada por SPEC-001, que trata federação.

## 3. Inconsistências documentais verificáveis

1. **Snapshot de hashes obsoleto** em `SPECS.md` para 15 de 21 SPECs.
2. **Versão/data ausentes no cabeçalho** em SPEC-002, SPEC-006, SPEC-010, SPEC-015, SPEC-021 (data) e SPEC-006 (formato de versão).
3. **Revisor independente não declarado** em SPEC-014.
4. **Revisões vinculadas a versões anteriores**: SPEC-007 (Vint revisou DRAFT-0.1), SPEC-018 (James revisou DRAFT-0.1), SPEC-019 (Leslie revisou DRAFT-0.2), SPEC-012 (Ada revisou DRAFT-0.3/0.5 conforme texto). Nenhuma é revisão da versão atual por hash.
5. **ADR sem SPEC citante** em 34 casos, incluindo ADR-007 (federação), ADR-043/044 (CrowdFind), ADR-046 e ADR-048–051 (Wi-Fi).
6. **`SPECS.md` declara "A conciliar" para todos os 60 requisitos**, enquanto `docs/09-equipe/cobertura-requisitos.md` já mapeia cada um com seção e aceite. Os dois documentos precisam ser conciliados; a matriz nova não substitui a tabela canônica.

## 4. Não verificado

- Se as alterações que produziram os hashes novos foram revisadas por alguém, ou quando ocorreram (o Git não tem commits; não há histórico de versões neste repositório).
- Conteúdo das contribuições externas citadas nas SPECs (`Shared/GodMesh/...`), indisponíveis nesta verificação.
- Correspondência entre cada ADR "aprovada" e evidência de aprovação do usuário; este registro apenas repete o status declarado no arquivo.
- Qualquer implementação ou execução de teste: inexistente no repositório.