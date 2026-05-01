import { useEffect, useMemo, useState, useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { GripVertical, Plus, MessageSquare, RefreshCw, XCircle, X, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { listEntries, toggleEntry, reorderEntries, listChannels, createEntry, testEntryLatency, deleteEntry, updateSettings, backfillEntryCatalogMeta, getSettings } from "@/lib/api";
import { DEFAULT_SETTINGS, type ApiEntry, type Channel, type ModelSortMode } from "@/types";
import { cn, formatResponseMs, parseResponseMs } from "@/lib/utils";
import { TestChatDialog } from "@/components/proxy/TestChatDialog";
import { getCatalogModel, getCatalogProviderLogo, formatTokenCount } from "@/lib/modelsCatalog";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

function StatusDot({ state }: { state: string }) {
  return (
    <span
      className={cn("inline-block h-2 w-2 rounded-full", {
        "bg-green-500": state === "closed",
        "bg-red-500": state === "open",
        "bg-gray-400": state === "disabled",
      })}
    />
  );
}

function formatReleaseDate(value?: string) {
  if (!value) return null;
  const compact = value.match(/^(\d{4})(\d{2})(\d{2})$/);
  if (compact) {
    return `${compact[1]}-${compact[2]}-${compact[3]}`;
  }
  // Normalize month-only format: "2026-04" → "2026-04-01"
  const monthOnly = value.match(/^(\d{4})-(\d{2})$/);
  if (monthOnly) {
    return `${value}-01`;
  }
  return value;
}

/** Parse release_date to epoch seconds for sorting, matching backend parse_release_date.
 * Supports: YYYY-MM-DD, YYYY-MM, YYYYMMDD. Returns null if unparseable. */
function parseReleaseDateForSort(entry: ApiEntry): number | null {
  const raw = entry.release_date?.trim();
  if (!raw) return null;

  // Try YYYY-MM-DD
  const m1 = raw.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (m1) return new Date(raw).getTime();

  // Try YYYYMMDD
  const m2 = raw.match(/^(\d{4})(\d{2})(\d{2})$/);
  if (m2) return new Date(`${m2[1]}-${m2[2]}-${m2[3]}`).getTime();

  // Try YYYY-MM (append -01)
  const m3 = raw.match(/^(\d{4})-(\d{2})$/);
  if (m3) return new Date(`${raw}-01`).getTime();

  return null;
}

type CatalogDisplayMeta = {
  logo: string;
  releaseDate: string;
  context: string;
  output: string;
  features: string[];
  modelMetaZh: string;
  modelMetaEn: string;
};

function buildCatalogDisplayMeta(modelId: string): CatalogDisplayMeta {
  const model = getCatalogModel(modelId);
  if (!model) {
    return {
      logo: getCatalogProviderLogo(modelId),
      releaseDate: "",
      context: "",
      output: "",
      features: [],
      modelMetaZh: "",
      modelMetaEn: "",
    };
  }

  const inputs = model.modalities?.input || [];
  const outputs = model.modalities?.output || [];
  const features: string[] = [];
  if (outputs.includes("image")) features.push("imageGeneration");
  if (inputs.includes("image")) features.push("imageUnderstanding");
  if (inputs.includes("audio") || outputs.includes("audio")) features.push("audio");
  if (inputs.includes("video") || outputs.includes("video")) features.push("video");
  if (inputs.includes("pdf") || outputs.includes("pdf")) features.push("pdf");
  if (model.reasoning) features.push("reasoning");
  if (model.interleaved) features.push("interleaved");
  if (model.tool_call) features.push("toolCall");
  if (model.structured_output) features.push("structuredOutput");
  if (model.attachment) features.push("attachment");
  if (model.temperature) features.push("temperature");

  const releaseDate = formatReleaseDate(model.release_date) || "";
  const context = formatTokenCount(model.limit?.context) || "";
  const output = formatTokenCount(model.limit?.output) || "";
  const zhFeatureLabels: Record<string, string> = {
    imageGeneration: "生图",
    imageUnderstanding: "识图",
    audio: "音频",
    video: "视频",
    pdf: "PDF",
    reasoning: "推理",
    interleaved: "思维链",
    toolCall: "工具调用",
    structuredOutput: "结构输出",
    attachment: "附件",
    temperature: "温度",
  };
  const enFeatureLabels: Record<string, string> = {
    imageGeneration: "Image Gen",
    imageUnderstanding: "Vision",
    audio: "Audio",
    video: "Video",
    pdf: "PDF",
    reasoning: "Reasoning",
    interleaved: "Reasoning Trace",
    toolCall: "Tool Calling",
    structuredOutput: "Struct Output",
    attachment: "Attachment",
    temperature: "Temperature",
  };
  const buildMeta = (labels: Record<string, string>, releaseLabel: string, contextLabel: string, outputLabel: string) => [
    releaseDate ? `${releaseLabel}: ${releaseDate}` : null,
    ...features.map(f => labels[f]).filter(Boolean),
    context ? `${contextLabel}: ${context}` : null,
    output ? `${outputLabel}: ${output}` : null,
  ].filter(Boolean).join(" / ");

  return {
    logo: getCatalogProviderLogo(modelId),
    releaseDate,
    context,
    output,
    features,
    modelMetaZh: buildMeta(zhFeatureLabels, "发布", "上下文", "输出"),
    modelMetaEn: buildMeta(enFeatureLabels, "Release", "Context", "Output"),
  };
}

function getEntryDisplayMeta(entry: ApiEntry, catalogMap: Map<string, CatalogDisplayMeta>): CatalogDisplayMeta {
  const fallback = catalogMap.get(entry.model) || buildCatalogDisplayMeta(entry.model);
  return {
    logo: entry.provider_logo || fallback.logo || "/logo/custom.svg",
    releaseDate: entry.release_date || fallback.releaseDate || "",
    context: fallback.context,
    output: fallback.output,
    features: fallback.features,
    modelMetaZh: entry.model_meta_zh || fallback.modelMetaZh || "",
    modelMetaEn: entry.model_meta_en || fallback.modelMetaEn || "",
  };
}

function ModelMetaBlock({ metaZh, metaEn, releaseDate, context, output, features }: {
  metaZh?: string;
  metaEn?: string;
  releaseDate?: string;
  context?: string;
  output?: string;
  features: string[];
}) {
  const { t, i18n } = useTranslation();
  const storedMeta = i18n.language?.startsWith("zh") ? metaZh : metaEn;
  if (storedMeta) {
    return <div className="mt-1 text-xs text-muted-foreground truncate">{storedMeta}</div>;
  }

  if (!releaseDate && features.length === 0 && !context && !output) return null;

  const segments = [
    releaseDate ? `${t("apiPool.modelMeta.releaseDate")}: ${releaseDate}` : null,
    ...features,
    context ? `${t("apiPool.modelMeta.context")}: ${context}` : null,
    output ? `${t("apiPool.modelMeta.output")}: ${output}` : null,
  ].filter(Boolean) as string[];

  if (segments.length === 0) return null;

  return (
    <div className="mt-1 text-xs text-muted-foreground truncate">
      {segments.join(" / ")}
    </div>
  );
}

function getEntryStatus(entry: ApiEntry) {
  const now = Math.floor(Date.now() / 1000);
  if (entry.cooldown_until && entry.cooldown_until > now) return "open";

  if (!entry.enabled) return "disabled";

  return "closed";
}

function formatCooldownRemaining(cooldownUntil: number | null | undefined) {
  if (!cooldownUntil) return null;
  const remaining = Math.max(0, cooldownUntil - Math.floor(Date.now() / 1000));
  if (remaining <= 0) return null;
  const minutes = Math.ceil(remaining / 60);
  return `${minutes}m`;
}

function CardBody({
  entry,
  onTest,
  onDelete,
  onToggleIntent,
  testingEntryIds,
  testResult,
  catalogLogo,
  catalogReleaseDate,
  catalogContext,
  catalogOutput,
  catalogFeatures,
  modelMetaZh,
  modelMetaEn,
}: {
  entry: ApiEntry;
  onTest: (entry: ApiEntry) => void;
  onDelete: (entry: ApiEntry) => void;
  onToggleIntent: (entry: ApiEntry, enabled: boolean, options: { ctrlKey: boolean; shiftKey: boolean; metaKey: boolean }) => void;
  testingEntryIds?: Set<string>;
  testResult?: string;
  catalogLogo: string;
  catalogReleaseDate?: string;
  catalogContext?: string;
  catalogOutput?: string;
  catalogFeatures: string[];
  modelMetaZh?: string;
  modelMetaEn?: string;
}) {
  const { t } = useTranslation();
  const cooldownRemaining = formatCooldownRemaining(entry.cooldown_until);

  return (
    <>
      <div className="h-10 w-10 rounded-md bg-muted/40 border flex items-center justify-center shrink-0 mt-0.5">
        <img
          src={catalogLogo}
          alt="provider"
          className="h-6 w-6 shrink-0"
          onError={(e) => {
            e.currentTarget.onerror = null;
            e.currentTarget.src = "/logo/custom.svg";
          }}
        />
      </div>
      <div className="flex-1 min-w-0 overflow-hidden">
        <div className="flex items-center gap-2 min-w-0">
          <span className="font-medium truncate">{entry.channel_name || "—"}</span>
          <StatusDot state={getEntryStatus(entry)} />
          <span className="font-medium truncate">{entry.model}</span>
          {testingEntryIds?.has(entry.id) ? (
            <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground shrink-0" />
          ) : testResult === "X" ? (
            <XCircle className="h-3 w-3 text-red-500 shrink-0" />
          ) : testResult ? (
            <span className="text-xs text-green-600 shrink-0">({formatResponseMs(testResult)})</span>
          ) : entry.response_ms === "X" ? (
            <XCircle className="h-3 w-3 text-red-500 shrink-0" />
          ) : entry.response_ms ? (
            <span className="text-xs text-green-600 shrink-0">({formatResponseMs(entry.response_ms)})</span>
          ) : null}
          {cooldownRemaining ? (
            <span className="text-xs text-red-500 shrink-0">
              {t("apiPool.cooldownInline", { time: cooldownRemaining })}
            </span>
          ) : null}
        </div>
        <ModelMetaBlock
          metaZh={modelMetaZh}
          metaEn={modelMetaEn}
          releaseDate={catalogReleaseDate}
          context={catalogContext}
          output={catalogOutput}
          features={catalogFeatures.map(f => t(`apiPool.modelMeta.features.${f}`))}
        />
      </div>
      <Button variant="ghost" size="icon" className="h-8 w-8 text-muted-foreground hover:text-foreground touch-none" onClick={() => onTest(entry)}>
        <MessageSquare className="h-4 w-4" />
      </Button>
      <Button variant="ghost" size="icon" className="h-8 w-8 text-muted-foreground hover:text-red-500 touch-none" onClick={() => onDelete(entry)}>
        <Trash2 className="h-4 w-4" />
      </Button>
      <Switch
        checked={entry.enabled}
        onClick={(e) => {
          e.stopPropagation();
          onToggleIntent(entry, !entry.enabled, { ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, metaKey: e.metaKey });
        }}
        onCheckedChange={() => {}}
        className="touch-none"
      />
    </>
  );
}

function SortablePoolEntryCard(props: {
  entry: ApiEntry;
  onTest: (entry: ApiEntry) => void;
  onDelete: (entry: ApiEntry) => void;
  onToggleIntent: (entry: ApiEntry, enabled: boolean, options: { ctrlKey: boolean; shiftKey: boolean; metaKey: boolean }) => void;
  testingEntryIds?: Set<string>;
  testResult?: string;
  catalogLogo: string;
  catalogReleaseDate?: string;
  catalogContext?: string;
  catalogOutput?: string;
  catalogFeatures: string[];
  modelMetaZh?: string;
  modelMetaEn?: string;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: props.entry.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 10 : undefined,
    opacity: isDragging ? 0.8 : undefined,
  };

  return (
    <Card ref={setNodeRef} style={style} className={cn("transition-opacity", !props.entry.enabled && "opacity-60")}>
      <CardContent className="flex items-center gap-3 p-4">
        <div {...attributes} {...listeners} className="cursor-pointer text-muted-foreground hover:text-foreground">
          <GripVertical className="h-3.5 w-3.5 shrink-0" />
        </div>
        <CardBody {...props} />
      </CardContent>
    </Card>
  );
}

function PoolEntryCard(props: {
  entry: ApiEntry;
  onTest: (entry: ApiEntry) => void;
  onDelete: (entry: ApiEntry) => void;
  onToggleIntent: (entry: ApiEntry, enabled: boolean, options: { ctrlKey: boolean; shiftKey: boolean; metaKey: boolean }) => void;
  testingEntryIds?: Set<string>;
  testResult?: string;
  catalogLogo: string;
  catalogReleaseDate?: string;
  catalogContext?: string;
  catalogOutput?: string;
  catalogFeatures: string[];
  modelMetaZh?: string;
  modelMetaEn?: string;
}) {
  return (
    <Card className={cn("transition-opacity", !props.entry.enabled && "opacity-60")}>
      <CardContent className="flex items-center gap-3 p-4">
        <CardBody {...props} />
      </CardContent>
    </Card>
  );
}

export function ApiPoolPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [localOrder, setLocalOrder] = useState<string[] | null>(null);
  const [filterText, setFilterText] = useState("");
  const [filterChannel, setFilterChannel] = useState<string>("all");
  const [showAdd, setShowAdd] = useState(false);
  const [testEntry, setTestEntry] = useState<ApiEntry | null>(null);
  const [testingEntryIds, setTestingEntryIds] = useState<Set<string>>(new Set());
  const [testResults, setTestResults] = useState<Record<string, string>>({});
  const [testProgress, setTestProgress] = useState<{ current: number; total: number } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ApiEntry | null>(null);
  // sortMode is driven by backend settings; localStorage is only a fallback
  // before the first settings response arrives.
  const [sortMode, setSortMode] = useState<ModelSortMode>(() => {
    const saved = localStorage.getItem("api-switch-sort-mode");
    if (saved === "latest" || saved === "fastest" || saved === "custom") return saved;
    return "custom";
  });

  // Listen for entries changes (cooldown, tray priority, etc.)
  useEffect(() => {
    const unlisten1 = listen("tray-priority-changed", () => {
      queryClient.invalidateQueries({ queryKey: ["entries"] });
    });
    const unlisten2 = listen("entries-changed", () => {
      queryClient.invalidateQueries({ queryKey: ["entries"] });
    });
    return () => {
      unlisten1.then((fn) => fn());
      unlisten2.then((fn) => fn());
    };
  }, [queryClient]);

  const { data: entries, isLoading } = useQuery({
    queryKey: ["entries"],
    queryFn: listEntries,
  });

  const { data: channels } = useQuery({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  // Always sync sortMode from backend settings once available.
  useEffect(() => {
    if (settings?.default_sort_mode === "latest" || settings?.default_sort_mode === "fastest" || settings?.default_sort_mode === "custom") {
      setSortMode(settings.default_sort_mode);
    }
  }, [settings?.default_sort_mode]);

  // Pre-compute catalog metadata for all entries (used for fallback display and backfill normalization).
  const catalogMap = useMemo(() => {
    const map = new Map<string, CatalogDisplayMeta>();
    for (const entry of (entries || [])) {
      if (!map.has(entry.model)) {
        map.set(entry.model, buildCatalogDisplayMeta(entry.model));
      }
    }
    return map;
  }, [entries]);

  useEffect(() => {
    const missing = (entries || [])
      .map((entry) => {
        const meta = catalogMap.get(entry.model);
        if (!meta) return null;
        const next = {
          id: entry.id,
          provider_logo: meta.logo || entry.provider_logo || "",
          release_date: meta.releaseDate || entry.release_date || "",
          model_meta_zh: meta.modelMetaZh || entry.model_meta_zh || "",
          model_meta_en: meta.modelMetaEn || entry.model_meta_en || "",
        };
        const changed =
          next.provider_logo !== (entry.provider_logo || "") ||
          next.release_date !== (entry.release_date || "") ||
          next.model_meta_zh !== (entry.model_meta_zh || "") ||
          next.model_meta_en !== (entry.model_meta_en || "");
        return changed ? next : null;
      })
      .filter(Boolean) as Array<{
        id: string;
        provider_logo: string;
        release_date: string;
        model_meta_zh: string;
        model_meta_en: string;
      }>;

    if (missing.length === 0) return;

    backfillEntryCatalogMeta(missing).then(() => {
      queryClient.invalidateQueries({ queryKey: ["entries"] });
    });
  }, [entries, catalogMap, queryClient]);

  // Sort entries to match backend router.rs apply_sort_mode logic.
  // Backend sorts ALL entries by the chosen metric without enabled/disabled split.
  // Frontend keeps enabled above disabled for display clarity, but applies the same
  // metric-based sort within each group so the order reflects actual routing priority.
  const sorted = useMemo(() => {
    const list = [...(entries || [])];
    const enabled = list.filter((e) => e.enabled);
    const disabled = list.filter((e) => !e.enabled);

    const sortGroup = (group: ApiEntry[]) => {
      switch (sortMode) {
        case "latest": {
          // Match backend sort_by_release_date: date desc, fallback to sort_index
          group.sort((a, b) => {
            const dateA = parseReleaseDateForSort(a);
            const dateB = parseReleaseDateForSort(b);
            if (dateA && dateB) return dateB - dateA; // newest first
            if (dateA) return -1;
            if (dateB) return 1;
            return a.sort_index - b.sort_index;
          });
          break;
        }
        case "fastest": {
          // Match backend sort_by_latency: response_ms asc, no data goes last
          group.sort((a, b) => {
            const msA = parseResponseMs(a.response_ms) ?? Infinity;
            const msB = parseResponseMs(b.response_ms) ?? Infinity;
            return msA - msB;
          });
          break;
        }
        default: {
          // custom: sort by sort_index
          group.sort((a, b) => a.sort_index - b.sort_index);
          break;
        }
      }
    };

    sortGroup(enabled);
    sortGroup(disabled);

    return [...enabled, ...disabled];
  }, [entries, sortMode, catalogMap]);

  const displayEntries = localOrder && sortMode === "custom"
    ? localOrder
      .map((id) => sorted.find((e) => e.id === id))
      .filter(Boolean) as ApiEntry[]
    : sorted;

  const filteredEntries = useMemo(() => {
    const term = filterText.trim().toLowerCase();
    return displayEntries.filter((entry) => {
      const matchesChannel = filterChannel === "all" || entry.channel_id === filterChannel;
      const matchesTerm = !term || [entry.display_name, entry.model, entry.channel_name || ""]
        .join(" ")
        .toLowerCase()
        .includes(term);
      return matchesChannel && matchesTerm;
    });
  }, [displayEntries, filterChannel, filterText]);

  const reorderMutation = useMutation({
    mutationFn: reorderEntries,
    onSuccess: () => {
      const scrollY = window.scrollY;
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      setLocalOrder(null);
      requestAnimationFrame(() => window.scrollTo(0, scrollY));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteEntry(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      setDeleteTarget(null);
    },
  });

  const handleToggleIntent = useCallback(async (
    entry: ApiEntry,
    enabled: boolean,
    options: { ctrlKey: boolean; shiftKey: boolean; metaKey: boolean },
  ) => {
    const hotKey = options.ctrlKey || options.metaKey;

    if (options.shiftKey) {
      const targetEntries = filteredEntries;
      const currentIds = localOrder
        ? localOrder
        : displayEntries.map((e) => e.id);
      await Promise.all(targetEntries.map((e) => toggleEntry(e.id, enabled)));
      queryClient.setQueryData<ApiEntry[] | undefined>(["entries"], (prev) =>
        prev?.map((e) => targetEntries.some((t) => t.id === e.id) ? { ...e, enabled } : e),
      );
      setLocalOrder(currentIds);
      requestAnimationFrame(() => {
        queryClient.invalidateQueries({ queryKey: ["entries"] });
      });
      return;
    }

    await toggleEntry(entry.id, enabled);
    queryClient.setQueryData<ApiEntry[] | undefined>(["entries"], (prev) =>
      prev?.map((e) => (e.id === entry.id ? { ...e, enabled } : e)),
    );

    if (enabled && hotKey && sortMode === "custom") {
      const current = localOrder
        ? localOrder.map((id) => displayEntries.find((e) => e.id === id)).filter(Boolean) as ApiEntry[]
        : displayEntries;
      const orderedIds = [entry.id, ...current.filter((e) => e.id !== entry.id).map((e) => e.id)];
      const scrollY = window.scrollY;
      setLocalOrder(orderedIds);
      reorderMutation.mutate(orderedIds);
      requestAnimationFrame(() => window.scrollTo(0, scrollY));
    } else {
      // Preserve visual position after single toggle
      const currentIds = localOrder
        ? localOrder
        : displayEntries.map((e) => e.id);
      setLocalOrder(currentIds);
      requestAnimationFrame(() => {
        queryClient.invalidateQueries({ queryKey: ["entries"] });
      });
    }
  }, [displayEntries, filteredEntries, localOrder, queryClient, reorderMutation]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleSortModeChange = useCallback((mode: ModelSortMode) => {
    setSortMode(mode);
    setLocalOrder(null);
    localStorage.setItem("api-switch-sort-mode", mode);
    // Use queryClient.getQueryData to get the latest settings without relying on stale closure
    const currentSettings = queryClient.getQueryData(["settings"]);
    const merged = { ...DEFAULT_SETTINGS, ...currentSettings, default_sort_mode: mode };
    updateSettings(merged).then(() => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    });
  }, [queryClient]);

  const handleDragEnd = (event: DragEndEvent) => {
    if (sortMode !== "custom") return;
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldIndex = filteredEntries.findIndex((e) => e.id === active.id);
    const newIndex = filteredEntries.findIndex((e) => e.id === over.id);
    if (oldIndex === -1 || newIndex === -1) return;

    const newOrder = arrayMove(filteredEntries, oldIndex, newIndex);
    const newIds = newOrder.map((e) => e.id);
    const remainingIds = displayEntries
      .filter((entry) => !newIds.includes(entry.id))
      .map((entry) => entry.id);
    const mergedOrder = [...newIds, ...remainingIds];
    setLocalOrder(mergedOrder);
    reorderMutation.mutate(mergedOrder);
  };

  const testAllEntries = useCallback(async () => {
    if (!entries || testProgress) return;
    const results: Record<string, string> = {};
    let completed = 0;
    const total = entries.length;
    setTestProgress({ current: 0, total });

    // Group entries by channel for parallel testing across channels
    const grouped = new Map<string, ApiEntry[]>();
    for (const entry of entries) {
      const list = grouped.get(entry.channel_id) || [];
      list.push(entry);
      grouped.set(entry.channel_id, list);
    }

    // Test one channel sequentially
    const testChannel = async (channelEntries: ApiEntry[]) => {
      for (const entry of channelEntries) {
        setTestingEntryIds((prev) => {
          const next = new Set(prev);
          // Remove previous entries from this channel
          for (const e of channelEntries) next.delete(e.id);
          next.add(entry.id);
          return next;
        });
        try {
          const result = await testEntryLatency(entry.id);
          if (result.status === "ok") {
            results[entry.id] = result.response_ms;
          } else if (result.status === "failed") {
            results[entry.id] = "X";
            await toggleEntry(entry.id, false);
            queryClient.setQueryData<ApiEntry[] | undefined>(["entries"], (prev) =>
              prev?.map((e) => (e.id === entry.id ? { ...e, enabled: false } : e)),
            );
          } else if (result.status === "disabled") {
            results[entry.id] = result.response_ms || "X";
            queryClient.setQueryData<ApiEntry[] | undefined>(["entries"], (prev) =>
              prev?.map((e) => (e.id === entry.id ? { ...e, enabled: false } : e)),
            );
          } else {
            results[entry.id] = "X";
          }
        } catch {
          results[entry.id] = "X";
        }
        completed++;
        setTestProgress({ current: completed, total });
        setTestResults({ ...results });
      }
    };

    // Run all channels in parallel
    await Promise.all([...grouped.values()].map(testChannel));

    // Refresh and clear
    setTestingEntryIds(new Set());
    setTestResults({});
    setTestProgress(null);
    queryClient.invalidateQueries({ queryKey: ["entries"] });
  }, [entries, queryClient, testProgress]);

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  return (
    <div className="p-6">
      <div className="flex items-center justify-between gap-4 flex-wrap">
        <div>
          <h1 className="text-xl font-semibold">{t("apiPool.title")}</h1>
          <p className="text-sm text-muted-foreground mt-1">{t("apiPool.description")}</p>
        </div>
        <div className="flex items-center gap-3">
          <Button size="sm" variant="outline" className="gap-1.5 min-w-[140px]" onClick={testAllEntries} disabled={!!testProgress}>
            <RefreshCw className={cn("h-4 w-4", testProgress && "animate-spin")} />
            {testProgress ? `${testProgress.current}/${testProgress.total}` : t("apiPool.testAllLatency")}
          </Button>
          <Button size="sm" className="gap-1.5" onClick={() => setShowAdd(true)}>
            <Plus className="h-4 w-4" />
            {t("apiPool.addModel")}
          </Button>
        </div>
      </div>

      <div className="sticky top-0 z-10 bg-background pt-1 pb-1">
        <div className="relative">
          <Input
            className="flex-1 pr-8"
            placeholder={t("apiPool.search")}
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
          />
          {filterText ? (
            <button
              type="button"
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              onClick={() => setFilterText("")}
            >
              <X className="h-4 w-4" />
            </button>
          ) : null}
        </div>
      </div>
      <div className="mt-2 -mx-px w-[calc(100%+2px)] rounded-t-md border border-b-0 bg-background">
        <div className="flex w-full items-center px-0 py-0">
          {(["custom", "latest", "fastest"] as ModelSortMode[]).map((mode) => (
            <Button
              key={mode}
              size="sm"
              variant={sortMode === mode ? "default" : "ghost"}
                className="h-6 flex-1 rounded-none first:rounded-tl-md last:rounded-tr-md px-3 text-xs"
              onClick={() => handleSortModeChange(mode)}
            >
              {t(`apiPool.sort.${mode}`)}
            </Button>
          ))}
        </div>
      </div>

      <Card className="rounded-t-none">
        <CardContent className="p-4 pt-4">
          {!entries?.length ? (
            <div className="flex h-48 items-center justify-center text-muted-foreground">
              {t("apiPool.empty")}
            </div>
          ) : sortMode === "custom" ? (
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleDragEnd}
            >
              <SortableContext
                items={filteredEntries.map((e) => e.id)}
                strategy={verticalListSortingStrategy}
              >
                <div className="flex flex-col gap-3">
                  {filteredEntries.map((entry) => {
                    const meta = getEntryDisplayMeta(entry, catalogMap);
                    return (
                      <SortablePoolEntryCard
                        key={entry.id}
                        entry={entry}
                        onTest={setTestEntry}
                        onDelete={setDeleteTarget}
                        onToggleIntent={handleToggleIntent}
                        testingEntryIds={testingEntryIds}
                        testResult={testResults[entry.id]}
                        catalogLogo={meta.logo}
                        catalogReleaseDate={meta.releaseDate}
                        catalogContext={meta.context}
                        catalogOutput={meta.output}
                        catalogFeatures={meta.features}
                        modelMetaZh={meta.modelMetaZh}
                        modelMetaEn={meta.modelMetaEn}
                      />
                    );
                  })}
                </div>
              </SortableContext>
            </DndContext>
          ) : (
            <div className="flex flex-col gap-3">
              {filteredEntries.map((entry) => {
                const meta = getEntryDisplayMeta(entry, catalogMap);
                return (
                  <PoolEntryCard
                    key={entry.id}
                    entry={entry}
                    onTest={setTestEntry}
                    onDelete={setDeleteTarget}
                    onToggleIntent={handleToggleIntent}
                    testingEntryIds={testingEntryIds}
                    testResult={testResults[entry.id]}
                    catalogLogo={meta.logo}
                    catalogReleaseDate={meta.releaseDate}
                    catalogContext={meta.context}
                    catalogOutput={meta.output}
                    catalogFeatures={meta.features}
                    modelMetaZh={meta.modelMetaZh}
                    modelMetaEn={meta.modelMetaEn}
                  />
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <AddApiDialog open={showAdd} onOpenChange={setShowAdd} channels={channels || []} />
      <TestChatDialog open={!!testEntry} onOpenChange={(v) => !v && setTestEntry(null)} entry={testEntry} />

      <Dialog open={!!deleteTarget} onOpenChange={(v) => !v && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("apiPool.deleteTitle")}</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            {t("apiPool.deleteDesc", { name: `${deleteTarget?.channel_name || "—"} / ${deleteTarget?.model || ""}` })}
          </p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={deleteMutation.isPending}
              onClick={() => {
                if (deleteTarget) deleteMutation.mutate(deleteTarget.id);
              }}
            >
              {t("common.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function AddApiDialog({
  open,
  onOpenChange,
  channels,
}: {
  open: boolean;
  onOpenChange: (value: boolean) => void;
  channels: Channel[];
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [channelId, setChannelId] = useState("");
  const [modelName, setModelName] = useState("");
  const [displayName, setDisplayName] = useState("");

  const channelOptions = channels.filter((c) => c.enabled);

  const createMutation = useMutation({
    mutationFn: () => {
      const meta = buildCatalogDisplayMeta(modelName);
      return createEntry({
        channel_id: channelId,
        model: modelName,
        display_name: displayName || undefined,
        provider_logo: meta.logo,
        release_date: meta.releaseDate,
        model_meta_zh: meta.modelMetaZh,
        model_meta_en: meta.modelMetaEn,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      onOpenChange(false);
    },
    onError: (err) => {
      toast.error(`${t("apiPool.addApi")} ${t("common.failed")}: ${err}`);
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("apiPool.addModel")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <div className="text-sm font-medium">{t("apiPool.channel")}</div>
            <Select
              value={channelId}
              onValueChange={(value) => {
                setChannelId(value);
                setModelName("");
                setDisplayName("");
              }}
            >
              <SelectTrigger>
                <SelectValue placeholder={t("apiPool.selectChannel")} />
              </SelectTrigger>
              <SelectContent>
                {channelOptions.map((channel) => (
                  <SelectItem key={channel.id} value={channel.id}>
                    {channel.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium">{t("apiPool.model")}</div>
            <Input
              value={modelName}
              onChange={(e) => setModelName(e.target.value)}
              placeholder={t("apiPool.modelPlaceholder")}
            />
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium">{t("apiPool.displayName")}</div>
            <Input
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder={t("apiPool.displayNamePlaceholder")}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={() => createMutation.mutate()} disabled={!channelId || !modelName || createMutation.isPending}>
            {t("common.add")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
