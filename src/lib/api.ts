import { invoke } from "@tauri-apps/api/core";
import type {
  Channel,
  CreateChannelParams,
  UpdateChannelParams,
  ModelInfo,
  ApiEntry,
  CreateEntryParams,
  AccessKey,
  UsageLog,
  UsageLogFilter,
  PaginatedResult,
  DashboardStats,
  ModelRanking,
  UserRanking,
  ChartDataPoint,
  DashboardFilter,
  AppSettings,
  ProxyStatus,
} from "../types";

// --- Channel ---

export async function listChannels(): Promise<Channel[]> {
  return invoke("list_channels");
}

export async function createChannel(params: CreateChannelParams): Promise<Channel> {
  return invoke("create_channel", { params });
}

export async function updateChannel(params: UpdateChannelParams): Promise<Channel> {
  return invoke("update_channel", { params });
}

export async function deleteChannel(id: string): Promise<void> {
  return invoke("delete_channel", { id });
}

export async function fetchModels(channelId: string): Promise<ModelInfo[]> {
  return invoke("fetch_models", { channelId });
}

export async function fetchModelsDirect(apiType: string, baseUrl: string, apiKey: string): Promise<ModelInfo[]> {
  return invoke("fetch_models_direct", { apiType, baseUrl, apiKey });
}

export async function selectModels(channelId: string, modelNames: string[]): Promise<void> {
  return invoke("select_models", { channelId, modelNames });
}

// --- API Pool ---

export async function listEntries(): Promise<ApiEntry[]> {
  return invoke("list_entries");
}

export async function toggleEntry(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_entry", { id, enabled });
}

export async function reorderEntries(orderedIds: string[]): Promise<void> {
  return invoke("reorder_entries", { orderedIds });
}

export async function createEntry(params: CreateEntryParams): Promise<ApiEntry> {
  return invoke("create_entry", { params });
}

// --- Access Keys ---

export async function listAccessKeys(): Promise<AccessKey[]> {
  return invoke("list_access_keys");
}

export async function createAccessKey(name: string): Promise<AccessKey> {
  return invoke("create_access_key", { name });
}

export async function deleteAccessKey(id: string): Promise<void> {
  return invoke("delete_access_key", { id });
}

export async function toggleAccessKey(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_access_key", { id, enabled });
}

// --- Usage ---

export async function getUsageLogs(
  filter: UsageLogFilter
): Promise<PaginatedResult<UsageLog>> {
  return invoke("get_usage_logs", { filter });
}

export async function getDashboardStats(filter?: DashboardFilter): Promise<DashboardStats> {
  return invoke("get_dashboard_stats", { filter });
}

export async function getModelConsumption(
  filter?: DashboardFilter
): Promise<ChartDataPoint[]> {
  return invoke("get_model_consumption", { filter });
}

export async function getCallTrend(
  filter?: DashboardFilter
): Promise<ChartDataPoint[]> {
  return invoke("get_call_trend", { filter });
}

export async function getModelDistribution(
  filter?: DashboardFilter
): Promise<ModelRanking[]> {
  return invoke("get_model_distribution", { filter });
}

export async function getModelRanking(
  filter?: DashboardFilter
): Promise<ModelRanking[]> {
  return invoke("get_model_ranking", { filter });
}

export async function getUserRanking(
  filter?: DashboardFilter
): Promise<UserRanking[]> {
  return invoke("get_user_ranking", { filter });
}

export async function getUserTrend(
  filter?: DashboardFilter
): Promise<ChartDataPoint[]> {
  return invoke("get_user_trend", { filter });
}

// --- Config ---

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function updateSettings(settings: Partial<AppSettings>): Promise<void> {
  return invoke("update_settings", { settings });
}

// --- Proxy ---

export async function startProxy(): Promise<ProxyStatus> {
  return invoke("start_proxy");
}

export async function stopProxy(): Promise<void> {
  return invoke("stop_proxy");
}

export async function getProxyStatus(): Promise<ProxyStatus> {
  return invoke("get_proxy_status");
}

// --- Update ---

export interface UpdateInfo {
  current: string;
  latest: string;
  url: string;
}

export async function checkUpdate(): Promise<UpdateInfo | null> {
  return invoke("check_update");
}

// --- Tray ---

export async function refreshTrayMenu(): Promise<void> {
  return invoke("refresh_tray_menu");
}

// --- Test Chat ---

export interface TestChatResponse {
  content: string;
  latency_ms: number;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

export async function testChat(
  entryId: string,
  messages: { role: string; content: string }[]
): Promise<TestChatResponse> {
  return invoke("test_chat", { entryId, messages });
}

// --- API Type Detect ---

export interface DetectApiResult {
  detected_type: string | null;
  models: ModelInfo[];
  message: string;
}

export async function detectApiType(baseUrl: string, apiKey: string): Promise<DetectApiResult> {
  return invoke("detect_api_type", { baseUrl, apiKey });
}

// --- URL Probe ---

export interface ProbeResult {
  reachable: boolean;
  status_code: number | null;
  latency_ms: number;
  detected_type: string | null;
  message: string;
}

export async function probeUrl(url: string): Promise<ProbeResult> {
  return invoke("probe_url", { url });
}
