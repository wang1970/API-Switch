import { useState, useRef, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { getProxyStatus } from "@/lib/api";
import { Send, Loader2, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { ApiEntry } from "@/types";

interface Message {
  role: "user" | "assistant";
  content: string;
  connect_ms?: number;
  think_ms?: number;
  usage?: { prompt_tokens: number; completion_tokens: number; total_tokens: number };
}

interface TestChatDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  entry: ApiEntry | null;
}

export function TestChatDialog({ open, onOpenChange, entry }: TestChatDialogProps) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [port, setPort] = useState(9090);
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (open && entry) {
      setMessages([]);
      setInput("");
      setError(null);
      getProxyStatus().then((status) => {
        if (status?.port) setPort(status.port);
      });
    }
  }, [open, entry]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  useEffect(() => {
    return () => { abortRef.current?.abort(); };
  }, []);

  const sendMessage = useCallback(async () => {
    const text = input.trim();
    if (!text || loading || !entry) return;

    setError(null);
    const userMessage: Message = { role: "user", content: text };
    const newMessages = [...messages, userMessage];
    setMessages(newMessages);
    setInput("");
    setLoading(true);

    const abortController = new AbortController();
    abortRef.current = abortController;

    const start = performance.now();
    let firstChunkTime = 0;
    let connect_ms = 0;
    let think_ms = 0;
    let prompt_tokens = 0;
    let completion_tokens = 0;

    try {
      const response = await fetch(`http://127.0.0.1:${port}/v1/chat/completions`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          model: entry.model,
          messages: newMessages.map((m) => ({ role: m.role, content: m.content })),
          stream: true,
        }),
        signal: abortController.signal,
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`HTTP ${response.status}: ${errorText}`);
      }

      const reader = response.body!.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      let fullContent = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed || !trimmed.startsWith("data: ")) continue;
          const payload = trimmed.slice(6);
          if (payload === "[DONE]") continue;

          try {
            const parsed = JSON.parse(payload);
            const delta = parsed.choices?.[0]?.delta?.content;
            if (delta) {
              if (firstChunkTime === 0) {
                firstChunkTime = performance.now();
                connect_ms = Math.round(firstChunkTime - start);
              }
              fullContent += delta;

              // Extract usage if present in stream
              if (parsed.usage) {
                prompt_tokens = parsed.usage.prompt_tokens || 0;
                completion_tokens = parsed.usage.completion_tokens || 0;
              }
            }
          } catch {
            // Skip malformed JSON
          }
        }
      }

      const endTime = performance.now();
      think_ms = Math.round(endTime - firstChunkTime);

      if (!abortController.signal.aborted) {
        setMessages([...newMessages, {
          role: "assistant",
          content: fullContent,
          connect_ms,
          think_ms,
          usage: prompt_tokens + completion_tokens > 0
            ? { prompt_tokens, completion_tokens, total_tokens: prompt_tokens + completion_tokens }
            : undefined,
        }]);
      }
    } catch (err: unknown) {
      if (err instanceof Error && err.name === "AbortError") return;
      if (!abortController.signal.aborted) {
        setError(err instanceof Error ? err.message : String(err));
        setMessages(newMessages);
      }
    } finally {
      if (!abortController.signal.aborted) {
        setLoading(false);
      }
      abortRef.current = null;
    }
  }, [input, loading, entry, messages, port]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const clearMessages = () => {
    abortRef.current?.abort();
    setMessages([]);
    setError(null);
    setLoading(false);
  };

  const handleClose = (v: boolean) => {
    abortRef.current?.abort();
    onOpenChange(v);
  };

  const formatMs = (ms: number) => {
    if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
    return `${ms}ms`;
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="flex flex-col sm:max-w-2xl h-[70vh]">
        <DialogHeader>
          <DialogTitle>
            {t("apiPool.testChat.title")} — {entry?.display_name || entry?.model}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {entry?.channel_name} / {entry?.model}
          </DialogDescription>
        </DialogHeader>

        {/* Messages area */}
        <div className="flex-1 min-h-0 rounded-md border bg-muted/30">
          <ScrollArea className="h-full">
            <div ref={scrollRef} className="p-4 space-y-3">
              {messages.length === 0 && (
                <div className="flex items-center justify-center h-32 text-sm text-muted-foreground">
                  {t("apiPool.testChat.placeholder")}
                </div>
              )}

              {messages.map((msg, idx) => (
                <div
                  key={idx}
                  className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
                >
                  <div
                    className={`max-w-[80%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap break-words ${
                      msg.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted"
                    }`}
                  >
                    {msg.content}
                    {msg.role === "assistant" && msg.connect_ms != null && (
                      <div className="mt-1 pt-1 border-t border-border text-[10px] text-muted-foreground">
                        <span title="连接时间 (TTFB)">🔗 {formatMs(msg.connect_ms)}</span>
                        <span className="mx-1.5">+</span>
                        <span title="思考/生成时间">💭 {formatMs(msg.think_ms || 0)}</span>
                        {msg.usage && (
                          <span className="ml-2">
                            IN:{msg.usage.prompt_tokens}+OUT:{msg.usage.completion_tokens}
                          </span>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              ))}

              {loading && (
                <div className="flex justify-start">
                  <div className="rounded-lg px-3 py-2 bg-muted">
                    <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                  </div>
                </div>
              )}

              {error && (
                <div className="rounded-lg border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                  {error}
                </div>
              )}
            </div>
          </ScrollArea>
        </div>

        {/* Input area */}
        <div className="flex items-end gap-2 pt-2">
          <textarea
            className="flex-1 resize-none rounded-md border bg-background px-3 py-2 text-sm min-h-[38px] max-h-[120px] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            placeholder={t("apiPool.testChat.inputPlaceholder")}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={loading}
            rows={1}
            onInput={(e) => {
              const target = e.target as HTMLTextAreaElement;
              target.style.height = "auto";
              target.style.height = Math.min(target.scrollHeight, 120) + "px";
            }}
          />
          {messages.length > 0 && (
            <Button
              variant="ghost"
              size="icon"
              onClick={clearMessages}
              disabled={loading}
              title={t("common.delete")}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          )}
          <Button
            size="icon"
            onClick={sendMessage}
            disabled={loading || !input.trim()}
          >
            {loading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Send className="h-4 w-4" />
            )}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
