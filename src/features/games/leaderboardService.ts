export interface LeaderboardParticipant {
  deviceId: string;
  nickname: string;
  isBot?: boolean;
}

export interface LeaderboardPolicy {
  maxPlayers?: number;
  minHumanPlayers?: number;
}

export function resolveLeaderboardThreshold(policy?: LeaderboardPolicy): number {
  const threshold = policy?.minHumanPlayers ?? (policy?.maxPlayers === 1 ? 1 : 2);
  if (!Number.isInteger(threshold) || threshold < 1) {
    throw new Error("minHumanPlayers 必须是大于等于 1 的整数");
  }
  if (policy?.maxPlayers !== undefined && threshold > policy.maxPlayers) {
    throw new Error("minHumanPlayers 不能超过 maxPlayers");
  }
  return threshold;
}

function uniqueHumanParticipants(players: readonly LeaderboardParticipant[]) {
  const unique = new Map<string, LeaderboardParticipant>();
  for (const player of players) {
    if (player.isBot || !player.deviceId || unique.has(player.deviceId)) continue;
    unique.set(player.deviceId, player);
  }
  return [...unique.values()];
}

export function eligibleLeaderboardParticipants(
  policy: LeaderboardPolicy | undefined,
  players: readonly LeaderboardParticipant[],
): LeaderboardParticipant[] {
  const humans = uniqueHumanParticipants(players);
  return humans.length >= resolveLeaderboardThreshold(policy) ? humans : [];
}

export function shouldPersistLeaderboardResult(
  policy: LeaderboardPolicy | undefined,
  players: readonly LeaderboardParticipant[],
): boolean {
  return eligibleLeaderboardParticipants(policy, players).length > 0;
}
