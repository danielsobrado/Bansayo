export const SYNC_ANALYSIS = {
  maxWindowUs:8_000_000,
  maxLagUs:200_000,
  minGridUs:5_000,
  minPairs:64,
  maxGapRatio:1.5,
  minCorrelation:0.7,
  ambiguityMargin:0.05,
  competingPeakUs:30_000,
  minDeviation:1e-6,
} as const;
