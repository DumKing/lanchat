import { describe, expect, it } from "vitest";
import { eligibleLeaderboardParticipants } from "../../src/features/games/leaderboardService";

describe("插件排行榜阈值", () => {
  it("多人游戏默认至少两名真人，单人游戏允许一人", () => {
    const player = [{ deviceId: "a", nickname: "A" }];
    expect(eligibleLeaderboardParticipants({ maxPlayers: 2 }, player)).toHaveLength(0);
    expect(eligibleLeaderboardParticipants({ maxPlayers: 1 }, player)).toHaveLength(1);
    expect(eligibleLeaderboardParticipants({ maxPlayers: 4, minHumanPlayers: 1 }, player)).toHaveLength(1);
  });
});
