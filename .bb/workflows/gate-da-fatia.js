export const meta = {
  name: "gate-da-fatia",
  description: "Revisao de contrato, ataque adversarial e verificacao por fatia implementada",
  phases: [
    { title: "Revisao", detail: "Leitura do contrato e da implementacao da fatia" },
    { title: "Ataque", detail: "Tentativa de violar o invariante da fatia" },
    { title: "Verificacao", detail: "Ceticos tentam refutar cada bloqueio alegado" },
    { title: "Sintese", detail: "Veredito final por fatia" },
  ],
};

const ROOT = "raiz do checkout GodMesh";
const MAX_VERIFICACOES = 2;

const fatiasDefault = [
  {
    key: "persistencia-incidente",
    invariante:
      "Um incidente SOS so e aceito com grant valido e politica de retencao existente, e a resposta nunca alega mais que persisted_local (ADR-058/062/064).",
    arquivos: [
      "crates/godmesh-server/src/api.rs",
      "crates/godmesh-server/src/http.rs",
      "migrations/0001_core.sql",
      "crates/godmesh-emergency/src/incident.rs",
    ],
  },
  {
    key: "outbox-replay",
    invariante:
      "Replay reconstroi projecao e nunca reemite efeito externo irreversivel; confirmar e diferente de publicar (ADR-061).",
    arquivos: ["crates/godmesh-sync/src/outbox.rs", "crates/godmesh-sync/tests/outbox.rs"],
  },
  {
    key: "container-producao",
    invariante:
      "A imagem de producao roda sem privilegio, falha de forma explicita sem configuracao e nao embute segredo nem toolchain de build.",
    arquivos: ["deploy/prod/Dockerfile", "deploy/README.md", "crates/godmesh-server/src/main.rs"],
  },
];

const fatias = Array.isArray(args && args.fatias) && args.fatias.length > 0 ? args.fatias : fatiasDefault;
if (fatias.length !== fatiasDefault.length) {
  log("Escopo reduzido por args: " + fatias.length + " fatia(s). Nao e cobertura completa do repositorio.");
}

const contexto = (fatia) =>
  "Repositorio: " + ROOT + "\n" +
  "Fatia: " + fatia.key + "\n" +
  "Invariante sob revisao: " + fatia.invariante + "\n" +
  "Arquivos da fatia:\n- " + fatia.arquivos.join("\n- ") + "\n";

const schemaRevisao = {
  type: "object",
  required: ["bloqueios", "observacoes"],
  properties: {
    bloqueios: {
      type: "array",
      items: {
        type: "object",
        required: ["alegacao", "evidencia", "gravidade"],
        properties: {
          alegacao: { type: "string" },
          evidencia: { type: "string" },
          gravidade: { enum: ["critica", "alta", "media", "baixa"] },
        },
      },
    },
    observacoes: { type: "string" },
  },
};

const schemaAtaque = {
  type: "object",
  required: ["ataques", "resumo"],
  properties: {
    ataques: {
      type: "array",
      items: {
        type: "object",
        required: ["cenario", "invariante_violado", "veredito", "evidencia"],
        properties: {
          cenario: { type: "string" },
          invariante_violado: { type: "string" },
          veredito: { enum: ["viola", "mantem", "indeterminado"] },
          evidencia: { type: "string" },
        },
      },
    },
    resumo: { type: "string" },
  },
};

const schemaVeredito = {
  type: "object",
  required: ["refutado", "motivo"],
  properties: {
    refutado: { type: "boolean" },
    motivo: { type: "string" },
  },
};

const schemaSintese = {
  type: "object",
  required: ["decisao", "accoes", "justificacao"],
  properties: {
    decisao: { enum: ["aprovar", "bloquear"] },
    accoes: { type: "array", items: { type: "string" } },
    justificacao: { type: "string" },
  },
};

// Execucao SEQUENCIAL por fatia (pico de 2 agentes simultaneos).
// Motivo observado em 18/11/2026: com 8 agentes concorrentes o provider `pi`
// disputa o lock de ~/.pi/agent e a chamada falha com ELOCKED, derrubando a
// sintese da fatia. Menos paralelismo, resultado deterministico.
log("Concorrencia limitada a 2 agentes: contencao de lock do provider pi derrubou a rodada anterior.");

const resumo = [];
for (let i = 0; i < fatias.length; i += 1) {
  const fatia = fatias[i];

  const revisao = await agent(
    contexto(fatia) +
      "Leia os arquivos e o contrato correspondente (docs/06-planejamento/decisoes.md, docs/08-specs/).\n" +
      "Aponte APENAS bloqueios com evidencia verificavel (arquivo:linha ou comando). Nao invente. Sem bloqueio: lista vazia.",
    { label: "revisao:" + fatia.key, phase: "Revisao", schema: schemaRevisao },
  );

  const ataque = await agent(
    contexto(fatia) +
      "Tente VIOLAR o invariante com cenario concreto (ator, pre-condicao, sequencia). " +
      "Classifique cada cenario como viola, mantem ou indeterminado, com evidencia no codigo.\n" +
      "Bloqueios ja levantados: " + JSON.stringify(revisao ? revisao.bloqueios : []),
    { label: "ataque:" + fatia.key, phase: "Ataque", schema: schemaAtaque },
  );

  const bloqueios = (revisao && revisao.bloqueios) || [];
  const violacoes = ((ataque && ataque.ataques) || []).filter((a) => a.veredito === "viola");
  const alvos = bloqueios.map((b) => b.alegacao).concat(violacoes.map((v) => v.cenario));
  const limitados = alvos.slice(0, MAX_VERIFICACOES);
  if (alvos.length > limitados.length) {
    log("Fatia " + fatia.key + ": " + (alvos.length - limitados.length) + " alegacao(oes) sem cetico por limite de custo.");
  }

  const vereditos = await parallel(
    limitados.map((alvo, j) => () =>
      agent(
        contexto(fatia) +
          "Tente REFUTAR esta alegacao, com evidencia no codigo: " + alvo + "\n" +
          "Se nao conseguir refutar, diga refutado=false e explique por que a falha existe.",
        { label: "cetico:" + fatia.key + ":" + j, phase: "Verificacao", schema: schemaVeredito },
      ).then((v) => ({ alvo, veredito: v })),
    ),
  );

  const sintese = await agent(
    contexto(fatia) +
      "Sintetize o gate. Dados:\n" +
      "Revisao: " + JSON.stringify(revisao) + "\n" +
      "Ataque: " + JSON.stringify(ataque) + "\n" +
      "Ceticos: " + JSON.stringify(vereditos.filter(Boolean)) + "\n" +
      "Regra: bloquear se houver bloqueio nao refutado ou ataque com veredito viola. " +
      "Accoes devem ser verificaveis (arquivo e comportamento esperado).",
    { label: "sintese:" + fatia.key, phase: "Sintese", schema: schemaSintese },
  );

  if (sintese) {
    resumo.push({ fatia: fatia.key, decisao: sintese.decisao, accoes: sintese.accoes, justificacao: sintese.justificacao });
  } else {
    log("Fatia " + fatia.key + ": sintese indisponivel nesta rodada.");
  }
}

log("Fatias avaliadas: " + resumo.length + " de " + fatias.length + ".");
return resumo;
