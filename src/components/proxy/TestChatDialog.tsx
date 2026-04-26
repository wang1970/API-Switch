import { useState, useRef, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { testChat } from "@/lib/api";
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
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<boolean>(false);

  useEffect(() => {
    if (open && entry) {
      setMessages([]);
      setInput("");
      setError(null);
    }
  }, [open, entry]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  useEffect(() => {
    return () => { abortRef.current = true; };
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
    abortRef.current = false;

    const start = performance.now();

    try {
      const result = await testChat(
        entry.id,
        newMessages.map((m) => ({ role: m.role, content: m.content }))
      );

      if (abortRef.current) return;

      const connect_ms = Math.round(performance.now() - start);

      setMessages([...newMessages, {
        role: "assistant",
        content: result.content,
        connect_ms,
        think_ms: 0,
        usage: result.usage
          ? { prompt_tokens: result.usage.prompt_tokens, completion_tokens: result.usage.completion_tokens, total_tokens: result.usage.total_tokens }
          : undefined,
      }]);
    } catch (err: unknown) {
      if (abortRef.current) return;
      setError(err instanceof Error ? err.message : String(err));
      setMessages(newMessages);
    } finally {
      if (!abortRef.current) {
        setLoading(false);
      }
    }
  }, [input, loading, entry, messages]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const clearMessages = () => {
    abortRef.current = true;
    setMessages([]);
    setError(null);
    setLoading(false);
  };

  const handleClose = (v: boolean) => {
    abortRef.current = true;
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
