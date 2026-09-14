const root = '/games/monopoly/';
export function atlasArt(path, index, x, y, size, label = '') {
  return `<svg class="art" x="${x-size/2}" y="${y-size}" width="${size}" height="${size}" viewBox="${index%2*640} ${Math.floor(index/2)*640} 640 640" style="overflow:hidden" aria-label="${label}"><image href="${root+path}" width="1280" height="1280"/></svg>`;
}
export function emptyArt(x,y,size=50) {
  return `<svg class="art" x="${x-size/2}" y="${y-size*.56}" width="${size}" height="${size*.62}" viewBox="0 0 100 62"><path d="M3 28 50 4 97 28 50 53Z" fill="var(--board-edge)" stroke="var(--board-line)"/><path d="M3 28 50 53 97 28V36L50 61 3 36Z" fill="var(--board-side)"/><path d="M16 29 50 12 84 29 50 46Z" fill="var(--board-garden)"/><path d="m29 26 26 13m-13-20 26 13" stroke="var(--board-line)" stroke-width="2"/><ellipse cx="24" cy="22" rx="5" ry="3" fill="var(--board-tree)"/><ellipse cx="73" cy="34" rx="5" ry="3" fill="var(--board-tree)"/></svg>`;
}
export function specialArt(tile,x,y,size=70) {
  if(tile.kind==='event') return `<image class="art" href="${root}events/slot-machine.png" x="${x-size/2}" y="${y-size}" width="${size}" height="${size}"/>`;
  return atlasArt('corners/corner-landmarks.png', {start:0,airport:1,price_double:2,jail:3}[tile.corner],x,y,size);
}
export function characterArt(index,x,y) {
  return `<g class="token" aria-label="${['红色礼帽绅士','蓝色飞行员','绿色探险犬','紫色魔女'][index]}"><ellipse cx="${x}" cy="${y+2}" rx="13" ry="6" fill="${['#d97867','#6d9dcd','#64a184','#a183bc'][index]}" stroke="white" stroke-width="2"/>${atlasArt('avatars/player-characters.png',index,x,y+2,52)}${index===0?`<text x="${x}" y="${y-55}" text-anchor="middle" font-size="10" fill="#ba6857" font-weight="700">你 ▼</text>`:''}</g>`;
}
