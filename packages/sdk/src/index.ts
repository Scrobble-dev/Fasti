export interface FastiClientOptions {
  baseUrl: string;
  token?: string;
}

export interface ActivityEvent {
  event_id: string;
  schema_version: string;
  actor_id: string;
  device_id: string;
  device_seq: number;
  kind: string;
  media: {
    source: string;
    id: string;
    grain: string;
    title?: string;
  };
  progress?: {
    value: number;
    total?: number;
    unit: string;
  };
  timestamps: {
    occurred_at: string;
    observed_at: string;
    received_at: string;
  };
  provenance: {
    channel: string;
    client: string;
    external_event_id?: string;
  };
  correction_of?: string | null;
  tombstone_of?: string | null;
}

export interface EventReceipt {
  event_id: string;
  received_at: string;
  status: "committed" | "duplicate_ignored" | "correction_accepted";
}

export class FastiClient {
  private baseUrl: string;
  private token?: string;

  constructor(options: FastiClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, "");
    this.token = options.token;
  }

  async health(): Promise<{ status: string; version: string }> {
    const res = await fetch(`${this.baseUrl}/api/v1/health`);
    if (!res.ok) throw new Error(`Health check failed: ${res.status}`);
    return res.json();
  }

  async submitEvent(event: ActivityEvent): Promise<EventReceipt> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.token) {
      headers["Authorization"] = `Bearer ${this.token}`;
    }

    const res = await fetch(`${this.baseUrl}/api/v1/events`, {
      method: "POST",
      headers,
      body: JSON.stringify(event),
    });

    if (!res.ok) throw new Error(`Failed to submit event: ${res.status}`);
    return res.json();
  }
}
