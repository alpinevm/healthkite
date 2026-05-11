import assert from "node:assert/strict";
import test from "node:test";
import {
  BadDateError,
  WirebodyClient,
  NotFoundError,
  UnauthorizedError,
} from "../src/client.ts";

test("client.status() calls GET / with bearer header when token is set and omits it when unset", async () => {
  const authorizedCalls: RequestCall[] = [];
  const authorizedClient = new WirebodyClient({
    baseUrl: "http://192.168.1.244:5606",
    token: "abc123",
    fetch: mockFetch(authorizedCalls, {
      name: "Wirebody",
      sampleEncoding: "columnar-v1",
      version: "1.0",
      workoutCount: 3,
    }),
  });

  const status = await authorizedClient.status();

  assert.equal(status.name, "Wirebody");
  assert.equal(authorizedCalls[0]?.url, "http://192.168.1.244:5606/");
  assert.equal(authorizedCalls[0]?.headers.Authorization, "Bearer abc123");

  const anonymousCalls: RequestCall[] = [];
  const anonymousClient = new WirebodyClient({
    baseUrl: "http://192.168.1.244:5606",
    fetch: mockFetch(anonymousCalls, {
      name: "Wirebody",
      sampleEncoding: "columnar-v1",
      version: "1.0",
      workoutCount: 3,
    }),
  });

  await anonymousClient.status();

  assert.equal(anonymousCalls[0]?.url, "http://192.168.1.244:5606/");
  assert.equal(anonymousCalls[0]?.headers.Authorization, undefined);
});

test("client.listWorkouts() builds the expected query string", async () => {
  const calls: RequestCall[] = [];
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    token: "token",
    fetch: mockFetch(calls, {
      workouts: [],
      limit: 10,
      offset: 0,
      total: 0,
      hasMore: false,
    }),
  });

  const page = await client.listWorkouts({ limit: 10, offset: 0 });

  assert.equal(calls[0]?.url, "http://phone.local:5606/workouts?limit=10&offset=0");
  assert.equal(page.limit, 10);
  assert.equal(page.offset, 0);
  assert.equal(page.hasMore, false);
});

test("client.getWorkout() returns the response body verbatim", async () => {
  const body = `{"uuid":"UUID-X","sampleEncoding":"columnar-v1","samples":{}}`;
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch([], body),
  });

  const result = await client.getWorkout("UUID-X");

  assert.equal(result, body);
});

test("client.listQuantityTypes() calls GET /quantity-types", async () => {
  const calls: RequestCall[] = [];
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    token: "token",
    fetch: mockFetch(calls, {
      types: [
        {
          identifier: "HKQuantityTypeIdentifierHeartRate",
          category: "heart",
          preferredUnit: "count/min",
          aggregationStyle: "discrete",
          permissionStatus: "granted",
          sampleCount: 2,
          firstSampleDate: "2026-05-04T00:00:00Z",
          lastSampleDate: "2026-05-04T00:01:00Z",
        },
      ],
    }),
  });

  const response = await client.listQuantityTypes();

  assert.equal(calls[0]?.url, "http://phone.local:5606/quantity-types");
  assert.equal(calls[0]?.headers.Authorization, "Bearer token");
  assert.equal(response.types[0]?.identifier, "HKQuantityTypeIdentifierHeartRate");
});

test("client.getQuantitySeries() passes query parameters", async () => {
  const calls: RequestCall[] = [];
  const body = `{"type":"HKQuantityTypeIdentifierHeartRate","sampleEncoding":"columnar-v1","samples":{"t":[],"v":[]}}`;
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch(calls, body),
  });

  const response = await client.getQuantitySeries({
    type: "HKQuantityTypeIdentifierHeartRate",
    from: "2026-05-04T00:00:00Z",
    to: "2026-05-11T00:00:00Z",
    limit: 100,
    offset: 50,
  });

  assert.equal(
    calls[0]?.url,
    "http://phone.local:5606/quantity/HKQuantityTypeIdentifierHeartRate?from=2026-05-04T00%3A00%3A00Z&to=2026-05-11T00%3A00%3A00Z&limit=100&offset=50",
  );
  assert.equal(response, body);
});

test("client.listSleepSessions() calls GET /sleep with query parameters", async () => {
  const calls: RequestCall[] = [];
  const body = `{"sampleEncoding":"columnar-v1","sessions":[],"total":0,"hasMore":false}`;
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch(calls, body),
  });

  const response = await client.listSleepSessions({
    from: "2026-04-11T00:00:00Z",
    to: "2026-05-11T00:00:00Z",
    limit: 30,
    offset: 5,
  });

  assert.equal(
    calls[0]?.url,
    "http://phone.local:5606/sleep?from=2026-04-11T00%3A00%3A00Z&to=2026-05-11T00%3A00%3A00Z&limit=30&offset=5",
  );
  assert.equal(response, body);
});

test("client.listSleepSessions() omits defaults when no params are provided", async () => {
  const calls: RequestCall[] = [];
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch(calls, `{"sessions":[]}`),
  });

  await client.listSleepSessions();

  assert.equal(calls[0]?.url, "http://phone.local:5606/sleep");
});

test("client.getDaySnapshot() calls GET /day-snapshot/{date}", async () => {
  const calls: RequestCall[] = [];
  const body = `{"date":"2026-05-11","sampleEncoding":"columnar-v1","sleep":[]}`;
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch(calls, body),
  });

  const response = await client.getDaySnapshot({ date: "2026-05-11" });

  assert.equal(calls[0]?.url, "http://phone.local:5606/day-snapshot/2026-05-11");
  assert.equal(response, body);
});

test("client.getDaySnapshot() validates bad dates", async () => {
  const calls: RequestCall[] = [];
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch(calls, `{"error":"bad_date"}`),
  });

  await assert.rejects(
    () => client.getDaySnapshot({ date: "May 11" }),
    (error) => error instanceof BadDateError && error.code === "BadDate",
  );
  assert.equal(calls.length, 0);
});

test("client maps 401 responses to UnauthorizedError", async () => {
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch([], { error: "unauthorized" }, { status: 401 }),
  });

  await assert.rejects(
    () => client.status(),
    (error) => error instanceof UnauthorizedError && error.code === "Unauthorized",
  );
});

test("client maps 404 responses to NotFoundError", async () => {
  const client = new WirebodyClient({
    baseUrl: "http://phone.local:5606",
    fetch: mockFetch([], { error: "not_found" }, { status: 404 }),
  });

  await assert.rejects(
    () => client.getWorkout("UUID-X"),
    (error) =>
      error instanceof NotFoundError &&
      error.code === "NotFound" &&
      error.message.includes("UUID-X"),
  );
});

interface RequestCall {
  url: string;
  headers: Record<string, string>;
}

function mockFetch(
  calls: RequestCall[],
  body: unknown,
  init: ResponseInit = { status: 200 },
): typeof fetch {
  return async (input, requestInit) => {
    const headers = requestInit?.headers as Record<string, string> | undefined;
    calls.push({
      url: String(input),
      headers: headers ?? {},
    });

    return new Response(
      typeof body === "string" ? body : JSON.stringify(body),
      {
        status: init.status ?? 200,
        statusText: init.statusText,
        headers: {
          "content-type": "application/json",
          ...init.headers,
        },
      },
    );
  };
}
