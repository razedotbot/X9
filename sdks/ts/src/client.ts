import { Http } from "./http.js";
import type {
  RazeTradingConfig, HealthResponse, QuoteOpts, QuoteResponse,
  BuyOpts, SellOpts, SwapResponse, InstructionsOpts, InstructionsResponse,
} from "./types.js";

export class RazeTrading {
  private http: Http;

  constructor(apiKey = "", config?: RazeTradingConfig) {
    this.http = new Http({
      baseUrl: config?.baseUrl ?? "http://localhost:8082",
      apiKey,
      timeout: config?.timeout ?? 30000,
      maxRetries: config?.maxRetries ?? 2,
    });
  }

  health(): Promise<HealthResponse> {
    return this.http.get<HealthResponse>("/health");
  }

  quote(mint: string, opts?: QuoteOpts): Promise<QuoteResponse> {
    const params: Record<string, string> = {};
    if (opts?.inputMint) params.inputMint = opts.inputMint;
    if (opts?.slippageBps !== undefined) params.slippageBps = String(opts.slippageBps);
    if (opts?.amount !== undefined) params.amount = String(opts.amount);
    return this.http.get<QuoteResponse>(`/swap/sol/quote/${mint}`, params);
  }

  /**
   * Answers without a credential, but an unauthenticated call falls back to the
   * compiled public fee tier — send your key.
   */
  buy(opts: BuyOpts): Promise<SwapResponse> {
    return this.http.post<SwapResponse>("/swap/sol/buy", opts);
  }

  /** Same fee-tier caveat as `buy` — send your key. */
  sell(opts: SellOpts): Promise<SwapResponse> {
    return this.http.post<SwapResponse>("/swap/sol/sell", opts);
  }

  instructions(opts: InstructionsOpts): Promise<InstructionsResponse> {
    return this.http.post<InstructionsResponse>("/swap/sol/instructions", opts);
  }
}
