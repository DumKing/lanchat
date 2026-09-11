import type { PluginBridgeHandlerContext } from "../runtime/BridgeDispatcher";
import type { PluginRoomService } from "./PluginRoomService";
import { createGameStatsRecord, type GameStatsRecord, upsertGameStatsRecords } from "../../games/gameLeaderboard";
import { createMinesweeperLeaderboardRecord, upsertMinesweeperLeaderboardRecords } from "../../games/minesweeperLeaderboard";
import { eligibleLeaderboardParticipants } from "../../features/games/leaderboardService";
import { api } from "../../services/tauri-api";
import type { Peer, Profile } from "../../types/lanchat";

export interface PluginLeaderboardPolicy {
  maxPlayers: number;
  minHumanPlayers?: number;
}

export interface PluginLeaderboardDependencies {
  profile(): Profile | null;
  peers(): readonly Peer[];
  policy(gameId: string): PluginLeaderboardPolicy | undefined;
  rooms: PluginRoomService;
}

export class PluginLeaderboardService {
  readonly #dependencies: PluginLeaderboardDependencies;
  readonly #submitted = new Set<string>();

  constructor(dependencies: PluginLeaderboardDependencies) {
    this.#dependencies = dependencies;
  }

  async list(input: { gameId: string; limit?: number }) {
    const limit = Math.max(1, Math.min(100, input.limit ?? 30));
    if (input.gameId === "minesweeper") {
      const records = await api.listMinesweeperLeaderboard();
      return records.slice(0, limit).map((record) => ({
        peerId: record.deviceId,
        nickname: record.nickname,
        score: record.elapsedMs,
        updatedAt: record.finishedAt,
      }));
    }
    const records = await api.listGameStats();
    return records.filter((record) => record.game === input.gameId).slice(0, limit).map((record) => ({
      peerId: record.deviceId,
      nickname: record.nickname,
      score: record.totalGames ? record.wins / record.totalGames : 0,
      wins: record.wins,
      losses: record.totalGames - record.wins,
      updatedAt: record.updatedAt,
    }));
  }

  async submit(input: { gameId: string; roomId: string; idempotencyKey: string; result: unknown }, context: PluginBridgeHandlerContext) {
    if (context.pluginId !== `com.lanchat.${input.gameId}`) throw new Error("插件不能提交其他游戏的排行榜");
    const submissionKey = `${input.gameId}\0${input.idempotencyKey}`;
    if (this.#submitted.has(submissionKey)) return;
    const room = this.#dependencies.rooms.getRoom(input.roomId);
    if (!room || room.gameId !== input.gameId) throw new Error("排行榜对应的房间不存在");
    const result = (input.result ?? {}) as Record<string, unknown>;
    const resultPlayers = Array.isArray(result.players) ? result.players as Array<Record<string, unknown>> : [];
    const humans = room.memberPeerIds.map((deviceId) => ({
      deviceId,
      nickname: this.#nickname(deviceId),
      isBot: resultPlayers.some((player) => player.peerId === deviceId && player.isBot === true),
    }));
    if (eligibleLeaderboardParticipants(this.#dependencies.policy(input.gameId), humans).length === 0) return;

    if (input.gameId === "minesweeper") {
      const peerId = String(result.peerId ?? "");
      if (!room.memberPeerIds.includes(peerId)) throw new Error("排行榜玩家不在房间中");
      const record = createMinesweeperLeaderboardRecord({
        deviceId: peerId,
        nickname: this.#nickname(peerId),
        width: Number(result.width),
        height: Number(result.height),
        mines: Number(result.mines),
        elapsedMs: Number(result.elapsedMs),
        moves: Number(result.moves),
      });
      const existing = await api.listMinesweeperLeaderboard();
      await api.upsertMinesweeperLeaderboard(upsertMinesweeperLeaderboardRecords(existing, [record]));
      this.#submitted.add(submissionKey);
      return;
    }

    const winnerIds = this.#winnerIds(input.gameId, room.memberPeerIds, result);
    const existing = await api.listGameStats();
    const updates = humans.filter((player) => !player.isBot).map((player) => {
      const current = existing.find((record) => record.game === input.gameId && record.deviceId === player.deviceId);
      return createGameStatsRecord({
        game: input.gameId as GameStatsRecord["game"],
        deviceId: player.deviceId,
        nickname: player.nickname,
        totalGames: (current?.totalGames ?? 0) + 1,
        wins: (current?.wins ?? 0) + (winnerIds.has(player.deviceId) ? 1 : 0),
      });
    });
    await api.upsertGameStats(upsertGameStatsRecords(existing, updates));
    this.#submitted.add(submissionKey);
  }

  handlers() {
    return {
      "leaderboard.list": (params: unknown) => this.list(params as { gameId: string; limit?: number }),
      "leaderboard.submit": (params: unknown, context: PluginBridgeHandlerContext) => this.submit(
        params as { gameId: string; roomId: string; idempotencyKey: string; result: unknown },
        context,
      ),
    };
  }

  #winnerIds(gameId: string, members: string[], result: Record<string, unknown>) {
    if (gameId === "doudizhu") {
      const landlord = String(result.landlordPeerId ?? "");
      return new Set(result.winnerRole === "landlord" ? [landlord] : members.filter((id) => id !== landlord));
    }
    return new Set([String(result.winnerPeerId ?? "")]);
  }

  #nickname(deviceId: string) {
    const profile = this.#dependencies.profile();
    if (profile?.device_id === deviceId) return profile.nickname;
    return this.#dependencies.peers().find((peer) => peer.device_id === deviceId)?.nickname ?? "局域网玩家";
  }
}
