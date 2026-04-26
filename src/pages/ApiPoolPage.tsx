import { useEffect, useMemo, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { GripVertical, Plus, MessageSquare } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
import { listEntries, toggleEntry, reorderEntries, listChannels, createEntry } from "@/lib/api";
import type { ApiEntry, Channel, ApiType } from "@/types";
import { cn } from "@/lib/utils";
import { API_TYPE_OPTIONS } from "@/types";
import { TestChatDialog } from "@/components/proxy/TestChatDialog";
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

function getEntryStatus(entry: ApiEntry) {
  const now = Math.floor(Date.now() / 1000);
  if (entry.cooldown_until && entry.cooldown_until > now) return "open";

  if (!entry.enabled) return "disabled";

  return "closed";
}

function SortablePoolEntryCard({
  entry,
  onTest,
}: {
  entry: ApiEntry;
  onTest: (entry: ApiEntry) => void;
}) {
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
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <StatusDot state={getEntryStatus(entry)} />
            <span className="font-medium truncate">{entry.display_name}</span>
          </div>
          <p className="text-xs text-muted-foreground mt-0.5">
            {entry.channel_name || "—"} / {entry.model}
            {entry.owned_by && `  |  ${entry.owned_by}`}
          </p>
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
        <Button size="sm" className="gap-1.5" onClick={() => setShowAdd(true)}>
          <Plus className="h-4 w-4" />
          {t("apiPool.addApi")}
        </Button>
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
                <div className="grid gap-3">
                  {filteredEntries.map((entry) => (
                    <SortablePoolEntryCard key={entry.id} entry={entry} onTest={setTestEntry} />
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
      alert(`Add API failed: ${err}`);
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("apiPool.addApi")}</DialogTitle>
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
