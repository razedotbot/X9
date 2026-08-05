export class RazeError extends Error {
  constructor(
    message: string,
    public status: number,
    public code: string,
    public body?: unknown,
  ) {
    super(message);
    this.name = "RazeError";
  }
}

export class AuthError extends RazeError {
  constructor(message: string, body?: unknown) {
    super(message, 401, "AUTH_ERROR", body);
    this.name = "AuthError";
  }
}

export class ValidationError extends RazeError {
  constructor(message: string, status: number, body?: unknown) {
    super(message, status, "VALIDATION_ERROR", body);
    this.name = "ValidationError";
  }
}

export class RateLimitError extends RazeError {
  constructor(
    message: string,
    public retryAfter?: number,
    body?: unknown,
  ) {
    super(message, 429, "RATE_LIMIT", body);
    this.name = "RateLimitError";
  }
}

export class ServerError extends RazeError {
  constructor(message: string, status: number, body?: unknown) {
    super(message, status, "SERVER_ERROR", body);
    this.name = "ServerError";
  }
}

interface HttpConfig {
  baseUrl: string;
  apiKey: string;
  timeout: number;
  maxRetries: number;
}

export class Http {
  private base: string;
  private headers: Record<string, string>;
  private timeout: number;
  private maxRetries: number;

  constructor(cfg: HttpConfig) {
    this.base = cfg.baseUrl.replace(/\/+$/, "");
    this.timeout = cfg.timeout;
    this.maxRetries = cfg.maxRetries;
    this.headers = {
      "content-type": "application/json",
      "user-agent": "raze-trading-ts/0.3.0",
    };
    // Route the credential to the header the router reads for its kind.
    if (cfg.apiKey) {
      if (cfg.apiKey.startsWith("sk_") || cfg.apiKey.startsWith("eyJ")) {
        this.headers["authorization"] = `Bearer ${cfg.apiKey}`;
      } else {
        // A license key lands here: x-api-key is checked first.
        this.headers["x-api-key"] = cfg.apiKey;
      }
    }
  }

  async get<T>(path: string, params?: Record<string, string>): Promise<T> {
    let url = this.base + path;
    if (params) {
      const qs = new URLSearchParams();
      for (const [k, v] of Object.entries(params)) {
        if (v !== undefined && v !== null) qs.set(k, v);
      }
      const str = qs.toString();
      if (str) url += "?" + str;
    }
    return this.request<T>("GET", url);
  }

  async post<T>(path: string, body: unknown): Promise<T> {
    return this.request<T>("POST", this.base + path, body);
  }

  private async request<T>(method: string, url: string, body?: unknown): Promise<T> {
    let lastErr: Error | undefined;

    for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
      if (attempt > 0) {
        await sleep(200 * Math.pow(2, attempt - 1));
      }

      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), this.timeout);

      let res: Response;
      try {
        res = await fetch(url, {
          method,
          headers: this.headers,
          body: body ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });
      } catch (err) {
        clearTimeout(timer);
        lastErr = err instanceof Error ? err : new Error(String(err));
        continue;
      } finally {
        clearTimeout(timer);
      }

      if (res.ok) {
        return (await res.json()) as T;
      }

      const text = await res.text().catch(() => "");
      let parsed: unknown;
      try { parsed = JSON.parse(text); } catch { parsed = text; }

      if (res.status === 401) {
        throw new AuthError(extractError(parsed) || "Unauthorized", parsed);
      }
      if (res.status === 400 || res.status === 422) {
        throw new ValidationError(extractError(parsed) || `Validation error (${res.status})`, res.status, parsed);
      }
      if (res.status === 429) {
        const retry = res.headers.get("retry-after");
        const ra = retry !== null && retry !== "" && !Number.isNaN(Number(retry))
          ? Number(retry)
          : undefined;
        throw new RateLimitError(extractError(parsed) || "Rate limit exceeded", ra, parsed);
      }
      if (res.status >= 500) {
        lastErr = new ServerError(extractError(parsed) || `Server error (${res.status})`, res.status, parsed);
        continue;
      }

      throw new RazeError(extractError(parsed) || `HTTP ${res.status}`, res.status, "HTTP_ERROR", parsed);
    }

    throw lastErr ?? new RazeError("Request failed", 0, "UNKNOWN");
  }
}

function extractError(body: unknown): string | undefined {
  if (body && typeof body === "object" && "error" in body) {
    return String((body as Record<string, unknown>).error);
  }
  if (typeof body === "string" && body.length < 500) return body;
  return undefined;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
