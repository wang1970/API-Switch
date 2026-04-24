import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Edit, Plus, RefreshCw, Save, Trash2, Link2, CheckSquare, Square, Eye, EyeOff, Power, PowerOff,
} from "lucide-react";
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
  selectModels,
} from "@/lib/api";
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
  api_type: "openai",
  base_url: API_TYPE_DEFAULT_URLS.openai,
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

  const { data: channels, isLoading } = useQuery({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteChannel,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
    onError: (err) => {
      alert(`Delete failed: ${err}`);
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
}: {
  channel: Channel;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: () => void;
  onDelete: () => void;
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
      alert(`Fetch models failed: ${err}`);
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
      alert(`Select models failed: ${err}`);
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
        <td className="px-4 py-3">
          <span className={cn(
            "inline-flex rounded-full px-2.5 py-1 text-xs font-medium",
            channel.enabled
              ? "bg-green-100 text-green-700"
              : "bg-muted text-muted-foreground",
          )}>
            {channel.enabled ? t("channel.enabled") : t("channel.disabled")}
          </span>
        </td>
        <td className="px-4 py-3">{availableModels.length}</td>
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
          <td colSpan={6} className="px-4 py-4">
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

  const isEdit = !!channel;

  useEffect(() => {
    if (!open) return;
    setAvailableModels([]);
    setSelectedModels([]);
    setModelSearch("");
    setShowApiKey(false);
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

  const setValue = <K extends keyof ChannelFormState>(key: K, value: ChannelFormState[K]) => {
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
    // Type changed 鈥?reset models
    setAvailableModels([]);
    setSelectedModels([]);
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    try {
      let channelId = form.id;
      if (!channelId) {
        // Create channel first to get an ID for fetching
        const saved = await createChannel({
          name: form.name,
          api_type: form.api_type,
          base_url: form.base_url,
          api_key: form.api_key,
          notes: form.notes,
        });
        setForm((prev) => ({ ...prev, id: saved.id }));
        channelId = saved.id;
        queryClient.invalidateQueries({ queryKey: ["channels"] });
      }
      const models = await fetchModels(channelId);
      setAvailableModels(models);
      // Auto-select none 鈥?user picks manually
      setSelectedModels([]);
    } catch (err) {
      alert(`Fetch models failed: ${err}`);
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

      // 2. Sync selected models to pool (if any)
      if (selectedModels.length > 0) {
        await selectModels(channelId, selectedModels);
      }

      queryClient.invalidateQueries({ queryKey: ["channels"] });
      onOpenChange(false);
    } catch (err) {
      alert(`Save failed: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const filteredModels = modelSearch
    ? availableModels.filter((m) => m.name.toLowerCase().includes(modelSearch.toLowerCase()))
    : availableModels;

  const canSave = form.name && form.base_url && form.api_key;

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) setSaving(false); onOpenChange(v); }}>
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
              <Input value={form.base_url} onChange={(e) => setValue("base_url", e.target.value)} placeholder="https://api.openai.com" />
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
