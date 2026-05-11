#!/usr/bin/env node
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { ConfigError } from "./client.js";
import { buildServer } from "./server.js";

try {
  const server = buildServer();
  const transport = new StdioServerTransport();
  await server.connect(transport);
} catch (error) {
  if (error instanceof ConfigError) {
    console.error(error.message);
    process.exit(1);
  }

  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
