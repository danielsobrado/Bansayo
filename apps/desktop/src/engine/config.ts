/** Engine endpoint. `__ENGINE_URL__` comes from `VITE_ENGINE_URL` or the default in vite.config.ts. */

export const ENGINE_ORIGIN: string = __ENGINE_URL__;
export const API_BASE = `${ENGINE_ORIGIN}/api/v1`;
export const STREAM_URL = `${ENGINE_ORIGIN.replace(/^http/, "ws")}/api/v1/stream?topics=frame,state,landmarks_2d`;
/** `127.0.0.1:7700`, for messages. */
export const ENGINE_HOST = new URL(ENGINE_ORIGIN).host;

/** The only `protocol_version` this build understands. */
export const SUPPORTED_PROTOCOL_VERSION = 1;
