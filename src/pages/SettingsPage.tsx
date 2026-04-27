import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { getSettings, updateSettings, getProxyStatus, startProxy, stopProxy } from "@/lib/api";
import { toast } from "sonner";
import { DEFAULT_SETTINGS } from "@/types";

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  const { data: proxyStatus } = useQuery({
    queryKey: ["proxyStatus"],
    queryFn: getProxyStatus,
  });

  const updateMutation = useMutation({
    mutationFn: updateSettings,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  const s = { ...DEFAULT_SETTINGS, ...settings };

  const update = (key: string, value: any) => {
    updateMutation.mutate({ ...s, [key]: value });
  };

  const changeLocale = (locale: string) => {
    i18n.changeLanguage(locale);
    localStorage.setItem("api-switch-locale", locale);
    update("locale", locale);
  };

  return (
    <div className="p-6 max-w-2xl">
      <h1 className="text-xl font-semibold mb-6">{t("settings.title")}</h1>

      {/* Proxy Settings */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("settings.proxy.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <Label>{t("settings.proxy.port")}</Label>
            <Input
              type="number"
              className="w-32"
              value={s.listen_port}
              onChange={(e) => update("listen_port", parseInt(e.target.value) || 9090)}
            />
          </div>
          <div className="flex items-center justify-between">
            <Label>{t("settings.proxy.enabled")}</Label>
            <Switch
              checked={proxyStatus?.running ?? s.proxy_enabled}
              onCheckedChange={async (v) => {
                try {
                  if (v) {
                    await startProxy();
                  } else {
                    await stopProxy();
                  }
                  queryClient.invalidateQueries({ queryKey: ["proxyStatus"] });
                  queryClient.invalidateQueries({ queryKey: ["settings"] });
                } catch (err) {
                  toast.error(`${t("settings.proxy.title")} ${t("common.failed")}: ${err}`);
                }
              }}
            />
          </div>
          {proxyStatus?.running && (
            <div className="text-sm text-muted-foreground">
              {t("settings.proxy.address")}: http://127.0.0.1:{proxyStatus.port}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Security */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("settings.security.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label>{t("settings.security.forceKey")}</Label>
              <p className="text-xs text-muted-foreground">{t("settings.security.forceKeyDesc")}</p>
            </div>
            <Switch
              checked={s.access_key_required}
              onCheckedChange={(v) => update("access_key_required", v)}
            />
          </div>
        </CardContent>
      </Card>

      {/* Circuit Breaker */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("settings.circuit.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <Label>{t("settings.circuit.threshold")}</Label>
            <Input
              type="number"
              className="w-32"
              value={s.circuit_failure_threshold}
              onChange={(e) => update("circuit_failure_threshold", parseInt(e.target.value) || 1)}
            />
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{t("settings.circuit.recovery")}</Label>
              <span className="text-sm text-muted-foreground w-16 text-right">
                {s.circuit_recovery_secs}s
              </span>
            </div>
            <Slider
              min={300}
              max={1800}
              step={30}
              value={s.circuit_recovery_secs}
              onValueChange={(val) => update("circuit_recovery_secs", val)}
            />
            <p className="text-xs text-muted-foreground">300s – 1800s</p>
          </div>
          <div className="space-y-2">
            <Label>{t("settings.circuit.disableCodes")}</Label>
            <Input
              value={s.circuit_disable_codes}
              onChange={(e) => update("circuit_disable_codes", e.target.value)}
            />
            <p className="text-xs text-muted-foreground">{t("settings.circuit.disableDesc")}</p>
          </div>
        </CardContent>
      </Card>

      {/* System Tray */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("settings.tray.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <Label>{t("settings.tray.autostart")}</Label>
            <Switch
              checked={s.autostart}
              onCheckedChange={(v) => update("autostart", v)}
            />
          </div>
          <div className="flex items-center justify-between">
            <Label>{t("settings.tray.startMinimized")}</Label>
            <Switch
              checked={s.start_minimized}
              onCheckedChange={(v) => update("start_minimized", v)}
            />
          </div>
        </CardContent>
      </Card>

      {/* General */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">{t("settings.general.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <Label>{t("settings.general.language")}</Label>
            <Select value={s.locale} onValueChange={changeLocale}>
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="zh">中文</SelectItem>
                <SelectItem value="en">English</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center justify-between">
            <Label>{t("settings.general.theme")}</Label>
            <Select
              value={s.theme}
              onValueChange={(v) => update("theme", v)}
            >
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="light">{t("settings.general.themeLight")}</SelectItem>
                <SelectItem value="dark">{t("settings.general.themeDark")}</SelectItem>
                <SelectItem value="system">{t("settings.general.themeSystem")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
