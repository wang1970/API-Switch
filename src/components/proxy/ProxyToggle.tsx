import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Power } from "lucide-react";
import { Button } from "@/components/ui/button";
import { startProxy, stopProxy, getProxyStatus } from "@/lib/api";
import { toast } from "sonner";

export function ProxyToggle() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data: status } = useQuery({
    queryKey: ["proxyStatus"],
    queryFn: getProxyStatus,
    refetchInterval: 5000,
  });

  const startMutation = useMutation({
    mutationFn: startProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxyStatus"] }),
    onError: (err) => {
      toast.error(`${t("settings.proxy.start")} ${t("common.failed")}: ${err}`, { duration: Infinity });
    },
  });

  const stopMutation = useMutation({
    mutationFn: stopProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxyStatus"] }),
    onError: (err) => {
      toast.error(`${t("settings.proxy.stop")} ${t("common.failed")}: ${err}`, { duration: Infinity });
    },
  });

  const running = status?.running ?? false;
  const port = status?.port ?? 9090;

  return (
    <div className="flex items-center gap-2">
      <Button
        variant={running ? "destructive" : "default"}
        size="sm"
        className="gap-1.5"
        onClick={() => (running ? stopMutation.mutate() : startMutation.mutate())}
        disabled={startMutation.isPending || stopMutation.isPending}
      >
        <Power className="h-3.5 w-3.5" />
        {running ? t("settings.proxy.stop") : t("settings.proxy.start")}
      </Button>
      <span className="text-xs text-muted-foreground">
        {running ? `:${port}` : t("settings.proxy.stopped")}
      </span>
    </div>
  );
}
