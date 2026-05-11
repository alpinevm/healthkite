export interface WirebodyStatus {
  name: string;
  sampleEncoding: string;
  version: string;
  workoutCount: number;
}

export interface WorkoutSummary {
  uuid: string;
  workoutActivityType: string;
  startDate: string;
  endDate: string;
  duration: number;
  totalEnergyBurned: Quantity | null;
  totalDistance: Quantity | null;
  averageHeartRate: Quantity | null;
  sourceRevision: unknown;
  device: unknown;
  metadata: Record<string, unknown>;
}

export interface Quantity {
  value: number;
  unit: string;
}

export interface PaginatedWorkoutsResponse {
  workouts: WorkoutSummary[];
  limit: number;
  offset: number;
  total: number;
  hasMore: boolean;
}

export interface QuantityTypeInfo {
  identifier: string;
  category: string;
  preferredUnit: string;
  aggregationStyle: "cumulative" | "discrete" | string;
  permissionStatus: "granted" | "denied" | "notDetermined" | "unknown" | string;
  sampleCount: number;
  firstSampleDate: string | null;
  lastSampleDate: string | null;
}

export interface QuantityTypesResponse {
  types: QuantityTypeInfo[];
}

export interface QuantitySeriesOptions {
  type: string;
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export interface SleepSessionsOptions {
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export interface DaySnapshotOptions {
  date: string;
}

export interface WirebodyClientOptions {
  baseUrl: string;
  token?: string;
  fetch?: typeof fetch;
}

export type WirebodyErrorCode =
  | "Unauthorized"
  | "NotFound"
  | "BadDate"
  | "Unreachable"
  | "ServerError"
  | "ConfigError";

export class WirebodyError extends Error {
  readonly code: WirebodyErrorCode;
  readonly status?: number;

  constructor(code: WirebodyErrorCode, message: string, status?: number) {
    super(message);
    this.name = `${code}Error`;
    this.code = code;
    this.status = status;
  }
}

export class UnauthorizedError extends WirebodyError {
  constructor() {
    super("Unauthorized", "WIREBODY_TOKEN is missing or wrong", 401);
  }
}

export class NotFoundError extends WirebodyError {
  constructor(message: string) {
    super("NotFound", message, 404);
  }
}

export class BadDateError extends WirebodyError {
  constructor(date: string) {
    super("BadDate", `Invalid day snapshot date: ${date}. Expected YYYY-MM-DD.`, 400);
  }
}

export class UnreachableError extends WirebodyError {
  constructor(baseUrl: string) {
    super(
      "Unreachable",
      `Cannot reach Wirebody at ${baseUrl}. Is the iOS app foregrounded and the LAN server enabled?`,
    );
  }
}

export class ServerError extends WirebodyError {
  constructor(message: string, status: number) {
    super("ServerError", message, status);
  }
}

export class ConfigError extends WirebodyError {
  constructor(message: string) {
    super("ConfigError", message);
  }
}

export class WirebodyClient {
  private readonly baseUrl: URL;
  private readonly token?: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: WirebodyClientOptions) {
    this.baseUrl = parseBaseUrl(options.baseUrl);
    this.token = options.token;
    this.fetchImpl = options.fetch ?? globalThis.fetch;
  }

  static fromEnv(env: NodeJS.ProcessEnv = process.env): WirebodyClient {
    const baseUrl = env.WIREBODY_URL;
    if (!baseUrl) {
      throw new ConfigError("WIREBODY_URL is required");
    }

    return new WirebodyClient({
      baseUrl,
      token: env.WIREBODY_TOKEN,
    });
  }

  async status(): Promise<WirebodyStatus> {
    return this.requestJson<WirebodyStatus>("/", "status");
  }

  async listWorkouts(
    options: { limit?: number; offset?: number } = {},
  ): Promise<PaginatedWorkoutsResponse> {
    const limit = options.limit ?? 50;
    const offset = options.offset ?? 0;
    return this.requestJson<PaginatedWorkoutsResponse>(
      `/workouts?limit=${encodeURIComponent(limit)}&offset=${encodeURIComponent(offset)}`,
      "workouts list",
    );
  }

  async getWorkout(uuid: string): Promise<string> {
    return this.requestText(`/workouts/${encodeURIComponent(uuid)}`, `workout ${uuid}`);
  }

  async listQuantityTypes(): Promise<QuantityTypesResponse> {
    return this.requestJson<QuantityTypesResponse>("/quantity-types", "quantity types");
  }

  async getQuantitySeries(options: QuantitySeriesOptions): Promise<string> {
    const params = new URLSearchParams();
    if (options.from) {
      params.set("from", options.from);
    }
    if (options.to) {
      params.set("to", options.to);
    }
    if (options.limit !== undefined) {
      params.set("limit", String(options.limit));
    }
    if (options.offset !== undefined) {
      params.set("offset", String(options.offset));
    }

    const query = params.toString();
    const path = `/quantity/${encodeURIComponent(options.type)}${query ? `?${query}` : ""}`;
    return this.requestText(path, `quantity series ${options.type}`);
  }

  async listSleepSessions(options: SleepSessionsOptions = {}): Promise<string> {
    const params = new URLSearchParams();
    if (options.from) {
      params.set("from", options.from);
    }
    if (options.to) {
      params.set("to", options.to);
    }
    if (options.limit !== undefined) {
      params.set("limit", String(options.limit));
    }
    if (options.offset !== undefined) {
      params.set("offset", String(options.offset));
    }

    const query = params.toString();
    return this.requestText(`/sleep${query ? `?${query}` : ""}`, "sleep sessions");
  }

  async getDaySnapshot(options: DaySnapshotOptions): Promise<string> {
    if (!isDaySnapshotDate(options.date)) {
      throw new BadDateError(options.date);
    }

    return this.requestText(
      `/day-snapshot/${encodeURIComponent(options.date)}`,
      `day snapshot ${options.date}`,
    );
  }

  private async requestJson<T>(path: string, context: string): Promise<T> {
    const text = await this.requestText(path, context);
    return JSON.parse(text) as T;
  }

  private async requestText(path: string, context: string): Promise<string> {
    const url = new URL(path, this.baseUrl);
    const headers: Record<string, string> = {
      Accept: "application/json",
    };

    if (this.token) {
      headers.Authorization = `Bearer ${this.token}`;
    }

    let response: Response;
    try {
      response = await this.fetchImpl(url, { method: "GET", headers });
    } catch {
      throw new UnreachableError(this.baseUrl.toString());
    }

    const body = await response.text();
    if (response.ok) {
      return body;
    }

    if (response.status === 401) {
      throw new UnauthorizedError();
    }

    if (response.status === 404) {
      throw new NotFoundError(`${context} was not found`);
    }

    if (response.status === 400 && body.includes("bad_date")) {
      throw new BadDateError(context);
    }

    throw new ServerError(body || response.statusText || `HTTP ${response.status}`, response.status);
  }
}

function parseBaseUrl(value: string): URL {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new ConfigError(`WIREBODY_URL is invalid: ${value}`);
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new ConfigError("WIREBODY_URL must start with http:// or https://");
  }

  if (!url.pathname.endsWith("/")) {
    url.pathname = `${url.pathname}/`;
  }

  return url;
}

function isDaySnapshotDate(value: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}
