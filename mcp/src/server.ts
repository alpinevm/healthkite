import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
  type CallToolResult,
  type Tool,
} from "@modelcontextprotocol/sdk/types.js";
import { WirebodyClient, WirebodyError } from "./client.js";

const tools: Tool[] = [
  {
    name: "status",
    description:
      "Get a quick status snapshot of the Wirebody iOS app's LAN server: name, version, workout count, and sample-encoding format.",
    inputSchema: {
      type: "object",
      properties: {},
      required: [],
    },
  },
  {
    name: "list_workouts",
    description:
      "List workouts from the Wirebody iOS app, sorted by start date descending. Returns a paginated envelope with workout summaries and pagination metadata. Use this to discover workout UUIDs that can be passed to get_workout.",
    inputSchema: {
      type: "object",
      properties: {
        limit: {
          type: "integer",
          minimum: 1,
          maximum: 200,
          default: 50,
          description: "Number of workouts per page (1-200).",
        },
        offset: {
          type: "integer",
          minimum: 0,
          default: 0,
          description: "Pagination offset, 0-indexed.",
        },
      },
      required: [],
    },
  },
  {
    name: "get_workout",
    description:
      "Fetch the full columnar workout detail for a single workout by UUID. Returns the same JSON shape Wirebody emits via share/copy/push: sampleEncoding 'columnar-v1', hoisted device+sourceRevision in 'sources', per-stream parallel arrays (unit, t, v, dur?, src?). Typical size ~50-100 KB for a long workout.",
    inputSchema: {
      type: "object",
      properties: {
        uuid: {
          type: "string",
          description:
            "Workout UUID, e.g., 06E9D6D6-7E37-406C-8465-1DAAD7C223A1. Get one from list_workouts.",
        },
      },
      required: ["uuid"],
    },
  },
  {
    name: "list_quantity_types",
    description:
      "List standalone HealthKit quantity types that Wirebody can export, including category, preferred unit, aggregation style, readable sample count, and first/last sample timestamps.",
    inputSchema: {
      type: "object",
      properties: {},
      required: [],
    },
  },
  {
    name: "get_quantity_series",
    description:
      "Fetch a standalone HealthKit quantity series in Wirebody's columnar-v1 shape. Use list_quantity_types to discover identifiers. Supports optional ISO-8601 from/to range, limit, and offset.",
    inputSchema: {
      type: "object",
      properties: {
        type: {
          type: "string",
          description:
            "HealthKit quantity type identifier, e.g. HKQuantityTypeIdentifierHeartRate or HKQuantityTypeIdentifierStepCount.",
        },
        from: {
          type: "string",
          description: "Optional ISO-8601 UTC start timestamp. Defaults to 7 days before to.",
        },
        to: {
          type: "string",
          description: "Optional ISO-8601 UTC end timestamp. Defaults to now.",
        },
        limit: {
          type: "integer",
          minimum: 1,
          maximum: 50000,
          default: 5000,
          description: "Maximum number of samples to return (1-50000).",
        },
        offset: {
          type: "integer",
          minimum: 0,
          default: 0,
          description: "Pagination offset, 0-indexed.",
        },
      },
      required: ["type"],
    },
  },
  {
    name: "list_sleep_sessions",
    description:
      "List reconciled HealthKit sleep sessions over a date range. Sessions contain in-bed, asleep, awake durations and sparse phase interval columns keyed by Apple's sleep phase names.",
    inputSchema: {
      type: "object",
      properties: {
        from: {
          type: "string",
          description: "Optional ISO-8601 UTC start timestamp. Defaults to 30 days before to.",
        },
        to: {
          type: "string",
          description: "Optional ISO-8601 UTC end timestamp. Defaults to now.",
        },
        limit: {
          type: "integer",
          minimum: 1,
          maximum: 365,
          default: 30,
          description: "Maximum number of sessions to return (1-365).",
        },
        offset: {
          type: "integer",
          minimum: 0,
          default: 0,
          description: "Pagination offset, 0-indexed.",
        },
      },
      required: [],
    },
  },
  {
    name: "get_day_snapshot",
    description:
      "Fetch a single local-calendar day overview from Wirebody: sleep, workouts, activity totals, heart, body, and mobility. Returns the same numbers as the iOS Day view.",
    inputSchema: {
      type: "object",
      properties: {
        date: {
          type: "string",
          description: "Local date in YYYY-MM-DD form, e.g. 2026-05-11.",
        },
      },
      required: ["date"],
    },
  },
];

export function buildServer(client = WirebodyClient.fromEnv()): Server {
  const server = new Server(
    {
      name: "wirebody-mcp",
      version: "0.5.0",
    },
    {
      capabilities: {
        tools: {},
      },
    },
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    try {
      return await callTool(client, request.params.name, request.params.arguments ?? {});
    } catch (error) {
      if (error instanceof WirebodyError) {
        throw new McpError(ErrorCode.InvalidRequest, error.message, { code: error.code });
      }

      throw error;
    }
  });

  return server;
}

async function callTool(
  client: WirebodyClient,
  name: string,
  args: Record<string, unknown>,
): Promise<CallToolResult> {
  switch (name) {
    case "status": {
      const payload = await client.status();
      return textResult(JSON.stringify(payload));
    }
    case "list_workouts": {
      const payload = await client.listWorkouts({
        limit: integerArg(args.limit, 50, 1, 200),
        offset: integerArg(args.offset, 0, 0),
      });
      return textResult(JSON.stringify(payload));
    }
    case "get_workout": {
      if (typeof args.uuid !== "string" || args.uuid.trim().length === 0) {
        throw new McpError(ErrorCode.InvalidParams, "get_workout requires a non-empty uuid string");
      }

      const payload = await client.getWorkout(args.uuid.trim());
      return textResult(payload);
    }
    case "list_quantity_types": {
      const payload = await client.listQuantityTypes();
      return textResult(JSON.stringify(payload));
    }
    case "get_quantity_series": {
      const type = stringArg(args.type);
      if (!type) {
        throw new McpError(
          ErrorCode.InvalidParams,
          "get_quantity_series requires a non-empty type string",
        );
      }

      const payload = await client.getQuantitySeries({
        type,
        from: optionalStringArg(args.from),
        to: optionalStringArg(args.to),
        limit: integerArg(args.limit, 5000, 1, 50000),
        offset: integerArg(args.offset, 0, 0),
      });
      return textResult(payload);
    }
    case "list_sleep_sessions": {
      const payload = await client.listSleepSessions({
        from: optionalStringArg(args.from),
        to: optionalStringArg(args.to),
        limit: integerArg(args.limit, 30, 1, 365),
        offset: integerArg(args.offset, 0, 0),
      });
      return textResult(payload);
    }
    case "get_day_snapshot": {
      const date = stringArg(args.date);
      if (!date) {
        throw new McpError(
          ErrorCode.InvalidParams,
          "get_day_snapshot requires a date string in YYYY-MM-DD form",
        );
      }

      const payload = await client.getDaySnapshot({ date });
      return textResult(payload);
    }
    default:
      throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${name}`);
  }
}

function textResult(text: string): CallToolResult {
  return {
    content: [{ type: "text", text }],
  };
}

function integerArg(
  value: unknown,
  defaultValue: number,
  min: number,
  max?: number,
): number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    return defaultValue;
  }

  const upperBounded = max === undefined ? value : Math.min(value, max);
  return Math.max(upperBounded, min);
}

function stringArg(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function optionalStringArg(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}
