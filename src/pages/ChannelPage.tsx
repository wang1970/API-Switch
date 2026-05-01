import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Edit, Plus, RefreshCw, Save, Trash2, Link2, CheckSquare, Square, Eye, EyeOff, Power, PowerOff, XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { cn, formatResponseMs } from "@/lib/utils";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  listChannels,
  createChannel,
  updateChannel,
  deleteChannel,
  fetchModels,
  fetchModelsDirect,
  selectModels,
  probeUrl,
  updateChannelResponseMs,
  listEntries,
} from "@/lib/api";
import type { ProbeResult, ModelCatalogMetaUpdate } from "@/lib/api";
import { getCatalogModel, getCatalogProviderLogo, formatTokenCount } from "@/lib/modelsCatalog";
import { API_TYPE_OPTIONS, API_TYPE_DEFAULT_URLS } from "@/types";
import type { ApiType, Channel, CreateChannelParams, ModelInfo, UpdateChannelParams } from "@/types";

type ChannelFormState = {
  id?: string;
  name: string;
  api_type: ApiType;
  base_url: string;
  api_key: string;
  notes: string;
  enabled: boolean;
};

function formatReleaseDate(value?: string) {
  if (!value) return "";
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

function buildEntryCatalogMeta(modelName: string): ModelCatalogMetaUpdate {
  const model = getCatalogModel(modelName);
  if (!model) {
    return {
      model: modelName,
      provider_logo: getCatalogProviderLogo(modelName),
      release_date: "",
      model_meta_zh: "",
      model_meta_en: "",
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

  const releaseDate = formatReleaseDate(model.release_date);
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
    model: modelName,
    provider_logo: getCatalogProviderLogo(modelName),
    release_date: releaseDate,
    model_meta_zh: buildMeta(zhFeatureLabels, "发布", "上下文", "输出"),
    model_meta_en: buildMeta(enFeatureLabels, "Release", "Context", "Output"),
  };
}

const defaultChannelForm = (): ChannelFormState => ({
  name: "",
  api_type: "custom",
  base_url: API_TYPE_DEFAULT_URLS.custom,
  api_key: "",
  notes: "",
  enabled: true,
});

export function ChannelPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [showEdit, setShowEdit] = useState(false);
  const [editingChannel, setEditingChannel] = useState<Channel | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [testingChannelId, setTestingChannelId] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, string>>({});

  const { data: channels, isLoading } = useQuery({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  // Fetch API pool entries to count per-channel entries
  const { data: entries } = useQuery({
    queryKey: ["entries"],
    queryFn: listEntries,
  });

  // Build a map of channel_id -> entry count
  const channelEntryCount = useMemo(() => {
    const map = new Map<string, number>();
    if (entries) {
      for (const entry of entries) {
        map.set(entry.channel_id, (map.get(entry.channel_id) || 0) + 1);
      }
    }
    return map;
  }, [entries]);

  const autoOpenedRef = useRef(false);
  useEffect(() => {
    if (!isLoading && channels && channels.length === 0 && !autoOpenedRef.current) {
      autoOpenedRef.current = true;
      setEditingChannel(null);
      setShowEdit(true);
    }
  }, [channels, isLoading]);

  const testAllChannels = useCallback(async () => {
    if (!channels) return;
    const toTest = [...channels];
    const results: Record<string, string> = {};
    for (const ch of toTest) {
      setTestingChannelId(ch.id);
      try {
        const probe = await probeUrl(ch.base_url);
        if (probe.reachable && probe.latency_ms > 0) {
          const secs = String(probe.latency_ms);
          await updateChannelResponseMs(ch.id, secs);
          results[ch.id] = secs;
        } else {
          results[ch.id] = "X";
        }
      } catch {
        results[ch.id] = "X";
      }
      setTestResults({ ...results });
      await new Promise((r) => setTimeout(r, 200));
    }
    setTestingChannelId(null);
    setTestResults({});
    queryClient.invalidateQueries({ queryKey: ["channels"] });
  }, [channels, queryClient]);

  const deleteMutation = useMutation({
    mutationFn: deleteChannel,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
    onError: (err) => {
      toast.error(`${t("channel.delete")} ${t("common.failed")}: ${err}`);
    },
  });

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between gap-4 flex-wrap">
        <div>
          <h1 className="text-xl font-semibold">{t("channel.title")}</h1>
          <p className="text-sm text-muted-foreground mt-1">{t("channel.description")}</p>
        </div>
        <div className="flex items-center gap-3">
          <Button
            size="sm"
            className="gap-1.5"
            onClick={() => {
              setEditingChannel(null);
              setShowEdit(true);
            }}
          >
            <Plus className="h-4 w-4" />
            {t("channel.add")}
          </Button>
        </div>
      </div>

      <div className="border rounded-lg overflow-hidden">
        <div className="overflow-x-hidden">
          <table className="w-full table-fixed text-sm">
              <colgroup>
                <col className="w-[18%]" />
                <col className="w-24" />
                <col />
                <col className="w-24" />
                <col className="w-24" />
                <col className="w-20" />
                <col className="w-32" />
              </colgroup>
              <thead>
                <tr className="border-b bg-muted/50">
                  <th className="px-4 py-3 text-left font-medium truncate">{t("channel.name")}</th>
                  <th className="px-4 py-3 text-left font-medium whitespace-nowrap">{t("channel.type")}</th>
                  <th className="px-4 py-3 text-left font-medium truncate">{t("channel.baseUrl")}</th>
                  <th className="px-4 py-3 text-left font-medium whitespace-nowrap">{t("channel.status")}</th>
                  <th className="px-4 py-3 text-left font-medium whitespace-nowrap">
                    <div className="flex items-center gap-1">
                      <span>{t("channel.responseTime")}</span>
                      <button
                        type="button"
                        onClick={testAllChannels}
                        disabled={testingChannelId !== null}
                        className="text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
                        title={t("channel.testAllLatency")}
                      >
                        <RefreshCw className={cn("h-3.5 w-3.5", testingChannelId !== null && "animate-spin")} />
                      </button>
                    </div>
                  </th>
                  <th className="px-4 py-3 text-left font-medium whitespace-nowrap">{t("channel.modelCount")}</th>
                  <th className="px-4 py-3 text-right font-medium whitespace-nowrap">{t("channel.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {(channels || []).map((channel) => (
                  <ChannelRow
                    key={channel.id}
                    channel={channel}
                    expanded={expandedId === channel.id}
                    onToggleExpand={() =>
                      setExpandedId((current) => (current === channel.id ? null : channel.id))
                    }
                    onEdit={() => {
                      setEditingChannel(channel);
                      setShowEdit(true);
                    }}
                    onDelete={() => deleteMutation.mutate(channel.id)}
                    entryCount={channelEntryCount.get(channel.id) || 0}
                    testingChannelId={testingChannelId}
                    testResults={testResults}
                  />
                ))}
              </tbody>
          </table>
        </div>

        {!(channels || []).length && (
          <div className="flex h-48 items-center justify-center text-muted-foreground">
            {t("common.noData")}
          </div>
        )}
      </div>

      <ChannelEditorDialog
        open={showEdit}
        channel={editingChannel}
        onOpenChange={setShowEdit}
      />
    </div>
  );
}

function ChannelRow({
  channel,
  expanded,
  onToggleExpand,
  onEdit,
  onDelete,
  entryCount,
  testingChannelId,
  testResults,
}: {
  channel: Channel;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: () => void;
  onDelete: () => void;
  entryCount: number;
  testingChannelId?: string | null;
  testResults?: Record<string, string>;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [modelSearch, setModelSearch] = useState("");
  const [fetching, setFetching] = useState(false);

  const fetchMutation = useMutation({
    mutationFn: () => fetchModels(channel.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      setFetching(false);
    },
    onError: (err) => {
      setFetching(false);
      toast.error(`${t("channel.models.fetch")} ${t("common.failed")}: ${err}`);
    },
  });

  const toggleMutation = useMutation({
    mutationFn: (enabled: boolean) =>
      updateChannel({ id: channel.id, enabled }),
    onMutate: async (enabled) => {
      await queryClient.cancelQueries({ queryKey: ["channels"] });
      const previous = queryClient.getQueryData<Channel[]>(["channels"]);
      queryClient.setQueryData<Channel[]>(["channels"], (old) =>
        old?.map((c) => (c.id === channel.id ? { ...c, enabled } : c)),
      );
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(["channels"], context.previous);
      }
      toast.error(`${t("common.save")} ${t("common.failed")}`);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
  });

  const selectMutation = useMutation({
    mutationFn: (models: string[]) => selectModels(channel.id, models, availableModels, models.map(buildEntryCatalogMeta)),
    onMutate: async (newSelected) => {
      await queryClient.cancelQueries({ queryKey: ["channels"] });
      const previous = queryClient.getQueryData<Channel[]>(["channels"]);
      queryClient.setQueryData<Channel[]>(["channels"], (old) =>
        old?.map((c) =>
          c.id === channel.id ? { ...c, selected_models: newSelected } : c,
        ),
      );
      return { previous };
    },
    onError: (err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(["channels"], context.previous);
      }
      toast.error(`${t("channel.models.saveAndSelect")} ${t("common.failed")}: ${err}`);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
  });

  const availableModels: ModelInfo[] = channel.available_models || [];
  const selectedModels: string[] = channel.selected_models || [];
  const modelCountText = `${entryCount} / ${availableModels.length}`;

  const filteredModels = modelSearch
    ? availableModels.filter((m) =>
      m.name.toLowerCase().includes(modelSearch.toLowerCase()),
    )
    : availableModels;

  const toggleModel = (modelName: string) => {
    if (selectMutation.isPending) return;
    const newSelected = selectedModels.includes(modelName)
      ? selectedModels.filter((m) => m !== modelName)
      : [...selectedModels, modelName];
    selectMutation.mutate(newSelected);
  };

  const selectAllFiltered = () => {
    if (selectMutation.isPending) return;
    const merged = Array.from(new Set([...selectedModels, ...filteredModels.map((m) => m.name)]));
    selectMutation.mutate(merged);
  };

  const clearAllSelected = () => {
    if (selectMutation.isPending) return;
    selectMutation.mutate([]);
  };

  return (
    <>
      <tr className="border-b hover:bg-muted/30">
        <td className="px-4 py-3 min-w-0">
          <button type="button" className="text-left min-w-0 max-w-full" onClick={onToggleExpand}>
            <div className="font-medium truncate">{channel.name}</div>
            {channel.notes ? (
              <div className="text-xs text-muted-foreground mt-1 line-clamp-1">{channel.notes}</div>
            ) : null}
          </button>
        </td>
        <td className="px-4 py-3">
          <span className="inline-flex rounded bg-secondary px-2 py-0.5 text-xs text-muted-foreground">
            {channel.api_type}
          </span>
        </td>
        <td className="px-4 py-3 font-mono text-xs min-w-0" title={channel.base_url}>
          <div className="truncate">{channel.base_url}</div>
        </td>
        <td className="px-4 py-3 whitespace-nowrap">
          <span className={cn(
            "inline-flex rounded-full px-2.5 py-1 text-xs font-medium",
            channel.enabled
              ? "bg-green-100 text-green-700"
              : "bg-muted text-muted-foreground",
          )}>
            {channel.enabled ? t("channel.enabled") : t("channel.disabled")}
          </span>
        </td>
        <td className="px-4 py-3 text-xs text-muted-foreground whitespace-nowrap">
          {testingChannelId === channel.id ? (
            <RefreshCw className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
          ) : testResults?.[channel.id] === "X" ? (
            <span className="text-red-500" title={t("channel.latencyTestFailed")}><XCircle className="h-3.5 w-3.5" /></span>
          ) : testResults?.[channel.id] ? (
            <span className="text-green-600">{formatResponseMs(testResults[channel.id])}</span>
          ) : channel.response_ms ? (
            <span className="text-green-600">{formatResponseMs(channel.response_ms)}</span>
          ) : (
            <span className="text-red-500" title={t("channel.latencyTestFailed")}><XCircle className="h-3.5 w-3.5" /></span>
          )}
        </td>
        <td className="px-4 py-3 whitespace-nowrap">{modelCountText}</td>
        <td className="px-4 py-3 whitespace-nowrap">
          <div className="flex items-center justify-end gap-1">
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onEdit}>
              <Edit className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => toggleMutation.mutate(!channel.enabled)}
              title={channel.enabled ? t("channel.disabled") : t("channel.enabled")}
            >
              {channel.enabled ? <Power className="h-4 w-4 text-green-600" /> : <PowerOff className="h-4 w-4 text-muted-foreground" />}
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onDelete}>
              <Trash2 className="h-4 w-4 text-destructive" />
            </Button>
          </div>
        </td>
      </tr>

      {expanded && (
        <tr className="border-b bg-muted/10">
          <td colSpan={7} className="px-4 py-4">
            <div className="space-y-4">
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  onClick={() => {
                    setFetching(true);
                    fetchMutation.mutate();
                  }}
                  disabled={fetching}
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", fetching && "animate-spin")} />
                  {fetching ? t("channel.models.fetching") : t("channel.models.fetch")}
                </Button>
                <Button size="sm" variant="outline" className="gap-1.5" onClick={onEdit}>
                  <Edit className="h-3.5 w-3.5" />
                  {t("channel.edit")}
                </Button>
              </div>

              <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4 text-sm">
                <InfoBlock label={t("channel.baseUrl")} value={channel.base_url} mono />
                <InfoBlock label={t("channel.apiKey")} value={channel.api_key} mono />
                <InfoBlock label={t("channel.updatedAt")} value={new Date(channel.updated_at * 1000).toLocaleString()} />
                <InfoBlock label={t("channel.modelCount")} value={modelCountText} />
              </div>

              <div className="rounded-md border bg-muted/20 p-4 space-y-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Link2 className="h-4 w-4" />
                  {t("channel.poolSyncTitle")}
                </div>
                <p className="text-sm text-muted-foreground">
                  {t("channel.poolSyncDesc")}
                </p>
                {selectedModels.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {selectedModels.slice(0, 12).map((model) => (
                      <span key={model} className="rounded-full border bg-background px-2.5 py-1 text-xs">
                        {model}
                      </span>
                    ))}
                    {selectedModels.length > 12 ? (
                      <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
                        +{selectedModels.length - 12}
                      </span>
                    ) : null}
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground">{t("channel.poolSyncEmpty")}</p>
                )}
              </div>

              <div className="space-y-2">
                <div className="flex flex-wrap gap-2 items-center">
                  <Input
                    placeholder={t("channel.models.search")}
                    value={modelSearch}
                    onChange={(e) => setModelSearch(e.target.value)}
                    className="h-8 text-sm flex-1 min-w-64"
                  />
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={selectAllFiltered}>
                    <CheckSquare className="h-3.5 w-3.5" />
                    {t("channel.models.selectFiltered")}
                  </Button>
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={clearAllSelected}>
                    <Square className="h-3.5 w-3.5" />
                    {t("channel.models.clearSelected")}
                  </Button>
                </div>
                {availableModels.length > 0 ? (
                  <div className="max-h-72 overflow-y-auto rounded-md border bg-background">
                    {filteredModels.map((model) => (
                      <label
                        key={model.id}
                        className="flex items-center gap-2 px-3 py-2 border-b last:border-b-0 hover:bg-accent cursor-pointer text-sm"
                      >
                        <Checkbox
                          checked={selectedModels.includes(model.name)}
                          onCheckedChange={() => toggleModel(model.name)}
                        />
                        <span className="truncate">{model.name}</span>
                        {model.owned_by ? (
                          <span className="text-xs text-muted-foreground ml-auto">{model.owned_by}</span>
                        ) : null}
                      </label>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground py-3">{t("channel.models.empty")}</p>
                )}
              </div>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function ChannelEditorDialog({
  open,
  channel,
  onOpenChange,
}: {
  open: boolean;
  channel: Channel | null;
  onOpenChange: (value: boolean) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<ChannelFormState>(defaultChannelForm());
  const [showApiKey, setShowApiKey] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [modelsValidated, setModelsValidated] = useState(false);
  const [modelSearch, setModelSearch] = useState("");
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [urlProbe, setUrlProbe] = useState<ProbeResult | null>(null);
  const [probingUrl, setProbingUrl] = useState(false);
  const probeSeqRef = useRef(0);
  const [endpointVerified, setEndpointVerified] = useState(false);

  const isEdit = !!channel;

  useEffect(() => {
    if (!open) return;
    setAvailableModels([]);
    setSelectedModels([]);
    setModelSearch("");
    setShowApiKey(false);
    setEndpointVerified(false);
    setModelsValidated(!!channel && (channel.available_models?.length || 0) > 0);
    if (channel) {
      setForm({
        id: channel.id,
        name: channel.name,
        api_type: channel.api_type as ApiType,
        base_url: channel.base_url,
        api_key: channel.api_key,
        notes: channel.notes,
        enabled: channel.enabled,
      });
      setAvailableModels(channel.available_models || []);
      setSelectedModels(channel.selected_models || []);
    } else {
      setForm(defaultChannelForm());
    }
  }, [channel, open]);

  // Debounced URL probe on base_url change (lightweight: just HTTP HEAD)
  useEffect(() => {
    const seq = ++probeSeqRef.current;
    if (!form.base_url.trim()) {
      setUrlProbe(null);
      setProbingUrl(false);
      return;
    }
    setUrlProbe(null);
    setProbingUrl(true);
    const t = setTimeout(async () => {
      try {
        const result = await probeUrl(form.base_url.trim());
        if (probeSeqRef.current === seq) setUrlProbe(result);
      } catch {
        if (probeSeqRef.current === seq) {
          setUrlProbe({ reachable: false, status_code: null, latency_ms: 0, detected_type: null, message: "Probe failed" });
        }
      } finally {
        if (probeSeqRef.current === seq) setProbingUrl(false);
      }
    }, 800);
    return () => clearTimeout(t);
  }, [form.base_url]);

  const setValue = <K extends keyof ChannelFormState>(key: K, value: ChannelFormState[K]) => {
    if (key === "api_type" || key === "base_url" || key === "api_key") {
      setEndpointVerified(false);
      setModelsValidated(false);
      setUrlProbe(null);
      setAvailableModels([]);
      setSelectedModels([]);
    }
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleApiTypeChange = (type: ApiType) => {
    setForm((prev) => ({
      ...prev,
      api_type: type,
      base_url: prev.base_url === API_TYPE_DEFAULT_URLS[prev.api_type]
        ? API_TYPE_DEFAULT_URLS[type] || prev.base_url
        : prev.base_url,
    }));
    setAvailableModels([]);
    setSelectedModels([]);
    setEndpointVerified(false);
    setModelsValidated(false);
  };

  const handleFetchModels = async () => {
    if (probingUrl) {
      toast.error("URL is still being checked. Please wait.");
      return;
    }

    let probe = urlProbe;
    if (!probe) {
      setProbingUrl(true);
      try {
        probe = await probeUrl(form.base_url.trim());
        setUrlProbe(probe);
      } catch {
        probe = { reachable: false, status_code: null, latency_ms: 0, detected_type: null, message: "Probe failed" };
        setUrlProbe(probe);
      } finally {
        setProbingUrl(false);
      }
    }

    if (!probe.reachable) {
      toast.error(`URL unreachable: ${probe.message}`);
      return;
    }

    setFetchingModels(true);
    setModelsValidated(false);
    try {
      // New and edit mode use the same semantics here:
      // fetch models validates only the current form values and updates the dialog state.
      // Persisting channel config and selected models happens only when the user clicks Save.
      const result = await fetchModelsDirect(form.api_type, form.base_url, form.api_key, false);

      setForm((prev) => ({
        ...prev,
        api_type: result.detected_type as ApiType,
        base_url: result.corrected_base_url || prev.base_url,
      }));
      setEndpointVerified(true);
      setModelsValidated(true);
      toast.success(`${t("channel.models.fetch")} → ${result.detected_type.toUpperCase()}`);
      setAvailableModels(result.models);
      const preSelected = await autoSelectModels(result.models, form.id);
      setSelectedModels(preSelected);
    } catch (err) {
      toast.error(`${t("channel.models.fetch")} ${t("common.failed")}: ${err}`);
    } finally {
      setFetchingModels(false);
    }
  };

  const toggleModel = (modelName: string) => {
    setSelectedModels((prev) =>
      prev.includes(modelName)
        ? prev.filter((m) => m !== modelName)
        : [...prev, modelName],
    );
  };

  const selectAllFiltered = () => {
    const filtered = modelSearch
      ? availableModels.filter((m) => m.name.toLowerCase().includes(modelSearch.toLowerCase()))
      : availableModels;
    const names = filtered.map((m) => m.name);
    setSelectedModels((prev) => Array.from(new Set([...prev, ...names])));
  };

  const clearAllSelected = () => {
    setSelectedModels([]);
  };

  const [saving, setSaving] = useState(false);

  const autoSelectModels = useCallback(async (models: ModelInfo[], channelId?: string): Promise<string[]> => {
    const sixMonthsAgo = new Date();
    sixMonthsAgo.setMonth(sixMonthsAgo.getMonth() - 6);
    const sixMonthsAgoStr = sixMonthsAgo.toISOString().slice(0, 10);

    // For existing channels, collect current channel's existing model names
    let existingModels = new Set<string>();
    if (channelId) {
      try {
        const entries = await listEntries();
        existingModels = new Set(
          entries.filter((e) => e.channel_id === channelId).map((e) => e.model.toLowerCase()),
        );
      } catch {}
    }

    const selected = new Set<string>();

    // 1. Select models released within 6 months
    for (const m of models) {
      const catalog = getCatalogModel(m.name);
      if (catalog?.release_date && catalog.release_date >= sixMonthsAgoStr) {
        selected.add(m.name);
      }
    }

    // 2. Select models already in this channel's API pool
    for (const m of models) {
      if (existingModels.has(m.name.toLowerCase())) {
        selected.add(m.name);
      }
    }

    return Array.from(selected);
  }, []);

  const handleSave = async () => {
    if (!form.name || !form.base_url || !form.api_key) return;
    setSaving(true);
    try {
      let channelId = form.id;

      // 1. Save channel info (create or update)
      if (channelId) {
        await updateChannel({
          id: channelId,
          name: form.name,
          api_type: form.api_type,
          base_url: form.base_url,
          api_key: form.api_key,
          notes: form.notes,
          enabled: form.enabled,
        });
      } else {
        const saved = await createChannel({
          name: form.name,
          api_type: form.api_type,
          base_url: form.base_url,
          api_key: form.api_key,
          notes: form.notes,
        });
        channelId = saved.id;
      }

      // 2. Save URL probe latency as response time
      if (urlProbe?.reachable && urlProbe.latency_ms > 0) {
        const ms = String(urlProbe.latency_ms);
        await updateChannelResponseMs(channelId, ms);
      }

      // 3. Sync selected models to pool (always sync to handle deletions)
      await selectModels(channelId, selectedModels, availableModels, selectedModels.map(buildEntryCatalogMeta));

      queryClient.invalidateQueries({ queryKey: ["channels"] });
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      onOpenChange(false);
    } catch (err) {
      toast.error(`${t("common.save")} ${t("common.failed")}: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const filteredModels = modelSearch
    ? availableModels.filter((m) => m.name.toLowerCase().includes(modelSearch.toLowerCase()))
    : availableModels;

  const canFetchModels = !!(form.name && form.base_url && form.api_key) && !probingUrl && urlProbe?.reachable !== false;
  const canSave = !!(form.name && form.base_url && form.api_key);

  const handleClose = () => {
    queryClient.invalidateQueries({ queryKey: ["channels"] });
    queryClient.invalidateQueries({ queryKey: ["entries"] });
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={(v) => {
      if (!v) setSaving(false);
      if (!v) {
        queryClient.invalidateQueries({ queryKey: ["channels"] });
        queryClient.invalidateQueries({ queryKey: ["entries"] });
      }
      onOpenChange(v);
    }}>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>{isEdit ? t("channel.edit") : t("channel.add")}</DialogTitle>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-auto">
          {/* Basic Configuration */}
          <div className="space-y-4 pb-4">
            <div className="space-y-2">
              <Label>{t("channel.name")} <span className="text-destructive">*</span></Label>
              <Input value={form.name} onChange={(e) => setValue("name", e.target.value)} placeholder={t("channel.name")} />
            </div>

            <div className="space-y-2">
              <Label>{t("channel.type")}</Label>
              <Select value={form.api_type} onValueChange={(v) => handleApiTypeChange(v as ApiType)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {API_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label>{t("channel.baseUrl")} <span className="text-destructive">*</span></Label>
              <div className="relative">
                <Input value={form.base_url} onChange={(e) => setValue("base_url", e.target.value)} placeholder="https://api.openai.com"
                  className={urlProbe ? (urlProbe.reachable ? "pr-24 border-green-500/50 focus-visible:ring-green-500/30" : "pr-24 border-red-500/50 focus-visible:ring-red-500/30") : "pr-8"} />
                <div className="absolute right-1.5 top-1/2 -translate-y-1/2 flex items-center gap-1 pointer-events-none">
                  {probingUrl ? (
                    <RefreshCw className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                  ) : urlProbe?.reachable ? (
                    <span className="text-[10px] text-green-600 font-medium whitespace-nowrap">{formatResponseMs(String(urlProbe.latency_ms))} ✓</span>
                  ) : urlProbe ? (
                    <span className="text-[10px] text-red-500" title={urlProbe.message}>✗</span>
                  ) : null}
                </div>
              </div>
            </div>

            <div className="space-y-2">
              <Label>{t("channel.apiKey")} <span className="text-destructive">*</span></Label>
              <div className="relative">
                <Input
                  type={showApiKey ? "text" : "password"}
                  value={form.api_key}
                  onChange={(e) => setValue("api_key", e.target.value)}
                  className="pr-10"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute right-0 top-0 h-full px-3 hover:bg-transparent"
                  onClick={() => setShowApiKey(!showApiKey)}
                >
                  {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </Button>
              </div>
            </div>
          </div>

          {/* Model Selection */}
          <div className="space-y-3 pt-4 border-t">
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm font-medium">
                  {availableModels.length > 0
                    ? t("channel.models.title", { count: availableModels.length })
                    : t("channel.models.empty")}
                </div>
                {availableModels.length > 0 && (
                  <div className="text-xs text-muted-foreground">
                    {t("channel.models.selected", { count: selectedModels.length })}
                  </div>
                )}
              </div>
              <Button
                size="sm"
                variant="outline"
                className="gap-1.5"
                onClick={handleFetchModels}
                disabled={!canFetchModels || fetchingModels}
              >
                <RefreshCw className={cn("h-3.5 w-3.5", fetchingModels && "animate-spin")} />
                {fetchingModels ? t("channel.models.fetching") : t("channel.models.fetch")}
              </Button>
            </div>

            {availableModels.length > 0 && (
              <>
                <div className="flex flex-wrap gap-2 items-center">
                  <Input
                    placeholder={t("channel.models.search")}
                    value={modelSearch}
                    onChange={(e) => setModelSearch(e.target.value)}
                    className="h-8 text-sm flex-1 min-w-48"
                  />
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={selectAllFiltered}>
                    <CheckSquare className="h-3.5 w-3.5" />
                    {t("channel.models.selectFiltered")}
                  </Button>
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={clearAllSelected}>
                    <Square className="h-3.5 w-3.5" />
                    {t("channel.models.clearSelected")}
                  </Button>
                </div>

                <ScrollArea className="h-48 rounded-md border">
                  <div>
                    {filteredModels.map((model) => (
                      <label
                        key={model.id}
                        className="flex items-center gap-2 px-3 py-1.5 border-b last:border-b-0 hover:bg-accent cursor-pointer text-sm"
                      >
                        <Checkbox
                          checked={selectedModels.includes(model.name)}
                          onCheckedChange={() => toggleModel(model.name)}
                        />
                        <span className="truncate">{model.name}</span>
                        {model.owned_by ? (
                          <span className="text-xs text-muted-foreground ml-auto">{model.owned_by}</span>
                        ) : null}
                      </label>
                    ))}
                  </div>
                </ScrollArea>

                {selectedModels.length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {selectedModels.slice(0, 20).map((model) => (
                      <span key={model} className="inline-flex items-center gap-1 rounded-full bg-secondary px-2 py-0.5 text-xs">
                        {model}
                        <button
                          type="button"
                          className="hover:text-destructive"
                          onClick={() => toggleModel(model)}
                        >
                          &times;
                        </button>
                      </span>
                    ))}
                    {selectedModels.length > 20 && (
                      <span className="rounded-full bg-secondary px-2 py-0.5 text-xs text-muted-foreground">
                        +{selectedModels.length - 20}
                      </span>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        </div>

        <DialogFooter className="gap-2 sm:gap-2">
          <div className="flex-1" />

          <Button variant="outline" onClick={handleClose} disabled={saving || fetchingModels}>
            {t("common.cancel")}
          </Button>
          <Button
            className="gap-1.5"
            onClick={handleSave}
            disabled={!canSave || saving || fetchingModels}
          >
            <Save className="h-4 w-4" />
            {saving ? "..." : isEdit ? t("common.save") : t("common.add")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
function InfoBlock({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-md border bg-background p-3">
      <div className="text-xs text-muted-foreground mb-1">{label}</div>
      <div className={cn("text-sm break-all", mono && "font-mono text-xs")}>{value}</div>
    </div>
  );
}

function maskSecret(value: string) {
  return value;
}

