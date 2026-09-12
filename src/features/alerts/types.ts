import type { PetAlertMode, SimulationMeta } from "../../types/lanchat";

export type AlertFeedbackResult = "real" | "false";

export type AlertFeedbackRecord = {
  responderDeviceId: string;
  responderNickname: string;
  result: AlertFeedbackResult;
  createdAt: number;
};

export type AlertRecord = {
  alertId: string;
  senderDeviceId: string;
  senderNickname: string;
  senderAddress?: string | null;
  content: string;
  mode: PetAlertMode;
  simulation?: SimulationMeta | null;
  createdAt: number;
  incoming: boolean;
  handled: boolean;
  localFeedback?: AlertFeedbackResult;
  feedbacks: AlertFeedbackRecord[];
};
