/** Typed client for the engine's `ApiResponse<T>` envelope. Never throws. */
import type { ApiResponse } from "@contracts";
import { API_BASE } from "./config";

export type HttpMethod = "GET" | "PUT" | "POST" | "DELETE";

export type Fetcher = (input: string, init: RequestInit) => Promise<Response>;

const failure = <T>(message: string): ApiResponse<T> => ({
  ok: false,
  error: { code: "unavailable", message, details: null },
});

function isEnvelope(v: unknown): v is ApiResponse<unknown> {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.ok === true) return "data" in o;
  if (o.ok === false) {
    const e = o.error as Record<string, unknown> | null | undefined;
    return typeof e === "object" && e !== null && typeof e.message === "string";
  }
  return false;
}

/**
 * Sends one request. A transport failure or a body that is not an envelope becomes an error
 * response with a user-facing message, so callers handle exactly one shape.
 */
export async function request<T>(
  method: HttpMethod,
  path: string,
  body?: unknown,
  fetcher: Fetcher = (input, init) => fetch(input, init),
): Promise<ApiResponse<T>> {
  const init: RequestInit = { method };
  if (body !== undefined) {
    init.headers = { "Content-Type": "application/json" };
    init.body = JSON.stringify(body);
  }
  let res: Response;
  try {
    res = await fetcher(`${API_BASE}${path}`, init);
  } catch {
    return failure("Could not reach the engine.");
  }
  let json: unknown;
  try {
    json = await res.json();
  } catch {
    return failure(
      `The engine answered with an unreadable response (HTTP ${res.status}).`,
    );
  }
  if (!isEnvelope(json))
    return failure(
      `The engine answered with an unexpected response (HTTP ${res.status}).`,
    );
  // The envelope is checked; the payload type is the route's contract.
  return json as ApiResponse<T>;
}

/** Encodes a path parameter. */
export const seg = (id: string): string => encodeURIComponent(id);
