import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Edit, Plus, RefreshCw, Save, Trash2, Link2, CheckSquare, Square, Eye, EyeOff, Power, PowerOff, XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
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
import type { ProbeResult } from "@/lib/api";
import { getCatalogModel } from "@/lib/modelsCatalog";
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
          const secs = probe.latency_ms >= 1000
            ? `${(probe.latency_ms / 1000).toFixed(1)}s`
            : `${probe.latency_ms}ms`;
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

      <Card>
        <CardHeader className="pb-3">
          <CardTitle>{t("channel.listTitle")}</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <div className="overflow-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-muted/50">
                  <th className="px-4 py-3 text-left font-medium">{t("channel.name")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.type")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.baseUrl")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.status")}</th>
                  <th className="px-4 py-3 text-left font-medium">
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
                  <th className="px-4 py-3 text-left font-medium">{t("channel.modelCount")}</th>
                  <th className="px-4 py-3 text-right font-medium">{t("channel.actions")}</th>
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
        </CardContent>
      </Card>

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
  testingChannelId,
  testResults,
}: {
  channel: Channel;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: () => void;
  onDelete: () => void;
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
    mutationFn: (models: string[]) => selectModels(channel.id, models),
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
        <td className="px-4 py-3">
          <button type="button" className="text-left" onClick={onToggleExpand}>
            <div className="font-medium">{channel.name}</div>
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
        <td className="px-4 py-3 font-mono text-xs max-w-[320px] truncate">{channel.base_url}</td>
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
            <span className="text-green-600">{testResults[channel.id]}</span>
          ) : channel.response_ms ? (
            <span className="text-green-600">{channel.response_ms}</span>
          ) : (
            <span className="text-red-500" title={t("channel.latencyTestFailed")}><XCircle className="h-3.5 w-3.5" /></span>
          )}
        </td>
        <td className="px-4 py-3 whitespace-nowrap">{availableModels.length}</td>
        <td className="px-4 py-3">
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
                <InfoBlock label={t("channel.apiKey")} value={maskSecret(channel.api_key)} mono />
                <InfoBlock label={t("channel.updatedAt")} value={new Date(channel.updated_at * 1000).toLocaleString()} />
                <InfoBlock label={t("channel.modelCount")} value={`${selectedModels.length} / ${availableModels.length}`} />
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
  const [modelSearch, setModelSearch] = useState("");
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [urlProbe, setUrlProbe] = useState<ProbeResult | null>(null);
  const [probingUrl, setProbingUrl] = useState(false);
  const [endpointVerified, setEndpointVerified] = useState(false);

  const isEdit = !!channel;

  useEffect(() => {
    if (!open) return;
    setAvailableModels([]);
    setSelectedModels([]);
    setModelSearch("");
    setShowApiKey(false);
    setEndpointVerified(false);
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
    if (!form.base_url.trim()) { setUrlProbe(null); return; }
    setProbingUrl(true);
    const t = setTimeout(async () => {
      try { setUrlProbe(await probeUrl(form.base_url.trim())); }
      catch { setUrlProbe({ reachable: false, status_code: null, latency_ms: 0, detected_type: null, message: "Probe failed" }); }
      finally { setProbingUrl(false); }
    }, 800);
    return () => clearTimeout(t);
  }, [form.base_url]);

  const setValue = <K extends keyof ChannelFormState>(key: K, value: ChannelFormState[K]) => {
    if (key === "api_type" || key === "base_url" || key === "api_key") {
      setEndpointVerified(false);
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
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    try {
      if (form.id) {
        // Edit mode: use existing channel ID
        const models = await fetchModels(form.id);
        setAvailableModels(models);
        queryClient.invalidateQueries({ queryKey: ["channels"] });
        const preSelected = await autoSelectModels(models, form.id);
        setSelectedModels(preSelected);
      } else {
        // New mode: smart fetch — auto-detect API type + fetch models in one call
        const result = await fetchModelsDirect(form.api_type, form.base_url, form.api_key, endpointVerified);
        setForm((prev) => ({
          ...prev,
          api_type: result.detected_type as ApiType,
          base_url: result.corrected_base_url || prev.base_url,
        }));
        setEndpointVerified(true);
        toast.success(`${t("channel.models.fetch")} → ${result.detected_type.toUpperCase()}`);
        setAvailableModels(result.models);
        const preSelected = await autoSelectModels(result.models, undefined);
        setSelectedModels(preSelected);
      }
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
        const secs = urlProbe.latency_ms >= 1000
          ? `${(urlProbe.latency_ms / 1000).toFixed(1)}s`
          : `${urlProbe.latency_ms}ms`;
        await updateChannelResponseMs(channelId, secs);
      }

      // 3. Sync selected models to pool (always sync to handle deletions)
      await selectModels(channelId, selectedModels);

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

  const canSave = form.name && form.base_url && form.api_key;

  return (
    <Dialog open={open} onOpenChange={(v) => {
      if (!v) setSaving(false);
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
              <Select value={form.api_type} onValueChange={(v) => handleApiTypeChange(v as ApiType)} disabled={isEdit}>
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
              {isEdit && (
                <p className="text-xs text-muted-foreground">{t("channel.typeChangeDisabled")}</p>
              )}
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
                    <span className="text-[10px] text-green-600 font-medium whitespace-nowrap">{urlProbe.latency_ms}ms ✓</span>
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
                disabled={!canSave || fetchingModels}
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

          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>
            {t("common.cancel")}
          </Button>
          <Button
            className="gap-1.5"
            onClick={handleSave}
            disabled={!canSave || saving}
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
  if (!value) return "-";
  if (value.length <= 8) return "*".repeat(value.length);
  return `${value.slice(0, 4)}****${value.slice(-4)}`;
}
