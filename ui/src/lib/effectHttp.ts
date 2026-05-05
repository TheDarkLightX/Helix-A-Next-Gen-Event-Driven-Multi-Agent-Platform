import * as Effect from "effect/Effect";
import * as Schedule from "effect/Schedule";
import type * as Duration from "effect/Duration";
import { loadApiToken } from "./apiAuth";

const DEFAULT_TIMEOUT: Duration.DurationInput = "5 seconds";
const DEFAULT_RETRY_DELAY = "150 millis";

export class ApiClientError extends Error {
  readonly _tag = "ApiClientError";

  constructor(
    readonly method: string,
    readonly path: string,
    readonly status: number | null,
    readonly body: string | null,
    readonly retryable: boolean,
    message: string
  ) {
    super(message);
    this.name = "ApiClientError";
  }
}

export type ApiRequestOptions = {
  retry?: boolean;
  timeout?: Duration.DurationInput;
  allowHttpErrors?: boolean;
};

function normalizeMethod(init?: RequestInit): string {
  return (init?.method ?? "GET").toUpperCase();
}

function normalizeMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  return "unknown error";
}

function isRetryableStatus(status: number): boolean {
  return status === 408 || status === 425 || status === 429 || status >= 500;
}

function buildNetworkError(method: string, path: string, error: unknown): ApiClientError {
  return new ApiClientError(
    method,
    path,
    null,
    null,
    true,
    `${method} ${path} failed before receiving a response: ${normalizeMessage(error)}`
  );
}

function buildTimeoutError(method: string, path: string): ApiClientError {
  return new ApiClientError(
    method,
    path,
    null,
    null,
    true,
    `${method} ${path} timed out`
  );
}

function buildHttpError(
  method: string,
  path: string,
  status: number,
  body: string | null
): ApiClientError {
  return new ApiClientError(
    method,
    path,
    status,
    body,
    isRetryableStatus(status),
    `${method} ${path} returned HTTP ${status}${body ? `: ${body}` : ""}`
  );
}

function buildJsonError(method: string, path: string, error: unknown): ApiClientError {
  return new ApiClientError(
    method,
    path,
    null,
    null,
    false,
    `${method} ${path} returned invalid JSON: ${normalizeMessage(error)}`
  );
}

function shouldRetry(method: string, options?: ApiRequestOptions): boolean {
  return options?.retry ?? method === "GET";
}

function withApiAuthHeaders(path: string, init?: RequestInit): RequestInit | undefined {
  if (!path.startsWith("/api/")) return init;

  const token = loadApiToken();
  if (!token) return init;

  const headers = new Headers(init?.headers);
  if (!headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  return {
    ...init,
    headers,
  };
}

export function requestResponse(
  baseUrl: string,
  path: string,
  init?: RequestInit,
  options?: ApiRequestOptions
): Promise<Response> {
  const method = normalizeMethod(init);
  const requestInit = withApiAuthHeaders(path, init);
  const effect = Effect.tryPromise({
    try: (signal) => fetch(`${baseUrl}${path}`, { ...requestInit, signal }),
    catch: (error) => buildNetworkError(method, path, error),
  }).pipe(
    Effect.timeoutFail({
      duration: options?.timeout ?? DEFAULT_TIMEOUT,
      onTimeout: () => buildTimeoutError(method, path),
    }),
    Effect.flatMap((response) => {
      if (options?.allowHttpErrors === true || response.ok) {
        return Effect.succeed(response);
      }

      return Effect.tryPromise({
        try: async () => {
          const body = await response.text();
          throw buildHttpError(method, path, response.status, body || null);
        },
        catch: (error) =>
          error instanceof ApiClientError
            ? error
            : buildNetworkError(method, path, error),
      });
    }),
    Effect.retry({
      times: shouldRetry(method, options) ? 1 : 0,
      while: (error) => error.retryable,
      schedule: Schedule.exponential(DEFAULT_RETRY_DELAY),
    })
  );

  return Effect.runPromise(effect);
}

export function requestJson<T>(
  baseUrl: string,
  path: string,
  init?: RequestInit,
  options?: ApiRequestOptions
): Promise<T> {
  const method = normalizeMethod(init);
  const effect = Effect.tryPromise({
    try: async () => {
      const response = await requestResponse(baseUrl, path, init, options);
      const text = await response.text();
      if (!text) {
        throw buildJsonError(method, path, "empty response body");
      }
      return JSON.parse(text) as T;
    },
    catch: (error) =>
      error instanceof ApiClientError ? error : buildJsonError(method, path, error),
  });

  return Effect.runPromise(effect);
}

export function readTextResponse(response: Response, method: string, path: string): Promise<string> {
  const effect = Effect.tryPromise({
    try: () => response.text(),
    catch: (error) => buildNetworkError(method, path, error),
  });

  return Effect.runPromise(effect);
}
