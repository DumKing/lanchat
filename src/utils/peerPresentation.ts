export type PresentablePeer = {
  device_id: string;
  nickname: string;
  note?: string | null;
  online: boolean;
};

export function sameDeviceId(left?: string | null, right?: string | null): boolean {
  const normalize = (value?: string | null) => {
    const trimmed = (value ?? "").trim().toLowerCase();
    const compact = trimmed.replace(/[:-]/g, "");
    return /^[0-9a-f]{12}$/.test(compact) ? compact : trimmed;
  };
  const leftValue = normalize(left);
  const rightValue = normalize(right);
  return !!leftValue && leftValue === rightValue;
}

export function peerDisplayName(peer: Pick<PresentablePeer, "nickname" | "note">): string {
  return peer.note?.trim() || peer.nickname;
}

export function peerOriginalName(peer: Pick<PresentablePeer, "nickname" | "note">): string {
  return peer.note?.trim() ? peer.nickname : "";
}

export function sortPeersForDisplay<T extends PresentablePeer>(peers: readonly T[]): T[] {
  const collator = new Intl.Collator("zh-CN", { sensitivity: "base" });
  return [...peers].sort((left, right) => {
    if (left.online !== right.online) return left.online ? -1 : 1;
    const leftHasNote = !!left.note?.trim();
    const rightHasNote = !!right.note?.trim();
    if (leftHasNote !== rightHasNote) return leftHasNote ? -1 : 1;
    const nameCompare = collator.compare(peerDisplayName(left), peerDisplayName(right));
    return nameCompare || collator.compare(left.device_id, right.device_id);
  });
}
