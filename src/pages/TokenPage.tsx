import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Copy, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("token.title")}</h1>
        <Button size="sm" className="gap-1.5" onClick={() => setShowCreate(true)}>
          <Plus className="h-4 w-4" />
          {t("token.add")}
        </Button>
      </div>

      <div className="grid gap-3">
        {keys?.map((key) => (
          <Card key={key.id}>
            <CardContent className="flex items-center gap-4 p-4">
              <div className="flex-1 min-w-0">
                <p className="font-medium">{key.name}</p>
                <div className="flex items-center gap-2 mt-1">
                  <code className="text-xs bg-muted px-2 py-0.5 rounded font-mono">
                    {key.key.slice(0, 8)}...{key.key.slice(-4)}
                  </code>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6"
                    onClick={() => copyKey(key.key, key.id)}
                  >
                    {copiedId === key.id ? (
                      <Check className="h-3 w-3 text-green-600" />
                    ) : (
                      <Copy className="h-3 w-3" />
                    )}
                  </Button>
                </div>
              </div>
              <Switch
                checked={key.enabled}
                onCheckedChange={(checked) =>
                  toggleMutation.mutate({ id: key.id, enabled: checked })
                }
              />
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => deleteMutation.mutate(key.id)}
              >
                <Trash2 className="h-4 w-4 text-destructive" />
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>

      {!keys?.length && (
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
                  <Copy className="h-4 w-4" />
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
