import { emptyArt, specialArt, characterArt } from './monopoly-3d-art.js';
// Standalone design preview. The original game's board and price constants are authoritative.
import { createMonopolyBoard, monopolyPropertyCityName, MONOPOLY_ECONOMY } from '/src/games/monopoly.ts';
import { setupBoardCamera } from './monopoly-3d-camera.js';
import appStyles from '/src/styles/global.css?raw';

const $ = (id) => document.getElementById(id);
// Reuse the actual app theme tokens without importing its global layout rules.
const themeStyles = document.createElement('style');
themeStyles.textContent = (appStyles.match(/\.provider-root\.theme-[\w-]+\{[^{}]*\}/g) ?? []).join('\n');
document.head.append(themeStyles);
$('theme').addEventListener('change', event => {
  document.body.className = `provider-root theme-${event.target.value}`;
});
const camera = setupBoardCamera($('board'));
const board = createMonopolyBoard();
const colors = ['#d97867', '#6d9dcd', '#64a184', '#a183bc'];
const people = ['小红 · 你', '蓝鲸', '小森林', '紫芋'];
const cornerNames = { start: '起点', airport: '飞机场', price_double: '地价翻倍', jail: '监狱' };
const cornerIcons = { start: '⚑', airport: '✈', price_double: '×2', jail: '▥' };
let selected = 1, position = 0, topView = false, moving = false, generation = 0;
const properties = Object.fromEntries(board.filter(t => t.kind === 'property').map((t, n) => [t.index, { owner: Math.floor(n / 4) % 4, level: n % 3, variant: n % 3 + 1, empty: n % 7 === 6 }]));
const tileName = t => t.kind === 'property' ? monopolyPropertyCityName(t.index) : t.kind === 'event' ? '随机事件' : cornerNames[t.corner];
function grid(i) { if(i <= 10) return [10 - i, 10]; if(i <= 20) return [0, 20 - i]; if(i <= 30) return [i - 20, 0]; return [10, i - 30]; }
function project(x,y) { return [540 + (x-y)*43, 95 + (x+y)*(topView ? 27 : 24)]; }
const pts = a => a.map(p => p.join(',')).join(' ');
const poly = (a,fill,extra='') => `<polygon points="${pts(a)}" fill="${fill}" ${extra}/>`;
function diamond(x,y,size=0.47) { return [project(x-size,y-size),project(x+size,y-size),project(x+size,y+size),project(x-size,y+size)]; }
function spriteMarkup(p,x,y,w=68) {
  const h=w*1.11, col=p.owner+1, row=p.level;
  return `<svg class="art" x="${x-w/2}" y="${y-h+7}" width="${w}" height="${h}" viewBox="0 0 307.2 341.333"><defs><clipPath id="crop-${x}-${y}"><polygon points="153,0 260,60 304,273 154,341 3,273 45,60"/></clipPath></defs><image href="/games/monopoly/buildings/style-${p.variant}.png" x="${-col*307.2}" y="${-row*341.333}" width="1536" height="1024" clip-path="url(#crop-${x}-${y})"/></svg>`;
}
function renderBoard() {
  const outer=[project(-.7,-.7),project(10.7,-.7),project(10.7,10.7),project(-.7,10.7)];
  let s='<defs><filter id="shadow"><feGaussianBlur stdDeviation="15"/></filter></defs>';
  s+=poly(outer.map(([x,y])=>[x,y+35]),'var(--board-shadow)','opacity=".16" filter="url(#shadow)"');
  s+=poly([outer[1],outer[2],outer[3],[outer[3][0],outer[3][1]+22],[outer[2][0],outer[2][1]+22],[outer[1][0],outer[1][1]+22]],'var(--board-side)');
  s+=poly(outer,'var(--board-base)','stroke="var(--board-edge)" stroke-width="4"');
  s+=poly([project(.55,.55),project(9.45,.55),project(9.45,9.45),project(.55,9.45)],'var(--board-park)','stroke="var(--board-edge)" stroke-width="4"');
  // A quiet central park, drawn behind every interactive tile.
  s+=poly([project(2,4.2),project(5,2),project(8,5.7),project(5.7,8)],'var(--board-garden)');
  s+=poly([project(3.2,5),project(5,3.5),project(7,5.1),project(5.3,6.6)],'var(--board-water)','stroke="var(--board-edge)" stroke-width="7"');
  s+=`<ellipse cx="540" cy="${project(5,5)[1]}" rx="31" ry="13" fill="var(--board-water-deep)"/><path d="M540 ${project(5,5)[1]}v-39" stroke="var(--board-edge)" stroke-width="4"/><ellipse cx="540" cy="${project(5,5)[1]-38}" rx="9" ry="3" fill="var(--board-edge)"/>`;
  for(const [x,y] of [[2,2],[3,2],[7,2],[8,3],[2,7],[3,8],[8,8],[8,7],[2,3]]){const [a,b]=project(x,y);s+=`<ellipse cx="${a}" cy="${b+5}" rx="13" ry="6" fill="var(--board-tree-shadow)"/><path d="M${a} ${b}v-20" stroke="var(--board-trunk)" stroke-width="5"/><ellipse cx="${a}" cy="${b-25}" rx="14" ry="21" fill="var(--board-tree)"/><ellipse cx="${a-4}" cy="${b-30}" rx="9" ry="15" fill="var(--board-tree-light)"/>`;}
  s+=`<text x="540" y="${project(5,5)[1]+103}" text-anchor="middle" fill="var(--board-ink)" font-size="20" font-weight="700" letter-spacing="5">大富翁 3D</text><text x="540" y="${project(5,5)[1]+124}" text-anchor="middle" fill="var(--muted)" font-size="9" letter-spacing="3">微 缩 城 市 · 交 互 预 览</text>`;
  for(const t of [...board].sort((a,b)=>{const aa=grid(a.index),bb=grid(b.index);return aa[0]+aa[1]-bb[0]-bb[1]})){
    const [gx,gy]=grid(t.index),[x,y]=project(gx,gy),p=properties[t.index],d=diamond(gx,gy);
    s+=`<g class="tile ${t.index===selected?'selected':''}">`;
    s+=poly([d[1],d[2],d[3],[d[3][0],d[3][1]+8],[d[2][0],d[2][1]+8],[d[1][0],d[1][1]+8]],'var(--board-side)');
    s+=poly(d,t.kind==='property'?'var(--board-tile)':t.kind==='event'?'var(--board-event)':'var(--board-corner)',`class="tile-top" stroke="var(--board-line)" stroke-width="1" role="button" tabindex="0" aria-label="${tileName(t)}，第${t.index+1}格" data-index="${t.index}"`);
    if(p){s+=poly([d[3],d[2],[d[2][0],d[2][1]+5],[d[3][0],d[3][1]+5]],p.empty?'var(--board-line)':colors[p.owner]); if(!p.empty)s+=spriteMarkup(p,x,y-4);else s+=emptyArt(x,y-2);}
    else s+=specialArt(t,x,y+2);
    s+=`<text x="${x}" y="${y+14}" text-anchor="middle" font-size="8" fill="var(--text)" font-weight="600">${tileName(t)}</text></g>`;
  }
  for(const [i,pos] of [[1,12],[2,23],[3,31],[0,position]]){const [x,y]=project(...grid(pos));s+=characterArt(i,x+12,y+9);}
  $('board').innerHTML=s;
}
function detail(){const t=board[selected],p=properties[selected];$('tile-number').textContent=`NO. ${String(selected+1).padStart(2,'0')}`;$('property-name').textContent=tileName(t);$('property-kind').textContent=t.kind==='property'?'城市地产':t.kind==='event'?'机遇与挑战':'特殊地块';$('owner-chip').textContent=p?(p.empty?'待购买':people[p.owner].split(' ·')[0]):'公共区域';$('property-art').innerHTML=p&&!p.empty?`<div class="sprite" style="background-image:url('/games/monopoly/buildings/style-${p.variant}.png');background-position:${(p.owner+1)*25}% ${p.level*50}%"></div>`:`<svg width="90" height="88" viewBox="0 0 100 100">${p?emptyArt(50,85,90):specialArt(t,50,95,90)}</svg>`;
  const rent=p&&!p.empty?[MONOPOLY_ECONOMY.houseRent,MONOPOLY_ECONOMY.level2Rent,MONOPOLY_ECONOMY.level3Rent][p.level]:0;
  $('property-facts').innerHTML=p?`<div><small>建筑等级</small><strong>${p.empty?'空地':['小屋','二级建筑','三级建筑'][p.level]}</strong></div><div><small>${p.empty?'购买价格':'基础过路费'}</small><strong>${p.empty?MONOPOLY_ECONOMY.emptyPurchase:rent} 金币</strong></div>`:'<div><small>格子类型</small><strong>全体玩家共享</strong></div>';
  $('property-description').textContent=p?(p.empty?'原版规则：购买后形成小屋。预览仅展示地块，不扣除金币。':'原版规则：再次落在自己的建筑上会免费升级。当前归属与等级为视觉示例。'):t.kind==='event'?'正式游戏中会触发随机事件。本预览仅演示落地效果。':'正式游戏沿用原版特殊地块效果。本预览仅演示落地效果。';}
function select(index){selected=index;renderBoard();detail();}
$('players').innerHTML=people.map((name,i)=>`<div class="player ${i===0?'you':''}"><div class="avatar" style="background-position:${i%2*100}% ${Math.floor(i/2)*100}%"></div><div class="player-copy"><strong>${name}</strong><small style="color:${colors[i]}">● ${['当前行动','等待回合','等待回合','等待回合'][i]}</small></div><div class="money">${['5,000','6,500','4,200','5,800'][i]}<small>金币 · 示例</small></div></div>`).join('');
$('board').addEventListener('click',e=>{const t=e.target.closest('[data-index]');if(t)select(Number(t.dataset.index));});
$('board').addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){const t=e.target.closest('[data-index]');if(t){e.preventDefault();const index=Number(t.dataset.index);select(index);$('board').querySelector(`[data-index="${index}"]`).focus();}}});
$('roll').addEventListener('click',async()=>{if(moving)return;moving=true;$('roll').disabled=true;const token=++generation,n=1+Math.floor(Math.random()*6);$('dice').textContent=['⚀','⚁','⚂','⚃','⚄','⚅'][n-1];for(let i=0;i<n;i++){await new Promise(r=>setTimeout(r,240));if(token!==generation)return;position=(position+1)%40;renderBoard();$('status').textContent=`掷出 ${n} 点 · 前进 ${i+1} / ${n} 格`;}select(position);$('status').textContent=`掷出 ${n} 点 · 已抵达${tileName(board[position])}（移动演示）`;moving=false;$('roll').disabled=false;});
$('angle').addEventListener('click',()=>{topView=!topView;$('angle').textContent=topView?'切换斜视':'切换俯视';$('angle').setAttribute('aria-pressed',String(topView));renderBoard();});
$('reset').addEventListener('click',()=>{generation++;position=0;moving=false;camera.reset();$('roll').disabled=false;$('status').textContent='从起点出发，试试掷骰子吧';$('dice').textContent='⚄';select(1);});
$('rules-open').addEventListener('click',()=>$('rules').showModal());$('rules-close').addEventListener('click',()=>$('rules').close());
renderBoard();detail();
