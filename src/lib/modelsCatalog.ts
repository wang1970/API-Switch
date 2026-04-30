import rawCatalog from "../../models.json";

export type CatalogModel = {
  id: string;
  provider_id?: string;
  name?: string;
  family?: string;
  release_date?: string;
  last_updated?: string;
  open_weights?: boolean;
  reasoning?: boolean;
  tool_call?: boolean;
  structured_output?: boolean;
  attachment?: boolean;
  temperature?: boolean;
  interleaved?: unknown;
  status?: string;
  experimental?: unknown;
  modalities?: {
    input?: string[];
    output?: string[];
  };
  limit?: {
    context?: number;
    input?: number;
    output?: number;
  };
};

type CatalogProvider = {
  id: string;
  name?: string;
  api?: string;
  doc?: string;
  models?: Record<string, CatalogModel>;
};

type CatalogData = Record<string, CatalogProvider>;

const catalog = rawCatalog as CatalogData;
const modelIndex = new Map<string, CatalogModel>();
const modelEntries: Array<{ key: string; normalized: string; model: CatalogModel }> = [];

const removableSuffixes = [
  "free",
  "beta",
  "preview",
  "latest",
  "exp",
  "experimental",
  "thinking",
  "search",
  "online",
];

function scoreModel(model: CatalogModel): number {
  let score = 0;
  if (model.release_date) score += 2;
  if (model.last_updated) score += 1;
  if (model.reasoning) score += 1;
  if (model.tool_call) score += 1;
  if (model.structured_output) score += 1;
  if (model.attachment) score += 1;
  if (model.temperature) score += 1;
  if (model.modalities?.input?.length) score += model.modalities.input.length;
  if (model.modalities?.output?.length) score += model.modalities.output.length;
  if (model.limit?.context) score += 1;
  if (model.limit?.output) score += 1;
  return score;
}

function normalizeModelKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/^[a-z0-9._-]+\//, "")
    .replace(/[._]/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/-v(\d)(?=-|$)/g, "-$1")
    .replace(/-(20\d{2}|19\d{2})(?=-|$)/g, "")
    .replace(/-(\d{4}-\d{2}-\d{2}|\d{4}-\d{2}|\d{8})$/g, "")
    .replace(/^-+|-+$/g, "");
}

function buildKeyVariants(value: string): string[] {
  const variants = new Set<string>();
  let current = normalizeModelKey(value);
  if (!current) return [];
  variants.add(current);

  while (true) {
    const next = removableSuffixes.find((suffix) => current.endsWith(`-${suffix}`));
    if (!next) break;
    current = current.slice(0, -(next.length + 1)).replace(/-+$/g, "");
    if (!current) break;
    variants.add(current);
  }

  return Array.from(variants);
}

function tokenize(value: string): string[] {
  return value.split(/[^a-z0-9]+/).filter(Boolean);
}

function similarityScore(input: string, candidate: string): number {
  if (input === candidate) return 10_000;
  if (candidate.includes(input)) return 8_000 - (candidate.length - input.length);
  if (input.includes(candidate)) return 7_000 - (input.length - candidate.length);

  const inputTokens = new Set(tokenize(input));
  const candidateTokens = new Set(tokenize(candidate));
  let overlap = 0;
  for (const token of inputTokens) {
    if (candidateTokens.has(token)) overlap += 1;
  }
  if (overlap === 0) return -1;

  const penalty = Math.abs(input.length - candidate.length);
  return overlap * 100 - penalty;
}

for (const provider of Object.values(catalog)) {
  for (const [modelKey, model] of Object.entries(provider.models || {})) {
    const enrichedModel: CatalogModel = { ...model, provider_id: provider.id };
    const key = modelKey.toLowerCase();
    const current = modelIndex.get(key);
    if (!current || scoreModel(enrichedModel) > scoreModel(current)) {
      modelIndex.set(key, enrichedModel);
    }
    if (enrichedModel.id && enrichedModel.id.toLowerCase() !== key) {
      const idKey = enrichedModel.id.toLowerCase();
      const currentById = modelIndex.get(idKey);
      if (!currentById || scoreModel(enrichedModel) > scoreModel(currentById)) {
        modelIndex.set(idKey, enrichedModel);
      }
    }

    for (const variant of new Set([key, enrichedModel.id?.toLowerCase()].filter(Boolean) as string[])) {
      for (const normalized of buildKeyVariants(variant)) {
        modelEntries.push({ key: variant, normalized, model: enrichedModel });
      }
    }
  }
}

const catalogModelCache = new Map<string, CatalogModel | null>();

export function getCatalogModel(modelId: string): CatalogModel | null {
  const key = modelId.trim().toLowerCase();
  if (!key) return null;

  // 直接查 modelIndex（O(1)），命中则返回
  const direct = modelIndex.get(key);
  if (direct) return direct;

  // 未命中，遍历 modelEntries 做模糊匹配（慢路径）
  let best: { score: number; model: CatalogModel } | null = null;
  for (const variant of buildKeyVariants(key)) {
    const directVariant = modelIndex.get(variant);
    if (directVariant) return directVariant;
    for (const entry of modelEntries) {
      const score = similarityScore(variant, entry.normalized);
      if (score <= 0) continue;
      if (!best || score > best.score || (score === best.score && scoreModel(entry.model) > scoreModel(best.model))) {
        best = { score, model: entry.model };
      }
    }
  }

  return best?.model ?? null;
}

const brandRules: [RegExp, string][] = [
  [/^gpt-/, "openai"],
  [/^o[134]-/, "openai"],
  [/^chatgpt-/, "openai"],
  [/^codex-/, "openai"],
  [/^dall-e/, "openai"],
  [/^claude-/, "anthropic"],
  [/^gemini-/, "google"],
  [/^glm-/, "zhipu"],
  [/^mimo-/, "xiaomi"],
  [/^deepseek-/, "deepseek"],
  [/^qwen-/, "alibaba"],
  [/^kimi-/, "moonshot"],
  [/^grok-/, "xai"],
  [/^mistral-/, "mistral"],
  [/^llama-/, "meta"],
  [/^yi-/, "01ai"],
  [/^ernie-/, "baidu"],
  [/^hunyuan-/, "tencent"],
  [/^doubao-/, "volcengine"],
  [/^jina-/, "jina"],
  [/^cohere-/, "cohere"],
];

const namespaceAliases: Record<string, string> = {
  minimaxai: "minimax",
  xiaomi: "xiaomi",
  openai: "openai",
  anthropic: "anthropic",
  google: "google",
  gemini: "google",
  zhipuai: "zhipu",
  zhipu: "zhipu",
  moonshotai: "moonshot",
  moonshot: "moonshot",
  xai: "xai",
  deepseek: "deepseek",
  qwen: "alibaba",
  alibaba: "alibaba",
};

const familyRules: [RegExp, string][] = [
  [/^gpt/, "openai"],
  [/^claude/, "anthropic"],
  [/^gemini/, "google"],
  [/^glm/, "zhipu"],
  [/^mimo/, "xiaomi"],
  [/^minimax/, "minimax"],
  [/^deepseek/, "deepseek"],
  [/^qwen/, "alibaba"],
  [/^kimi/, "moonshot"],
  [/^grok/, "xai"],
  [/^mistral/, "mistral"],
  [/^llama/, "meta"],
];

export function getCatalogProviderLogo(modelId: string): string {
  const id = modelId.trim().toLowerCase();
  const catalogModel = getCatalogModel(modelId);
  const family = catalogModel?.family?.trim().toLowerCase() || "";

  // Layer 1: family rules (most stable for repeated provider aliases like minimaxai/...)
  for (const [re, brand] of familyRules) {
    if (family && re.test(family)) return `/logo/${brand}.svg`;
  }

  // Layer 2: namespace prefix with alias normalization
  const slashIdx = id.indexOf("/");
  if (slashIdx > 0) {
    const ns = id.slice(0, slashIdx);
    const brand = namespaceAliases[ns] || ns;
    return `/logo/${brand}.svg`;
  }

  // Layer 3: model id prefix rules
  for (const [re, brand] of brandRules) {
    if (re.test(id)) return `/logo/${brand}.svg`;
  }

  // Layer 4: custom fallback
  return "/logo/custom.svg";
}

export function formatTokenCount(value?: number): string | null {
  if (!value || value <= 0) return null;
  if (value >= 1_000_000) {
    const n = value / 1_000_000;
    return `${Number.isInteger(n) ? n : n.toFixed(1)}M`;
  }
  if (value >= 1_000) {
    const n = value / 1_000;
    return `${Number.isInteger(n) ? n : n.toFixed(1)}K`;
  }
  return String(value);
}
