/** One function per training, content, athlete and progress route used by the slice (14 §5). */
import type {
  ApiResponse,
  AthleteProfile,
  ContentIndex,
  CreateSessionRequest,
  MotionClip,
  ProgressSummary,
  RepTrace,
  RepetitionResult,
  SeriesResponse,
  TrainingCommandRequest,
  TrainingEvent,
  TrainingSessionDetail,
  TrainingSessionSummary,
  TrainingState,
} from "@contracts";
import { request, seg } from "./http";

type R<T> = Promise<ApiResponse<T>>;

/** Encodes query parameters, skipping absent values. */
export function qs(params: Record<string, string | number | null | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params))
    if (v !== null && v !== undefined && v !== "") p.set(k, String(v));
  const s = p.toString();
  return s ? `?${s}` : "";
}

export const paths = {
  contentIndex: "/content/index",
  i18n: (lang: string) => `/content/i18n/${seg(lang)}`,
  athletes: "/athletes",
  sessions: (athleteId: string, limit?: number) =>
    `/training/sessions${qs({ athlete_id: athleteId, limit })}`,
  session: (id: string) => `/training/sessions/${seg(id)}`,
  clinicReview: (id: string) => `/training/sessions/${seg(id)}/clinic-review`,
  reps: (id: string) => `/training/sessions/${seg(id)}/reps`,
  rep: (id: string, rep: number) => `/training/reps/${seg(id)}/${rep}`,
  trace: (id: string, rep: number) => `/training/reps/${seg(id)}/${rep}/trace`,
  motion: (id: string, rep: number) => `/training/reps/${seg(id)}/${rep}/motion`,
  series: (p: {
    athlete_id: string;
    technique: string;
    metric: string;
    side?: string | null;
    origin?: string | null;
  }) => `/progress/series${qs(p)}`,
  summary: (athleteId: string, origin?: string | null) =>
    `/progress/summary${qs({ athlete_id: athleteId, origin })}`,
} as const;

export const trainingActions = {
  state: (): R<TrainingState | null> => request("GET", "/training/state"),
  events: (id: string, afterSeq: number): R<TrainingEvent[]> =>
    request("GET", `/training/sessions/${seg(id)}/events${qs({ after_seq: afterSeq, limit: 1000 })}`),
  createSession: (body: CreateSessionRequest): R<TrainingState> =>
    request("POST", "/training/sessions", body),
  command: (sessionId: string, body: Omit<TrainingCommandRequest, "session_id">): R<TrainingState> =>
    request("POST", "/training/command", { ...body, session_id: sessionId }),
  createAthlete: (body: {
    id: string;
    display_name: string;
    height_m: number;
  }): R<AthleteProfile> => request("POST", "/athletes", body),
  consent: (id: string, motionData: boolean): R<AthleteProfile> =>
    request("POST", `/athletes/${seg(id)}/consent`, { motion_data: motionData }),
  setActive: (id: string): R<unknown> =>
    request("PUT", "/athletes/active", { athlete_id: id }),
} as const;

export type {
  ContentIndex,
  MotionClip,
  ProgressSummary,
  RepTrace,
  RepetitionResult,
  SeriesResponse,
  TrainingSessionDetail,
  TrainingSessionSummary,
};
