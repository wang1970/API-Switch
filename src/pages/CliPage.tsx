import { useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Plug, Terminal } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import bundledCliData from "../../cli.json";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getSettings, getProxyStatus, listAccessKeys, setUserEnvVars, getCliData } from "@/lib/api";

export interface CliEnvItem {
  key: string;
  description: string;
  default?: string;
}

export interface CliItem {
  id: string;
  name: string;
  package: string;
  env: {
    minimal: CliEnvItem[];
    extended: CliEnvItem[];
  };
}

const BUNDLED = bundledCliData as CliItem[];

function defaultValue(envKey: string, baseUrl: string, accessKey: string) {
  if (envKey.includes("BASE_URL")) return baseUrl;
  if (envKey.includes("API_KEY")) return accessKey;
  if (envKey === "PROVIDER") return "anthropic";
  return "";
}

function EnvRow({
  item,
  value,
  onChange,
  accessKeyOptions,
}: {
  item: CliEnvItem;
  value: string;
  onChange: (value: string) => void;
  accessKeyOptions: string[];
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between gap-3">
        <code className="text-xs bg-muted px-1.5 py-0.5 rounded">{item.key}</code>
        <span className="text-xs text-muted-foreground text-right">{item.description}</span>
      </div>
      {item.key === "PROVIDER" ? (
        <Select value={value || "anthropic"} onValueChange={onChange}>
          <SelectTrigger className="h-8 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="anthropic">anthropic</SelectItem>
            <SelectItem value="openai">openai</SelectItem>
          </SelectContent>
        </Select>
      ) : item.key.includes("API_KEY") ? (
        <>
          <Input className="h-8 text-xs font-mono" value={value} onChange={(e) => onChange(e.target.value)} list={`${item.key}-options`} />
          <datalist id={`${item.key}-options`}>
            {accessKeyOptions.map((key) => (
              <option key={key} value={key} />
            ))}
          </datalist>
        </>
      ) : (
        <Input className="h-8 text-xs font-mono" value={value} onChange={(e) => onChange(e.target.value)} />
      )}
    </div>
  );
}

export function CliPage() {
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: getSettings });
  const { data: proxyStatus } = useQuery({ queryKey: ["proxyStatus"], queryFn: getProxyStatus, refetchInterval: 2000 });
  const { data: accessKeys } = useQuery({ queryKey: ["accessKeys"], queryFn: listAccessKeys });
  const { data: remoteCli } = useQuery({
    queryKey: ["cliData"],
    queryFn: getCliData,
    staleTime: 1000 * 60 * 60,
  });
  const cliItems: CliItem[] = useMemo(() => {
    if (remoteCli && Array.isArray(remoteCli) && remoteCli.length > 0) {
      return remoteCli as CliItem[];
    }
    return BUNDLED;
  }, [remoteCli]);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});

  const port = proxyStatus?.port || settings?.listen_port || 9090;
  const baseUrl = `http://127.0.0.1:${port}/v1`;
  const enabledKeys = useMemo(() => (accessKeys || []).filter((key) => key.enabled).map((key) => key.key), [accessKeys]);
  const fallbackKey = "auto";

  const getValue = (cli: CliItem, item: CliEnvItem) => {
    const id = `${cli.id}:${item.key}`;
    return values[id] ?? item.default ?? defaultValue(item.key, baseUrl, fallbackKey);
  };

  const setValue = (cli: CliItem, item: CliEnvItem, value: string) => {
    setValues((prev) => ({ ...prev, [`${cli.id}:${item.key}`]: value }));
  };

  const connect = async (cli: CliItem) => {
    const envItems = expanded[cli.id]
      ? [...cli.env.minimal, ...cli.env.extended]
      : cli.env.minimal;
    const vars = envItems
      .map((item) => ({ key: item.key, value: getValue(cli, item) }))
      .filter((item) => item.key && item.value);
    await setUserEnvVars(vars);
    toast.success(`${cli.name} 环境变量已写入系统`);
  };

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-semibold">连接 CLI</h1>
        <p className="text-sm text-muted-foreground mt-1">
          选择 CLI 后点击连接，将当前卡片中的环境变量写入系统用户环境变量。默认只显示最小配置；展开后可配置扩展变量。
        </p>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-4">
        {cliItems.map((cli) => {
          const isExpanded = !!expanded[cli.id];
          const visibleEnv = isExpanded ? [...cli.env.minimal, ...cli.env.extended] : cli.env.minimal;
          return (
            <Card key={cli.id}>
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <Terminal className="h-4 w-4 text-muted-foreground" />
                      <h2 className="font-semibold truncate">{cli.name}</h2>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1">{cli.package}</p>
                  </div>
                  <Button size="sm" onClick={() => connect(cli)}>
                    <Plug className="h-3.5 w-3.5" />
                    连接
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-3">
                  {visibleEnv.map((item) => (
                    <EnvRow
                      key={item.key}
                      item={item}
                      value={getValue(cli, item)}
                      onChange={(value) => setValue(cli, item, value)}
                      accessKeyOptions={enabledKeys.length ? enabledKeys : ["auto"]}
                    />
                  ))}
                </div>

                <div className="rounded-md border bg-muted/30 p-3 text-xs space-y-1">
                  <div><span className="text-muted-foreground">Base URL:</span> <code>{baseUrl}</code></div>
                  <div><span className="text-muted-foreground">API KEY:</span> <code>auto</code></div>
                  <div><span className="text-muted-foreground">model:</span> <code>auto</code></div>
                </div>

                {cli.env.extended.length ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full justify-center gap-1.5"
                    onClick={() => setExpanded((prev) => ({ ...prev, [cli.id]: !prev[cli.id] }))}
                  >
                    {isExpanded ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
                    {isExpanded ? "收起扩展配置" : "展开扩展配置"}
                  </Button>
                ) : null}
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
