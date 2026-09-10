<script setup lang="ts">
import { computed, ref } from 'vue';

export type Board3DTile = {
  index: number; kind: string; corner?: string; title: string; meta: string;
  level: string; buildingStyle: Record<string, string>; ownerColor: string;
  toll: number; selected: boolean; target: boolean; focused: boolean;
  markers: Array<{ kind: string; label: string; title: string; god?: string }>;
  players: Array<{ id: string; name: string; tone: number; color: string; mine: boolean; moving: boolean; god?: string }>;
};
const props = defineProps<{ tiles: Board3DTile[]; announcement?: string }>();
const emit = defineEmits<{ select: [index: number] }>();
const svg = ref<SVGSVGElement>();
const zoom = ref(1), cx = ref(540), cy = ref(370), overhead = ref(false);
const viewBox = computed(() => `${cx.value-540/zoom.value} ${cy.value-370/zoom.value} ${1080/zoom.value} ${740/zoom.value}`);
function grid(i: number): [number, number] { return i<=10 ? [10-i,10] : i<=20 ? [0,20-i] : i<=30 ? [i-20,0] : [10,i-30]; }
function point(x: number,y: number): [number,number] { return [540+(x-y)*43,100+(x+y)*(overhead.value?27:24)]; }
function points(list: number[][]) { return list.map(p=>p.join(',')).join(' '); }
function diamond(x: number,y: number,r=.47) { return [point(x-r,y-r),point(x+r,y-r),point(x+r,y+r),point(x-r,y+r)]; }
function lerp(a: number[],b: number[],ratio: number) { return [a[0]!+(b[0]!-a[0]!)*ratio,a[1]!+(b[1]!-a[1]!)*ratio]; }
function lowered(p: number[]) { return [p[0]!,p[1]!+7]; }
function cityEdgeLevelSegments(index: number,d: number[][],x: number,y: number) {
  const boardSide=index<10?0:index<20?1:index<30?2:3;
  const [start,end]=boardSide%2===0?[d[3]!,d[2]!]:[d[2]!,d[1]!];
  const midpoint=lerp(start,end,.5);
  const inset=[(x-midpoint[0]!)*.2,(y-midpoint[1]!)*.2];
  return Array.from({length:3},(_,segmentIndex)=>{
    const a=lerp(start,end,segmentIndex/3),b=lerp(start,end,(segmentIndex+1)/3);
    return points([a,b,[b[0]!+inset[0]!,b[1]!+inset[1]!],[a[0]!+inset[0]!,a[1]!+inset[1]!]]);
  });
}
const tiles = computed(()=>props.tiles.map(t=>{const [gx,gy]=grid(t.index);const [x,y]=point(gx,gy);const d=diamond(gx,gy);return {...t,x,y,depth:gx+gy,top:points(d),side:points([d[1],d[2],d[3],lowered(d[3]!),lowered(d[2]!),lowered(d[1]!)]),levelSegments:cityEdgeLevelSegments(t.index,d,x,y)};}).sort((a,b)=>a.depth-b.depth));
const outer = computed(()=>points(diamond(5,5,5.7)));
const park = computed(()=>points(diamond(5,5,4.45)));
const water = computed(()=>points(diamond(5,5,1.5)));
const trees = computed(()=>[[2,2],[3,2],[8,3],[2,7],[3,8],[8,8]].map(([x,y])=>point(x,y)));
function cornerPosition(corner?: string) { const n=['start','airport','price_double','jail'].indexOf(corner??'');return `${n%2*100}% ${Math.floor(n/2)*100}%`; }
function itemPosition(kind: string) { const n=Math.max(0,['roadblock','angel','devil','wealth','poverty','double'].indexOf(kind)); return `${n%3*50}% ${Math.floor(n/3)*100}%`; }
function propertyLevelCount(level: string) { return Math.max(0, ['empty','house','level2','level3'].indexOf(level)); }
function tileTextMarkers(markers: Board3DTile['markers']) { return markers.filter(marker=>marker.kind!=='double'&&marker.kind!=='god'); }
function multiplierRoofTransform(tile: { x: number; y: number; level: string }) {
  const roofLift = tile.level === 'level3' ? 43 : tile.level === 'level2' ? 36 : 29;
  return `translate(${tile.x},${tile.y-roofLift})`;
}
function nameplateTransform(tile: { index: number; kind: string; x: number; y: number }) {
  if(tile.kind!=='property') return `translate(${tile.x},${tile.y+22})`;
  const slope=overhead.value?27:24;
  const angle=Math.atan2(slope,43)*180/Math.PI;
  const side=tile.index<10?0:tile.index<20?1:tile.index<30?2:3;
  // Use the camera-facing edge: inward on the back sides, outward on the front sides.
  const dx=[-36,36,-36,36][side],dy=slope;
  return `translate(${tile.x+dx},${tile.y+dy}) rotate(${side%2===0?angle:-angle})`;
}
function setZoom(next: number, anchor={x:cx.value,y:cy.value}) {
  next=Math.max(.6,Math.min(2.2,next));const ratio=zoom.value/next;
  cx.value=anchor.x+(cx.value-anchor.x)*ratio;cy.value=anchor.y+(cy.value-anchor.y)*ratio;zoom.value=next;
}
function reset() { zoom.value=1;cx.value=540;cy.value=370; }
function wheel(e: WheelEvent) {
  if(e.ctrlKey||e.metaKey||!e.deltaY)return;const m=svg.value?.getScreenCTM();if(!m)return;e.preventDefault();
  const anchor=new DOMPoint(e.clientX,e.clientY).matrixTransform(m.inverse());
  const delta=e.deltaY*(e.deltaMode===1?16:e.deltaMode===2?740:1);
  setZoom(zoom.value*Math.exp(-Math.max(-150,Math.min(150,delta))*.002),anchor);
}
let drag: {id:number;x:number;y:number;cx:number;cy:number;matrix:DOMMatrix;moved:boolean}|null=null;
let suppressClick=false;
const dragging=ref(false);
function down(e: PointerEvent) {const m=svg.value?.getScreenCTM();if(e.button!==0||!e.isPrimary||!m)return;suppressClick=false;drag={id:e.pointerId,x:e.clientX,y:e.clientY,cx:cx.value,cy:cy.value,matrix:m.inverse(),moved:false};}
function move(e: PointerEvent) {
  if(!drag||drag.id!==e.pointerId)return;if(!drag.moved&&Math.hypot(e.clientX-drag.x,e.clientY-drag.y)<5)return;
  if(!drag.moved){drag.moved=true;dragging.value=true;svg.value?.setPointerCapture(e.pointerId);}
  const a=new DOMPoint(drag.x,drag.y).matrixTransform(drag.matrix),b=new DOMPoint(e.clientX,e.clientY).matrixTransform(drag.matrix);
  cx.value=Math.max(0,Math.min(1080,drag.cx+a.x-b.x));cy.value=Math.max(0,Math.min(740,drag.cy+a.y-b.y));
}
function up(e: PointerEvent) {if(!drag||drag.id!==e.pointerId)return;suppressClick=drag.moved;drag=null;dragging.value=false;if(svg.value?.hasPointerCapture(e.pointerId))svg.value.releasePointerCapture(e.pointerId);}
function click(e: MouseEvent) {if(suppressClick){suppressClick=false;e.stopPropagation();return;}const el=(e.target as Element).closest('[data-index]');if(el)emit('select',Number(el.getAttribute('data-index')));}
</script>

<template>
  <section class="board3d-scene">
    <svg ref="svg" class="board3d-svg" :class="{dragging}" :viewBox="viewBox" aria-label="大富翁立体棋盘" @wheel="wheel" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up" @lostpointercapture="up" @pointerleave="e=>{if(drag&&!drag.moved)up(e)}" @click="click">
      <polygon :points="outer" transform="translate(0 20)" class="base-side"/>
      <polygon :points="outer" class="base"/>
      <polygon :points="park" class="park"/>
      <polygon :points="water" class="water"/>
      <g v-for="([x,y],i) in trees" :key="i" class="decoration"><path :d="`M${x} ${y}v-22`" stroke="var(--muted)" stroke-width="4"/><ellipse :cx="x" :cy="y-24" rx="13" ry="21" class="tree"/></g>
      <text x="540" :y="point(5,5)[1]+104" class="park-label">大富翁 3D</text>
      <g v-for="tile in tiles" :key="tile.index" class="tile3d" :class="{selected:tile.selected,target:tile.target,focused:tile.focused}">
        <polygon :points="tile.side" class="tile-side"/>
        <polygon :points="tile.top" class="tile-top" :data-index="tile.index" role="button" tabindex="0" :aria-label="`${tile.title}，${tile.meta}`" @keydown.enter.prevent="emit('select',tile.index)" @keydown.space.prevent="emit('select',tile.index)"><title>{{ tile.title }} · {{ tile.meta }}</title></polygon>
        <g v-if="tile.kind==='property'&&propertyLevelCount(tile.level)>0" class="city-edge-level-signal" :style="{'--owner-color':tile.ownerColor}" :aria-label="`${tile.title} · ${propertyLevelCount(tile.level)} 级地产`">
          <polygon v-for="(segmentPoints,segmentIndex) in tile.levelSegments" :key="segmentIndex" :points="segmentPoints" :class="{ active: segmentIndex < propertyLevelCount(tile.level) }"/>
        </g>
        <ellipse v-if="tile.kind!=='property'||tile.level!=='empty'" :cx="tile.x" :cy="tile.y+3" rx="24" ry="10" class="ground-shadow"/>
        <foreignObject :x="tile.x-35" :y="tile.y-52" width="70" height="70" class="art">
          <div v-if="tile.kind==='property'&&tile.level!=='empty'" class="building" :style="tile.buildingStyle"></div>
          <div v-else-if="tile.kind==='corner'" class="corner" :style="{backgroundPosition:cornerPosition(tile.corner)}"></div>
          <img v-else-if="tile.kind==='event'" src="/games/monopoly/events/slot-machine.png" alt="随机事件摇奖机"/>
          <svg v-else class="vacant-lot" viewBox="0 0 70 70" aria-label="待开发空地">
            <path d="M7 51 35 36 63 51v5L35 71 7 56z" class="vacant-soil"/>
            <path d="m7 51 28-15 28 15-28 15z" class="vacant-grass"/>
            <path d="m13 51 22-12 22 12-22 12z" class="vacant-boundary"/>
            <path d="m17 54 5 3m26-12 5 3M26 44l5-3" class="vacant-blades"/>
            <path d="M35 51V32" class="vacant-post"/>
            <rect x="23" y="25" width="24" height="15" rx="3" class="vacant-sign"/>
            <path d="m29 33 6-5 6 5m-10-1v5h8v-5m-5 5v-3h2v3" class="vacant-house"/>
          </svg>
        </foreignObject>
        <g v-if="tileTextMarkers(tile.markers).length" class="status-markers"><rect :x="tile.x-25" :y="tile.y-66" width="50" height="13" rx="3"/><text :x="tile.x" :y="tile.y-56"><title>{{ tileTextMarkers(tile.markers).map(m=>m.title).join(' · ') }}</title>{{ tileTextMarkers(tile.markers).map(m=>m.label).join(' ') }}</text></g>
      </g>
      <g v-for="tile in tiles" :key="`name-${tile.index}`" :transform="nameplateTransform(tile)" class="tile-nameplate"><rect x="-20" y="-6" width="40" height="13" rx="2"/><text y="3" class="tile-label">{{ tile.title }}</text></g>
      <g v-for="tile in tiles" :key="`items-${tile.index}`" class="characters">
        <foreignObject v-for="(marker,i) in tile.markers.filter(m=>m.kind==='god'||m.kind==='roadblock')" :key="marker.kind" :x="tile.x-20+i*15" :y="tile.y-19" width="38" height="38"><div class="board-item" :style="{backgroundPosition:itemPosition(marker.god??marker.kind)}" :title="marker.title"/></foreignObject>
      </g>
      <g v-for="tile in tiles" :key="`players-${tile.index}`" class="characters">
        <g v-for="(player,i) in tile.players" :key="player.id" :transform="`translate(${tile.x+(i-(tile.players.length-1)/2)*18},${tile.y+8})`" :class="{moving:player.moving}">
          <title>{{ player.name }}</title><ellipse cy="2" rx="12" ry="5" :fill="player.color" stroke="white" stroke-width="1.5"/>
          <foreignObject x="-25" y="-48" width="50" height="50"><div class="character" :style="{backgroundPosition:`${player.tone%2*100}% ${Math.floor(player.tone/2)*100}%`}"></div></foreignObject>
          <text v-if="player.mine" y="-51" class="mine-label">你 ▼</text>
          <foreignObject v-if="player.god" x="12" y="-40" width="28" height="28"><div class="board-item attached" :style="{backgroundPosition:itemPosition(player.god)}" :title="`神明附身：${player.god}`"/></foreignObject>
        </g>
      </g>
      <g v-for="tile in tiles.filter(item=>item.toll>0)" :key="`toll-${tile.index}`" :transform="`translate(${tile.x},${tile.y+1})`" class="tile-toll-badge">
        <text y="3">{{ tile.toll }}</text>
      </g>
      <g v-for="tile in tiles.filter(item=>item.markers.some(marker=>marker.kind==='double'))" :key="`double-${tile.index}`" :transform="multiplierRoofTransform(tile)" class="multiplier-badge">
        <g v-for="marker in tile.markers.filter(item=>item.kind==='double')" :key="marker.kind" :aria-label="marker.title">
          <title>{{ marker.title }}</title><ellipse class="roof-contact" cx="0" cy="5" rx="10" ry="3"/><rect x="-14" y="-7" width="28" height="12" rx="6"/><circle cx="-9" cy="-1" r="4"/><path d="M-10.2-2.2h2.4v2.4h-2.4z"/><text x="-3" y="2">{{ marker.label }}</text>
        </g>
      </g>
      <foreignObject v-if="announcement" x="270" :y="point(5,5)[1]-24" width="540" height="64" class="board-announcement" aria-live="polite"><slot name="announcement"><div class="announcement-fallback">{{ announcement }}</div></slot></foreignObject>
    </svg>
    <div class="camera-controls" aria-label="立体棋盘视角"><button :disabled="zoom<=.6" @click="setZoom(zoom-.1)" aria-label="缩小棋盘">−</button><output>{{ Math.round(zoom*100) }}%</output><button :disabled="zoom>=2.2" @click="setZoom(zoom+.1)" aria-label="放大棋盘">＋</button><button @click="reset">复位</button><button :aria-pressed="overhead" @click="overhead=!overhead">{{ overhead?'斜视':'俯视' }}</button></div>
    <small class="camera-hint">滚轮缩放 · 拖动棋盘 · 点击地块</small>
  </section>
</template>

<style scoped>
.vacant-lot{display:block;width:100%;height:100%;overflow:visible}
.vacant-soil{fill:color-mix(in srgb,var(--accent) 26%,var(--line))}
.vacant-grass{fill:color-mix(in srgb,var(--accent) 14%,var(--panel-bg));stroke:color-mix(in srgb,var(--accent) 38%,var(--panel-bg));stroke-width:1}
.vacant-boundary{fill:none;stroke:var(--panel-bg);stroke-width:1.5;stroke-dasharray:3 2;stroke-linejoin:round}
.vacant-blades{fill:none;stroke:color-mix(in srgb,var(--accent) 38%,var(--panel-bg));stroke-width:1.3;stroke-linecap:round}
.vacant-post{fill:none;stroke:color-mix(in srgb,var(--accent) 55%,var(--muted));stroke-width:2.5}
.vacant-sign{fill:var(--panel-bg);stroke:var(--b-side);stroke-width:1}
.vacant-house{fill:none;stroke:var(--accent);stroke-width:1.3;stroke-linejoin:round;stroke-linecap:round}
.multiplier-badge{pointer-events:none}.multiplier-badge .roof-contact{fill:#291a0d;opacity:.24}.multiplier-badge rect{fill:var(--panel-bg);stroke:#dba541;stroke-width:.6;filter:drop-shadow(0 1px 1px #0004)}.multiplier-badge circle{fill:#f8cd59;stroke:#b97b19;stroke-width:.8}.multiplier-badge path{fill:#fff5c3;stroke:#b97b19;stroke-width:.5}.multiplier-badge text{fill:#956012;font-size:9px;font-weight:800;text-anchor:start}
.tile-side{fill:var(--b-side)}.city-edge-level-signal{pointer-events:none}.city-edge-level-signal polygon{fill:color-mix(in srgb,var(--b-side) 64%,var(--panel-bg));stroke:var(--panel-bg);stroke-width:1}.city-edge-level-signal polygon.active{fill:var(--owner-color);filter:drop-shadow(0 1px 1px #0004)}.tile-toll-badge{pointer-events:none}.tile-toll-badge text{fill:#74440e;paint-order:stroke;stroke:var(--panel-bg);stroke-width:2px;stroke-linejoin:round;font-size:9px;font-weight:900;text-anchor:middle}
.board-item{width:100%;height:100%;background-image:url('/games/monopoly/items/board-items.png');background-size:300% 200%;background-repeat:no-repeat}.attached{filter:drop-shadow(0 0 3px #ffd965)}
.tile-nameplate{pointer-events:none}.tile-nameplate rect{fill:var(--panel-bg);stroke:var(--b-side);stroke-width:.7}.tile-nameplate .tile-label{font-size:8px;font-weight:700}
.ground-shadow{fill:#000;opacity:.12;pointer-events:none}
.board-announcement{pointer-events:none;overflow:hidden}
.announcement-fallback{display:flex;align-items:center;justify-content:center;min-height:44px;max-height:100%;padding:8px 18px;box-sizing:border-box;text-align:center;color:var(--text);font-size:18px;font-weight:700;line-height:1.5;overflow-wrap:anywhere;background:color-mix(in srgb,var(--accent) 18%,var(--panel-bg));border:1px solid color-mix(in srgb,var(--accent) 52%,var(--panel-bg));border-radius:999px;box-shadow:0 6px 16px color-mix(in srgb,var(--accent) 20%,transparent)}
.board3d-scene{--b-base:color-mix(in srgb,var(--accent) 23%,var(--panel-bg));--b-side:color-mix(in srgb,var(--accent) 35%,var(--line));position:relative;min-height:170px;flex:1;overflow:hidden;border:1px solid var(--line);border-radius:8px;background:var(--chat-bg)}
.board3d-svg{display:block;width:100%;height:100%;position:absolute;inset:0;touch-action:none;cursor:grab}.dragging,.dragging .tile-top{cursor:grabbing}.base{fill:var(--b-base);stroke:var(--panel-bg);stroke-width:3}.base-side{fill:var(--b-side)}.park{fill:color-mix(in srgb,var(--accent) 12%,var(--panel-bg))}.water{fill:color-mix(in srgb,var(--accent) 40%,var(--panel-bg));stroke:var(--panel-bg);stroke-width:5}.tree{fill:color-mix(in srgb,var(--accent) 45%,var(--panel-bg))}.park-label{fill:var(--muted);font-size:20px;text-anchor:middle;letter-spacing:4px}.tile-top{fill:var(--panel-bg);stroke:var(--b-side);cursor:pointer}.tile-top:hover,.tile-top:focus-visible,.selected .tile-top,.target .tile-top,.focused .tile-top{fill:var(--soft-accent);stroke:var(--accent);stroke-width:3;outline:none}.target .tile-top{stroke-dasharray:5 2}.art,.characters,.tile-label,.tile-toll,.status-markers,.decoration{pointer-events:none}.art{overflow:hidden}.art>div,.art>img,.character{width:100%;height:100%;display:block}.building{background-image:var(--monopoly-building-sheet);background-size:500% 300%;background-position:var(--monopoly-building-column) var(--monopoly-building-row)}.corner{background-image:url('/games/monopoly/corners/corner-landmarks.png');background-size:200% 200%}.art img{object-fit:contain}.character{background-image:url('/games/monopoly/avatars/player-characters.png');background-size:200% 200%}.art .empty-lot{height:29px;margin-top:40px;clip-path:polygon(50% 0,100% 50%,50% 100%,0 50%);background:linear-gradient(135deg,var(--b-base) 50%,var(--b-side) 50%)}.empty-lot i{position:absolute;width:7px;height:5px;background:var(--accent);border-radius:50%;bottom:13px;left:17px}.empty-lot i+i{left:45px;bottom:9px}.tile-label{fill:var(--text);font-size:8px;text-anchor:middle}.tile-toll{fill:var(--muted);font-size:8px;text-anchor:middle}.status-markers rect{fill:var(--accent)}.status-markers text{fill:#fff;font-size:9px;text-anchor:middle}.mine-label{font-size:10px;fill:var(--accent);text-anchor:middle;font-weight:700}.camera-controls{position:absolute;top:8px;left:8px;display:flex;align-items:center;gap:3px;background:var(--panel-bg);border:1px solid var(--line);padding:3px;border-radius:6px}.camera-controls button{border:0;border-radius:4px;padding:5px 7px;background:var(--panel-bg);color:var(--text);cursor:pointer;font-size:11px}.camera-controls button:hover{background:var(--soft-accent)}.camera-controls button:disabled{opacity:.4;cursor:default}.camera-controls output{font-size:10px;min-width:35px;text-align:center;color:var(--muted)}.camera-hint{position:absolute;bottom:6px;left:50%;transform:translateX(-50%);white-space:nowrap;pointer-events:none;color:var(--muted);font-size:10px}
</style>
