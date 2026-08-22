export interface FastiClientOptions {
  baseUrl: string;
}

export class FastiClient {
  private baseUrl: string;

  constructor(options: FastiClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, "");
  }

  async health(): Promise<{ status: string; version: string }> {
    const res = await fetch(`${this.baseUrl}/api/v1/health`);
    if (!res.ok) throw new Error(`Health check failed: ${res.status}`);
    return res.json();
  }
}
