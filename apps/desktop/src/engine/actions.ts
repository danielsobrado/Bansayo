/**
 * One function per HTTP route in docs/04 §5. State-changing routes are acknowledged here; the
 * resulting change arrives as a `state` message on the stream.
 */
import type {
  ApiResponse,
  AssignmentRequest,
  CalibrationProfileSummary,
  DeletedResponse,
  DeviceId,
  DeviceStatus,
  FaultRequest,
  HealthResponse,
  PlaybackStartRequest,
  PlaybackState,
  PlaybackStopResponse,
  PlaybackUpdateRequest,
  ProfileId,
  RecordingSession,
  RecordingStartRequest,
  RecordingState,
  ReferencePoseRequest,
  SimulationState,
  SimulationUpdateRequest,
  SkeletonDefinition,
  SystemState,
} from "@contracts";
import { request, seg } from "./http";

type R<T> = Promise<ApiResponse<T>>;

export const actions = {
  health: (): R<HealthResponse> => request("GET", "/health"),
  state: (): R<SystemState> => request("GET", "/state"),
  skeleton: (): R<SkeletonDefinition> => request("GET", "/skeleton"),
  assignDevice: (id: DeviceId, body: AssignmentRequest): R<DeviceStatus> =>
    request("PUT", `/devices/${seg(id)}/assignment`, body),
  captureReferencePose: (body: ReferencePoseRequest): R<CalibrationProfileSummary[]> =>
    request("POST", "/calibration/reference-pose", body),
  deleteCalibration: (id: ProfileId): R<DeletedResponse> => request("DELETE", `/calibration/${seg(id)}`),
  updateSimulation: (body: SimulationUpdateRequest): R<SimulationState> => request("PUT", "/simulation", body),
  simulateFault: (id: DeviceId, body: FaultRequest): R<DeviceStatus> =>
    request("POST", `/simulation/devices/${seg(id)}/fault`, body),
  sessions: (): R<RecordingSession[]> => request("GET", "/sessions"),
  startRecording: (body: RecordingStartRequest): R<RecordingState> => request("POST", "/recording/start", body),
  stopRecording: (): R<RecordingSession> => request("POST", "/recording/stop", {}),
  startPlayback: (body: PlaybackStartRequest): R<PlaybackState> => request("POST", "/playback/start", body),
  updatePlayback: (body: PlaybackUpdateRequest): R<PlaybackState> => request("PUT", "/playback", body),
  stopPlayback: (): R<PlaybackStopResponse> => request("POST", "/playback/stop", {}),
} as const;
