import { FormEvent, useMemo, useState } from "react";

type ChatMessage = {
  role: "user" | "assistant";
  content: string;
};

type OpenAiResponse = {
  choices?: Array<{
    message?: {
      content?: string;
    };
  }>;
  error?: {
    message?: string;
  };
};

const baseUrl = import.meta.env.VITE_LLM_BASE_URL ?? "http://localhost:8000/v1";
const apiKey = import.meta.env.VITE_LLM_API_KEY ?? "local-key";
const model = import.meta.env.VITE_LLM_MODEL ?? "Qwen/Qwen2.5-7B-Instruct";

export default function App() {
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      role: "assistant",
      content: "Hello, I'm connected to Gaia. Ask your question.",
    },
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const endpoint = useMemo(
    () => `${String(baseUrl).replace(/\/$/, "")}/chat/completions`,
    []
  );

  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const content = input.trim();
    if (!content || loading) return;

    const nextMessages = [...messages, { role: "user" as const, content }];
    setMessages(nextMessages);
    setInput("");
    setError(null);
    setLoading(true);

    try {
      const response = await fetch(endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${apiKey}`,
        },
        body: JSON.stringify({
          model,
          messages: nextMessages,
          temperature: 0.7,
          stream: false,
        }),
      });

      const data = (await response.json()) as OpenAiResponse;
      if (!response.ok) {
        throw new Error(data.error?.message ?? "Unknown error");
      }

      const assistantText = data.choices?.[0]?.message?.content?.trim();
      if (!assistantText) {
        throw new Error("Empty model response");
      }

      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: assistantText,
        },
      ]);
    } catch (caught) {
      const message =
        caught instanceof Error ? caught.message : "Unexpected network error";
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="page">
      <section className="chat-card">
        <header className="chat-header">
          <h1>Gaia chatbot</h1>
          <p>
            Model: <strong>{model}</strong>
          </p>
        </header>

        <div className="messages">
          {messages.map((message, index) => (
            <article
              key={`${message.role}-${index}`}
              className={`bubble ${message.role}`}
            >
              <span className="role">{message.role}</span>
              <p>{message.content}</p>
            </article>
          ))}
          {loading && (
            <article className="bubble assistant">
              <span className="role">assistant</span>
              <p>Generating...</p>
            </article>
          )}
        </div>

        {error && <p className="error">{error}</p>}

        <form className="composer" onSubmit={onSubmit}>
          <input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder="Type your message..."
            disabled={loading}
          />
          <button type="submit" disabled={loading || !input.trim()}>
            {loading ? "Sending..." : "Send"}
          </button>
        </form>
      </section>
    </main>
  );
}
