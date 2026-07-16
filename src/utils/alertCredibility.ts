export type AlertFeedbackLike = {
  result: "real" | "false" | string;
};

export type AlertRecordLike = {
  senderDeviceId: string;
  createdAt: number;
  feedbacks: AlertFeedbackLike[];
};

export type AlertTruthScore = {
  probability: number;
  weight: number;
  realCount: number;
  falseCount: number;
  feedbackCount: number;
};

const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;

function timeWeight(createdAt: number, now: number) {
  const ageMs = Math.max(0, now - createdAt);
  if (ageMs <= SEVEN_DAYS_MS) return 1;
  if (ageMs <= THIRTY_DAYS_MS) return 0.7;
  return 0.4;
}

export function alertTruthScore(alert: AlertRecordLike, now = Date.now()): AlertTruthScore {
  const realCount = alert.feedbacks.filter((feedback) => feedback.result === "real").length;
  const falseCount = alert.feedbacks.filter((feedback) => feedback.result === "false").length;
  const feedbackCount = realCount + falseCount;
  const probability = Math.round(((realCount + 1) / (feedbackCount + 2)) * 100);
  if (feedbackCount === 0) {
    return { probability, weight: 0, realCount, falseCount, feedbackCount };
  }
  const confidenceWeight = Math.min(1, feedbackCount / 5);
  return {
    probability,
    weight: confidenceWeight * timeWeight(alert.createdAt, now),
    realCount,
    falseCount,
    feedbackCount,
  };
}

export function senderCredibility(alerts: AlertRecordLike[], senderDeviceId: string, now = Date.now()) {
  let totalWeight = 0;
  let weightedScore = 0;
  for (const alert of alerts) {
    if (alert.senderDeviceId !== senderDeviceId) continue;
    const score = alertTruthScore(alert, now);
    if (score.weight <= 0) continue;
    totalWeight += score.weight;
    weightedScore += score.probability * score.weight;
  }
  if (totalWeight === 0) return null;
  return Math.round(weightedScore / totalWeight);
}

export function alertTemperature(credibility: number | null) {
  if (credibility === null) return 100;
  if (credibility >= 80) return Math.round(90 + ((credibility - 80) / 20) * 10);
  if (credibility >= 60) return Math.round(70 + ((credibility - 60) / 20) * 19);
  if (credibility >= 40) return Math.round(45 + ((credibility - 40) / 20) * 24);
  return Math.round(20 + (Math.max(0, credibility) / 40) * 24);
}
