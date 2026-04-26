import { useState, Fragment, useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { getUsageLogs } from "@/lib/api";
import type { UsageLogFilter } from "@/types";

export function LogPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<UsageLogFilter>({ page: 1, page_size: 100 });
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  // Real-time log push
  useEffect(() => {
    const unlisten = listen("new-usage-log", () => {
      queryClient.invalidateQueries({ queryKey: ["usageLogs"] });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  const { data: result, isLoading } = useQuery({
    queryKey: ["usageLogs", filter],
    queryFn: () => getUsageLogs(filter),
  });

  const logs = result?.items || [];
  const totalPrompt = logs.reduce((sum, log) => sum + log.prompt_tokens, 0);
  const totalCompletion = logs.reduce((sum, log) => sum + log.completion_tokens, 0);
  const successCount = logs.filter((log) => log.success).length;

  const toggleErrorsOnly = (checked: boolean) => {
    setErrorsOnly(checked);
    setFilter((f) => ({
      ...f,
      success: checked ? false : undefined,
      page: 1,
    }));
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("log.title")}</h1>
        <div className="flex items-center gap-2 text-sm">
          <span className={errorsOnly ? "text-red-500" : "text-muted-foreground"}>
            {t("log.all")}
          </span>
          <Switch checked={errorsOnly} onCheckedChange={toggleErrorsOnly} />
          <span className={!errorsOnly ? "text-muted-foreground" : "text-red-500"}>
            {t("log.failed")}
          </span>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-4 mb-4">
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.recentLogs")}</div>
            <div className="text-2xl font-semibold mt-1">{logs.length}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.promptTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{totalPrompt}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.completionTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{totalCompletion}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.successRate")}</div>
            <div className="text-2xl font-semibold mt-1">
              {logs.length ? `${((successCount / logs.length) * 100).toFixed(1)}%` : "0%"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Table */}
      <div className="rounded-md border overflow-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b bg-muted/50">
              <th className="px-3 py-2 text-left font-medium">{t("log.time")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.channel")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.token")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.model")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.duration")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.promptTokens")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.completionTokens")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.status")}</th>
            </tr>
          </thead>
          <tbody>
            {logs.map((log) => {
              const isExpanded = expandedId === log.id;
              return (
                <Fragment key={log.id}>
                  <tr
                    className="border-b hover:bg-muted/30 cursor-pointer"
                    onClick={() => setExpandedId(isExpanded ? null : log.id)}
                  >
                    <td className="px-3 py-2 whitespace-nowrap">
                      <div>{new Date(log.created_at * 1000).toLocaleString()}</div>
                    </td>
                    <td className="px-3 py-2">
                      <div>{log.channel_name}</div>
                    </td>
                    <td className="px-3 py-2">
                      <div>{log.token_name || log.access_key_name || <span className="text-muted-foreground">-</span>}</div>
                    </td>
                    <td className="px-3 py-2 font-mono text-xs">
                      <div>
                        {log.requested_model === "auto"
                          ? `(auto)${log.model}`
                          : log.model}
                      </div>
                    </td>
                    <td className="px-3 py-2 whitespace-nowrap">
                      <div>{`${log.use_time || Math.ceil(log.latency_ms / 1000)}s${log.is_stream && log.first_token_ms > 0 ? ` / ${(log.first_token_ms / 1000).toFixed(1)}s` : ""}  ${log.is_stream ? t("log.streamShort") : t("log.nonStreamShort")}`}</div>
                    </td>
                    <td className="px-3 py-2 text-right">{log.prompt_tokens}</td>
                    <td className="px-3 py-2 text-right">{log.completion_tokens}</td>
                    <td className="px-3 py-2">
                      <span className={log.success ? "text-green-600" : "text-red-500"}>
                        {log.success ? t("log.success") : t("log.failed")}
                      </span>
                    </td>
                  </tr>
                  {isExpanded ? (
                    <tr className="border-b bg-muted/20">
                      <td colSpan={8} className="px-4 py-3">
                        <div className="space-y-2 text-xs max-w-3xl">
                          {log.other ? (
                            <div>
                              <div className="font-medium text-muted-foreground mb-1">Meta</div>
                              <pre className="whitespace-pre-wrap break-all text-muted-foreground">{log.other}</pre>
                            </div>
                          ) : null}
                          {log.content ? (
                            <div>
                              <div className="font-medium text-muted-foreground mb-1">{t("log.details")}</div>
                              <pre className="whitespace-pre-wrap break-all">{log.content}</pre>
                            </div>
                          ) : null}
                          {log.error_message ? (
                            <div>
                              <div className="font-medium text-red-500 mb-1">{t("log.error")}</div>
                              <pre className="whitespace-pre-wrap break-all text-red-500">{log.error_message}</pre>
                            </div>
                          ) : null}
                          {!log.content && !log.error_message && !log.other ? (
                            <span className="text-muted-foreground">{t("log.noError")}</span>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>

      {!logs.length && !isLoading && (
        <div className="flex h-32 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}
    </div>
  );
}
