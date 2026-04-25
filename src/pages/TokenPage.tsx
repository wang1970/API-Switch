import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Copy, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import { listAccessKeys, createAccessKey, deleteAccessKey, toggleAccessKey } from "@/lib/api";
import type { AccessKey } from "@/types";

export function TokenPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [createdKey, setCreatedKey] = useState<AccessKey | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const { data: keys, isLoading } = useQuery({
    queryKey: ["accessKeys"],
    queryFn: listAccessKeys,
  });

  const createMutation = useMutation({
    mutationFn: createAccessKey,
    onSuccess: (key) => {
      queryClient.invalidateQueries({ queryKey: ["accessKeys"] });
      setCreatedKey(key);
      setNewKeyName("");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteAccessKey,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["accessKeys"] }),
  });

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      toggleAccessKey(id, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["accessKeys"] }),
  });

  const copyKey = async (key: string, id: string) => {
    await navigator.clipboard.writeText(key);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 3000);
  };

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  const formatDate = (ts: number) => {
    const d = new Date(ts * 1000);
    return d.toLocaleString();
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("token.title")}</h1>
        <Button size="sm" className="gap-1.5" onClick={() => setShowCreate(true)}>
          <Plus className="h-4 w-4" />
          {t("token.add")}
        </Button>
      </div>

      {keys?.length ? (
        <div className="border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-muted/50 text-left text-muted-foreground">
                <th className="px-4 py-2 font-medium">{t("token.name")}</th>
                <th className="px-4 py-2 font-medium">{t("token.key")}</th>
                <th className="px-4 py-2 font-medium">{t("token.status")}</th>
                <th className="px-4 py-2 font-medium">{t("token.created")}</th>
                <th className="px-4 py-2 font-medium w-24">{t("common.action")}</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((key) => (
                <tr key={key.id} className="border-b last:border-b-0 hover:bg-muted/30">
                  <td className="px-4 py-3 font-medium">{key.name}</td>
                  <td className="px-4 py-3">
                    <code className="text-xs bg-muted px-1.5 py-0.5 rounded font-mono">
                      {key.key.slice(0, 8)}...{key.key.slice(-4)}
                    </code>
                  </td>
                  <td className="px-4 py-3">
                    <span className={key.enabled ? "text-green-600" : "text-muted-foreground"}>
                      {key.enabled ? t("token.active") : t("token.disabled")}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-muted-foreground text-xs">{formatDate(key.created_at)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => copyKey(key.key, key.id)}
                      >
                        {copiedId === key.id ? (
                          <Check className="h-3.5 w-3.5 text-green-600" />
                        ) : (
                          <Copy className="h-3.5 w-3.5" />
                        )}
                      </Button>
                      <Switch
                        checked={key.enabled}
                        onCheckedChange={(checked) =>
                          toggleMutation.mutate({ id: key.id, enabled: checked })
                        }
                      />
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => deleteMutation.mutate(key.id)}
                      >
                        <Trash2 className="h-3.5 w-3.5 text-destructive" />
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="flex h-64 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}

      {/* Create Dialog */}
      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("token.add")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("token.name")}</Label>
              <Input
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder="My Laptop"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowCreate(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              onClick={() => createMutation.mutate(newKeyName)}
              disabled={!newKeyName || createMutation.isPending}
            >
              {t("common.add")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Created Key Dialog */}
      <Dialog open={!!createdKey} onOpenChange={(v) => !v && setCreatedKey(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("token.add")}</DialogTitle>
            <DialogDescription>{t("token.keyWarning")}</DialogDescription>
          </DialogHeader>
          {createdKey && (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <code className="flex-1 text-sm bg-muted p-3 rounded font-mono break-all">
                  {createdKey.key}
                </code>
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => copyKey(createdKey.key, createdKey.id)}
                >
                  {copiedId === createdKey.id ? (
                    <Check className="h-4 w-4 text-green-600" />
                  ) : (
                    <Copy className="h-4 w-4" />
                  )}
                </Button>
              </div>
            </div>
          )}
          <DialogFooter>
            <Button onClick={() => setCreatedKey(null)}>{t("common.close")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
