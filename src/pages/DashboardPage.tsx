import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  getDashboardStats,
  getModelConsumption,
  getCallTrend,
  getModelDistribution,
  getUserTrend,
} from "@/lib/api";
import type { DashboardFilter } from "@/types";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  LineChart,
  Line,
} from "recharts";

const COLORS = [
  "#8884d8", "#82ca9d", "#ffc658", "#ff7300", "#0088fe",
  "#00C49F", "#FFBB28", "#FF8042", "#a855f7", "#ec4899",
];

type SeriesPoint = {
  time: string;
  [key: string]: string | number;
};

function buildSeriesData(
  items: Array<{ time: string; model: string; value: number }> | undefined,
  topN = 8,
): { data: SeriesPoint[]; series: string[] } {
  if (!items?.length) {
    return { data: [], series: [] };
  }

  const totals = new Map<string, number>();
  for (const item of items) {
    totals.set(item.model, (totals.get(item.model) ?? 0) + item.value);
  }

  const series = [...totals.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, topN)
    .map(([model]) => model);

  const allowed = new Set(series);
  const byTime = new Map<string, SeriesPoint>();

  for (const item of items) {
    const timeEntry = byTime.get(item.time) ?? { time: item.time };
    const key = allowed.has(item.model) ? item.model : "Other";
    const current = typeof timeEntry[key] === "number" ? Number(timeEntry[key]) : 0;
    timeEntry[key] = current + item.value;
    byTime.set(item.time, timeEntry);
  }

  const finalSeries = byTime.size && items.some((item) => !allowed.has(item.model))
    ? [...series, "Other"]
    : series;

  return {
    data: [...byTime.values()].sort((a, b) => String(a.time).localeCompare(String(b.time))),
    series: finalSeries,
  };
}

function StatCard({ title, value, totalLabel }: { title: string; value: number; totalLabel?: string }) {
  return (
    <Card>
      <CardContent className="p-4">
        <p className="text-sm text-muted-foreground">{title}</p>
        <p className="text-2xl font-bold mt-1">{value.toLocaleString()}</p>
        {totalLabel !== undefined && (
          <p className="text-xs text-muted-foreground mt-1">{totalLabel}</p>
        )}
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<DashboardFilter>({ granularity: "hour" });

  const { data: stats } = useQuery({
    queryKey: ["dashboardStats", filter],
    queryFn: () => getDashboardStats(filter),
  });

  const { data: consumption } = useQuery({
    queryKey: ["modelConsumption", filter],
    queryFn: () => getModelConsumption(filter),
  });

  const { data: callTrend } = useQuery({
    queryKey: ["callTrend", filter],
    queryFn: () => getCallTrend(filter),
  });

  const { data: distribution } = useQuery({
    queryKey: ["modelDistribution", filter],
    queryFn: () => getModelDistribution(filter),
  });

  const { data: userTrend } = useQuery({
    queryKey: ["userTrend", filter],
    queryFn: () => getUserTrend(filter),
  });

  const totalTokens = (stats?.total_prompt_tokens ?? 0) + (stats?.total_completion_tokens ?? 0);
  const todayTokens = (stats?.today_prompt_tokens ?? 0) + (stats?.today_completion_tokens ?? 0);
  const consumptionSeries = buildSeriesData(consumption);
  const callTrendSeries = buildSeriesData(callTrend);
  const userTrendSeries = buildSeriesData(userTrend, 6);

  const setTimeRange = (range: string) => {
    const now = Date.now() / 1000;
    let start: number;
    switch (range) {
      case "today":
        start = now - 86400;
        break;
      case "7d":
        start = now - 7 * 86400;
        break;
      case "30d":
        start = now - 30 * 86400;
        break;
      default:
        start = 0;
    }
    setFilter((prev) => ({ ...prev, start_time: start || undefined, end_time: undefined }));
  };

  return (
    <div className="p-6">
      <h1 className="text-xl font-semibold mb-6">{t("dashboard.title")}</h1>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 xl:grid-cols-4 gap-4 mb-6">
        <StatCard
          title={t("dashboard.cards.todayRequests")}
          value={stats?.today_requests ?? 0}
          totalLabel={`${t("dashboard.cards.total")}: ${(stats?.total_requests ?? 0).toLocaleString()}`}
        />
        <StatCard
          title={t("dashboard.cards.todayTokens")}
          value={todayTokens}
          totalLabel={`${t("dashboard.cards.total")}: ${totalTokens.toLocaleString()}`}
        />
        <StatCard
          title={t("dashboard.cards.todayPrompt")}
          value={stats?.today_prompt_tokens ?? 0}
          totalLabel={`${t("dashboard.cards.total")}: ${(stats?.total_prompt_tokens ?? 0).toLocaleString()}`}
        />
        <StatCard
          title={t("dashboard.cards.todayCompletion")}
          value={stats?.today_completion_tokens ?? 0}
          totalLabel={`${t("dashboard.cards.total")}: ${(stats?.total_completion_tokens ?? 0).toLocaleString()}`}
        />
      </div>

      <div className="grid gap-6">
        {/* Charts */}
        <Tabs defaultValue="consumption">
          <TabsList>
            <TabsTrigger value="consumption">{t("dashboard.charts.consumption")}</TabsTrigger>
            <TabsTrigger value="callTrend">{t("dashboard.charts.callTrend")}</TabsTrigger>
            <TabsTrigger value="distribution">{t("dashboard.charts.distribution")}</TabsTrigger>
            <TabsTrigger value="userTrend">{t("dashboard.charts.userTrend")}</TabsTrigger>
          </TabsList>

          <TabsContent value="consumption">
            <Card>
              <CardHeader className="pb-0">
                <div className="flex items-center justify-between gap-3">
                  <CardTitle>{t("dashboard.charts.consumption")}</CardTitle>
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <span>{t("dashboard.filter.hour")}</span>
                    <Switch
                      checked={filter.granularity === "day"}
                      onCheckedChange={(checked) =>
                        setFilter((prev) => ({
                          ...prev,
                          granularity: checked ? "day" : "hour",
                        }))
                      }
                    />
                    <span>{t("dashboard.filter.day")}</span>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <BarChart data={consumptionSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {consumptionSeries.series.map((series, index) => (
                      <Bar
                        key={series}
                        dataKey={series}
                        stackId="consumption"
                        fill={COLORS[index % COLORS.length]}
                      />
                    ))}
                  </BarChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="callTrend">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <LineChart data={callTrendSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {callTrendSeries.series.map((series, index) => (
                      <Line
                        key={series}
                        type="monotone"
                        dataKey={series}
                        stroke={COLORS[index % COLORS.length]}
                        strokeWidth={2}
                        dot={false}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="distribution">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <PieChart>
                    <Pie
                      data={distribution || []}
                      dataKey="count"
                      nameKey="model"
                      cx="50%"
                      cy="50%"
                      outerRadius={150}
                      label
                    >
                      {(distribution || []).map((_, index) => (
                        <Cell key={index} fill={COLORS[index % COLORS.length]} />
                      ))}
                    </Pie>
                    <Tooltip />
                    <Legend />
                  </PieChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="userTrend">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <LineChart data={userTrendSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {userTrendSeries.series.map((series, index) => (
                      <Line
                        key={series}
                        type="monotone"
                        dataKey={series}
                        stroke={COLORS[index % COLORS.length]}
                        strokeWidth={2}
                        dot={false}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
