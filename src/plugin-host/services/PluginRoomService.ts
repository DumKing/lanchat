import type { PluginRoomEvent } from "../contracts/bridge";
import type { PluginBridgeHandlerContext } from "../runtime/BridgeDispatcher";
import type { GameFrame, Profile } from "../../types/lanchat";

export interface PluginRoomSummary {
  roomId: string;
  gameId: string;
  name: string;
  ownerPeerId: string;
  memberPeerIds: string[];
  maxPlayers: number;
  schemaVersion: number;
  updatedAt: number;
}

export interface PluginRoomServiceDependencies {
  profile(): Profile | null;
  send(frame: GameFrame): Promise<void>;
  emit(gameId: string, event: PluginRoomEvent): Promise<void>;
}

type PluginRoomWirePayload =
  | { action: "upsert"; room: PluginRoomSummary }
  | { action: "remove"; roomId: string }
  | { action: "event"; event: PluginRoomEvent };

export class PluginRoomService {
  readonly #dependencies: PluginRoomServiceDependencies;
  readonly #rooms = new Map<string, PluginRoomSummary>();
  readonly #eventSequences = new Map<string, number>();
  readonly #seenEvents = new Set<string>();

  constructor(dependencies: PluginRoomServiceDependencies) {
    this.#dependencies = dependencies;
  }

  list(gameId?: string) {
    return [...this.#rooms.values()]
      .filter((room) => !gameId || room.gameId === gameId)
      .sort((left, right) => right.updatedAt - left.updatedAt);
  }

  getRoom(roomId: string) {
    return this.#rooms.get(roomId) ?? null;
  }

  async create(input: { gameId: string; name: string; maxPlayers: number; schemaVersion: number }) {
    const profile = this.#requireProfile();
    const room: PluginRoomSummary = {
      roomId: crypto.randomUUID(),
      gameId: input.gameId,
      name: input.name.trim(),
      ownerPeerId: profile.device_id,
      memberPeerIds: [profile.device_id],
      maxPlayers: input.maxPlayers,
      schemaVersion: input.schemaVersion,
      updatedAt: Date.now(),
    };
    this.#rooms.set(room.roomId, room);
    await this.#broadcast({ action: "upsert", room });
    return room;
  }

  async join(roomId: string) {
    const profile = this.#requireProfile();
    const current = this.#requireRoom(roomId);
    if (!current.memberPeerIds.includes(profile.device_id) && current.memberPeerIds.length >= current.maxPlayers) {
      throw new Error("房间人数已满");
    }
    const room = {
      ...current,
      memberPeerIds: [...new Set([...current.memberPeerIds, profile.device_id])],
      updatedAt: Date.now(),
    };
    this.#rooms.set(roomId, room);
    await this.#broadcast({ action: "upsert", room });
    return room;
  }

  async leave(roomId: string) {
    const profile = this.#requireProfile();
    const current = this.#requireRoom(roomId);
    const members = current.memberPeerIds.filter((id) => id !== profile.device_id);
    if (current.ownerPeerId === profile.device_id || members.length === 0) {
      this.#rooms.delete(roomId);
      await this.#broadcast({ action: "remove", roomId });
      return;
    }
    const room = { ...current, memberPeerIds: members, updatedAt: Date.now() };
    this.#rooms.set(roomId, room);
    await this.#broadcast({ action: "upsert", room });
  }

  async send(input: { roomId: string; type: string; payload: unknown; idempotencyKey: string }) {
    const profile = this.#requireProfile();
    const room = this.#requireRoom(input.roomId);
    if (!room.memberPeerIds.includes(profile.device_id)) throw new Error("尚未加入此房间");
    const event: PluginRoomEvent = {
      sequence: (this.#eventSequences.get(room.roomId) ?? 0) + 1,
      roomId: room.roomId,
      gameId: room.gameId,
      senderPeerId: profile.device_id,
      schemaVersion: room.schemaVersion,
      idempotencyKey: input.idempotencyKey,
      type: input.type,
      payload: input.payload,
    };
    this.#eventSequences.set(room.roomId, event.sequence);
    await this.#acceptEvent(event);
    await this.#broadcast({ action: "event", event });
  }

  snapshot(roomId: string) {
    this.#requireRoom(roomId);
    return null;
  }

  async receive(frame: GameFrame) {
    if (frame.kind !== "plugin_room") return false;
    const payload = frame.payload as PluginRoomWirePayload;
    if (payload.action === "upsert" && payload.room?.roomId) this.#rooms.set(payload.room.roomId, payload.room);
    else if (payload.action === "remove" && payload.roomId) this.#rooms.delete(payload.roomId);
    else if (payload.action === "event" && payload.event) await this.#acceptEvent(payload.event);
    else return false;
    return true;
  }

  handlers() {
    return {
      "rooms.list": (params: unknown) => this.list((params as { gameId?: string })?.gameId),
      "rooms.create": (params: unknown, context: PluginBridgeHandlerContext) => this.create(this.#guardGame(params as Parameters<PluginRoomService["create"]>[0], context)),
      "rooms.join": (params: unknown) => this.join((params as { roomId: string }).roomId),
      "rooms.leave": (params: unknown) => this.leave((params as { roomId: string }).roomId),
      "rooms.send": (params: unknown) => this.send(params as Parameters<PluginRoomService["send"]>[0]),
      "rooms.snapshot": (params: unknown) => this.snapshot((params as { roomId: string }).roomId),
    };
  }

  #guardGame<T extends { gameId: string }>(input: T, context: PluginBridgeHandlerContext) {
    const expectedPluginId = `com.lanchat.${input.gameId}`;
    if (context.pluginId !== expectedPluginId) throw new Error("插件不能创建其他插件的游戏房间");
    return input;
  }

  async #acceptEvent(event: PluginRoomEvent) {
    const key = `${event.roomId}\0${event.idempotencyKey}`;
    if (this.#seenEvents.has(key)) return;
    this.#seenEvents.add(key);
    if (this.#seenEvents.size > 2048) this.#seenEvents.delete(this.#seenEvents.values().next().value!);
    await this.#dependencies.emit(event.gameId, event);
  }

  async #broadcast(payload: PluginRoomWirePayload) {
    const profile = this.#requireProfile();
    const gameId = payload.action === "upsert" ? payload.room.gameId : payload.action === "event" ? payload.event.gameId : "plugin";
    const roomId = payload.action === "upsert" ? payload.room.roomId : payload.action === "event" ? payload.event.roomId : payload.roomId;
    await this.#dependencies.send({
      frame_id: crypto.randomUUID(),
      game: gameId,
      room_id: roomId,
      sender_device_id: profile.device_id,
      sender_nickname: profile.nickname,
      kind: "plugin_room",
      payload,
      created_at: Date.now(),
    });
  }

  #requireProfile() {
    const profile = this.#dependencies.profile();
    if (!profile) throw new Error("本机资料尚未就绪");
    return profile;
  }

  #requireRoom(roomId: string) {
    const room = this.#rooms.get(roomId);
    if (!room) throw new Error("房间不存在或已经解散");
    return room;
  }
}
