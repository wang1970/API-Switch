import { useEffect, useMemo, useState, useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { GripVertical, Plus, MessageSquare, RefreshCw, XCircle } from "lucide-react";
import { toast } from "sonner";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
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
import { listEntries, toggleEntry, reorderEntries, listChannels, createEntry, testEntryLatency } from "@/lib/api";
import type { ApiEntry, Channel } from "@/types";
import { cn } from "@/lib/utils";
import { TestChatDialog } from "@/components/proxy/TestChatDialog";
import { getCatalogModel, formatTokenCount, type CatalogModel } from "@/lib/modelsCatalog";
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

function modalityLabel(value: string, t: (key: string) => string) {
  switch (value) {
    case "text": return t("apiPool.modelMeta.modalities.text");
    case "image": return t("apiPool.modelMeta.modalities.image");
    case "pdf": return t("apiPool.modelMeta.modalities.pdf");
    case "audio": return t("apiPool.modelMeta.modalities.audio");
    case "video": return t("apiPool.modelMeta.modalities.video");
    default: return value;
  }
}

function getFeatureLabels(model: CatalogModel, t: (key: string) => string) {
  const labels: string[] = [];
  const inputs = model.modalities?.input || [];
  const outputs = model.modalities?.output || [];

  if (outputs.includes("image")) labels.push(t("apiPool.modelMeta.features.imageGeneration"));
  if (inputs.includes("image")) labels.push(t("apiPool.modelMeta.features.imageUnderstanding"));
  if (inputs.includes("audio") || outputs.includes("audio")) labels.push(t("apiPool.modelMeta.features.audio"));
  if (inputs.includes("video") || outputs.includes("video")) labels.push(t("apiPool.modelMeta.features.video"));
  if (inputs.includes("pdf") || outputs.includes("pdf")) labels.push(t("apiPool.modelMeta.features.pdf"));
  if (model.reasoning) labels.push(t("apiPool.modelMeta.features.reasoning"));
  if (model.interleaved) labels.push(t("apiPool.modelMeta.features.interleaved"));
  if (model.tool_call) labels.push(t("apiPool.modelMeta.features.toolCall"));
  if (model.structured_output) labels.push(t("apiPool.modelMeta.features.structuredOutput"));
  if (model.attachment) labels.push(t("apiPool.modelMeta.features.attachment"));
  if (model.temperature) labels.push(t("apiPool.modelMeta.features.temperature"));
  return labels;
}

function shortReleaseDate(value?: string) {
  if (!value) return null;
  const match = value.match(/^(\d{4})-(\d{2})/);
  if (match) {
    return `${match[1]}-${match[2]}`;
  }
  return value;
}

function ModelMetaBlock({ modelId }: { modelId: string }) {
  const { t } = useTranslation();
  const model = getCatalogModel(modelId);

  if (!model) return null;

  const features = getFeatureLabels(model, t);
  const releaseDate = shortReleaseDate(model.release_date);
  const context = formatTokenCount(model.limit?.context);
  const output = formatTokenCount(model.limit?.output);
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

function SortablePoolEntryCard({
  entry,
  onTest,
  testingEntryIds,
  testResult,
}: {
  entry: ApiEntry;
  onTest: (entry: ApiEntry) => void;
  testingEntryIds?: Set<string>;
  testResult?: string;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const toggleMutation = useMutation({
    mutationFn: (enabled: boolean) => toggleEntry(entry.id, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["entries"] }),
  });

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: entry.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 10 : undefined,
    opacity: isDragging ? 0.8 : undefined,
  };
  const cooldownRemaining = formatCooldownRemaining(entry.cooldown_until);

  return (
    <Card
      ref={setNodeRef}
      style={style}
      className={cn("transition-opacity", !entry.enabled && "opacity-60")}
    >
      <CardContent className="flex items-center gap-3 p-4">
        <div
          {...attributes}
          {...listeners}
          className="cursor-pointer text-muted-foreground hover:text-foreground"
        >
          <GripVertical className="h-3.5 w-3.5 shrink-0" />
        </div>
        <div className="flex-1 min-w-0 overflow-hidden">
          <div className="flex items-center gap-2 min-w-0">
            <StatusDot state={getEntryStatus(entry)} />
            <span className="font-medium truncate">
              {entry.channel_name || "—"} / {entry.model}
            </span>
            {testingEntryIds?.has(entry.id) ? (
              <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground shrink-0" />
            ) : testResult === "X" ? (
              <XCircle className="h-3 w-3 text-red-500 shrink-0" />
            ) : testResult ? (
              <span className="text-xs text-green-600 shrink-0">({testResult})</span>
            ) : entry.response_ms === "X" ? (
              <XCircle className="h-3 w-3 text-red-500 shrink-0" />
            ) : entry.response_ms ? (
              <span className="text-xs text-green-600 shrink-0">({entry.response_ms})</span>
            ) : null}
            {cooldownRemaining ? (
              <span className="text-xs text-red-500 shrink-0">
                {t("apiPool.cooldownInline", { time: cooldownRemaining })}
              </span>
            ) : null}
          </div>
          <ModelMetaBlock modelId={entry.model} />
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-muted-foreground hover:text-foreground touch-none"
          onClick={() => onTest(entry)}
        >
          <MessageSquare className="h-4 w-4" />
        </Button>
        <Switch
          checked={entry.enabled}
          onCheckedChange={(checked) => toggleMutation.mutate(checked)}
          className="touch-none"
        />
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

  const sorted = [...(entries || [])].sort((a, b) => {
    if (a.enabled !== b.enabled) return a.enabled ? -1 : 1;
    return a.sort_index - b.sort_index;
  });

  const displayEntries = localOrder
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
      queryClient.invalidateQueries({ queryKey: ["entries"] });
      setLocalOrder(null);
    },
  });

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
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
          results[entry.id] = result.response_ms;
          if (result.status === "cooldown") {
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
    <div className="p-6 space-y-6">
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

      <Card>
        <CardHeader className="pb-3">
          <Input
            className="flex-1"
            placeholder={t("apiPool.search")}
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
          />
        </CardHeader>
        <CardContent>
          {!entries?.length ? (
            <div className="flex h-48 items-center justify-center text-muted-foreground">
              {t("apiPool.empty")}
            </div>
          ) : (
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
                  {filteredEntries.map((entry) => (
                    <SortablePoolEntryCard key={entry.id} entry={entry} onTest={setTestEntry} testingEntryIds={testingEntryIds} testResult={testResults[entry.id]} />
                  ))}
                </div>
              </SortableContext>
            </DndContext>
          )}
        </CardContent>
      </Card>

      <AddApiDialog open={showAdd} onOpenChange={setShowAdd} channels={channels || []} />
      <TestChatDialog open={!!testEntry} onOpenChange={(v) => !v && setTestEntry(null)} entry={testEntry} />
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
    mutationFn: () => createEntry({
      channel_id: channelId,
      model: modelName,
      display_name: displayName || undefined,
    }),
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
