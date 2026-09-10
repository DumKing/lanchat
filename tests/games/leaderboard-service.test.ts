import { describe, expect, it } from "vitest";
import {
  eligibleLeaderboardParticipants,
  shouldPersistLeaderboardResult,
  type LeaderboardParticipant,
} from "../../src/features/games/leaderboardService";

const participant = (id: string, isBot = false): LeaderboardParticipant => ({
  deviceId: id,
  nickname: id,
  isBot,
});

describe("leaderboardService", () => {
  it("默认需要至少两名真人参与", () => {
    expect(shouldPersistLeaderboardResult(undefined, [participant("a")])).toBe(false);
    expect(shouldPersistLeaderboardResult(undefined, [participant("a"), participant("b")])).toBe(true);
  });

  it("单机游戏默认允许一名真人计榜", () => {
    expect(shouldPersistLeaderboardResult({ maxPlayers: 1 }, [participant("a")])).toBe(true);
    expect(shouldPersistLeaderboardResult({ maxPlayers: 1 }, [participant("bot", true)])).toBe(false);
  });

  it("按游戏声明的真人阈值决定是否计榜", () => {
    const players = [participant("a"), participant("b")];

    expect(shouldPersistLeaderboardResult({ minHumanPlayers: 2 }, players)).toBe(true);
    expect(shouldPersistLeaderboardResult({ minHumanPlayers: 3 }, players)).toBe(false);
  });

  it("机器人永远不计入阈值，也不产生排行记录", () => {
    const players = [participant("a"), participant("b"), participant("bot", true)];

    expect(shouldPersistLeaderboardResult({ minHumanPlayers: 3 }, players)).toBe(false);
    expect(eligibleLeaderboardParticipants({ minHumanPlayers: 2 }, players).map((player) => player.deviceId)).toEqual(["a", "b"]);
  });

  it("单人游戏可以显式声明阈值为一", () => {
    const players = [participant("a")];

    expect(shouldPersistLeaderboardResult({ minHumanPlayers: 1 }, players)).toBe(true);
    expect(eligibleLeaderboardParticipants({ minHumanPlayers: 1 }, players)).toEqual(players);
  });

  it("扫雷仍然排除机器人和重复设备", () => {
    const players = [participant("a"), participant("a"), participant("bot", true)];

    expect(eligibleLeaderboardParticipants({ minHumanPlayers: 1 }, players)).toEqual([participant("a")]);
  });

  it("拒绝无效阈值，避免插件绕过计榜策略", () => {
    const players = [participant("a"), participant("b"), participant("c")];

    expect(() => shouldPersistLeaderboardResult({ minHumanPlayers: 0 }, players)).toThrow("minHumanPlayers");
    expect(() => shouldPersistLeaderboardResult({ minHumanPlayers: 1.5 }, players)).toThrow("minHumanPlayers");
  });
});
