import { createApp, computed, ref, h } from 'vue';
import Board from '../src/components/MonopolyBoard3D.vue';
import RoomChat from '../src/components/MonopolyRoomChat.vue';
import { createMonopolyState, monopolyPropertyCityName } from '../src/games/monopoly';
import '../src/styles/global.css';
import appSource from '../src/App.vue?raw';
import roomCSS from '../src/styles/monopoly3d-room.css?raw';
const style=document.createElement('style');style.textContent=(appSource.match(/<style scoped>([\s\S]*?)<\/style>/)?.[1]??'')+roomCSS;document.head.append(style);
// Browser-only component fixture; never included by the desktop entry point.
createApp({components:{Board},setup(){
  const game=createMonopolyState([{deviceId:'a',nickname:'甲'},{deviceId:'b',nickname:'乙'}]);
  const selected=ref(-1),position=ref(0),visible=ref(true),ended=ref(false);
  const picker=ref(false);
  const chatDraft=ref('');
  const chatMessages=ref([{id:'one',sender:'蓝色飞行员',content:'下一轮到我了',mine:false}]);
  const sendChat=()=>{if(chatDraft.value.trim()){chatMessages.value.push({id:String(Date.now()),sender:'我',content:chatDraft.value,mine:true});chatDraft.value='';}};
  const tiles=computed(()=>game.board.map(t=>({index:t.index,kind:t.kind,corner:t.kind==='corner'?t.corner:undefined,title:t.kind==='property'?monopolyPropertyCityName(t.index):t.kind==='event'?'随机事件':t.corner==='airport'?'飞机场':t.corner==='jail'?'监狱':t.corner==='start'?'起点':'地价翻倍',meta:'组件验收',level:t.kind==='property'?(['empty','house','level2','level3'][t.index%4]??'empty'):'empty',buildingStyle:{'--monopoly-building-sheet':"url('/games/monopoly/buildings/style-1.png')",'--monopoly-building-column':'25%','--monopoly-building-row':'0%'},ownerColor:'#e45a5a',toll:t.kind==='property'?500:0,selected:selected.value===t.index,target:t.index===10,focused:false,markers:t.index===3?[{kind:'roadblock',label:'障',title:'路障'}]:[],players:t.index===position.value?[{id:'a',name:'甲',tone:0,color:'#e45a5a',mine:true,moving:false}]:[]})));
  const el=(cls:string,children:any)=>h('div',{class:cls},children);
  const cards=(offset:number)=>[0,1].map(i=>h('article',{class:'monopoly-player-seat',style:{'--monopoly-player-color':['#e45a5a','#377fe8','#3ca56a','#9a62d6'][i+offset]}},[
    el('monopoly-player-seat-main',[h('span',{class:`monopoly-virtual-avatar monopoly-avatar-tone-${i+offset+1}`}),el('',[h('strong',['红色绅士','蓝色飞行员','探险犬','紫色魔女'][i+offset]),h('small','3 地块 · 2 道具')])]),h('small',{class:'monopoly-player-seat-coins'},'5,000 金币')
  ]));
  return ()=>h('div',{class:'provider-root theme-lan',style:'height:100vh;display:flex;flex-direction:column;background:var(--app-bg)'},[
    h('button',{onClick:()=>picker.value=!picker.value,style:'position:fixed;right:8px;bottom:8px;z-index:20'},'验收玩家选择'),
    picker.value?h('div',{class:'monopoly-card-target-picker',style:'position:fixed;z-index:30;top:80px;left:30px;width:260px;padding:16px;background:var(--panel-bg);border:1px solid var(--line);border-radius:12px'},[h('p','选择目标玩家'),...['红色绅士','蓝色飞行员'].map((name,i)=>h('button',{class:'monopoly-card-target-player',style:{'--monopoly-player-color':`var(--monopoly-player-${i+1}, ${['#e45a5a','#377fe8'][i]})`},onClick:()=>picker.value=false},[h('span',{class:`monopoly-virtual-avatar monopoly-avatar-tone-${i+1}`}),h('strong',name)]))]):null,
    el('game-header-actions',[h('button',{onClick:()=>visible.value=!visible.value},'切换组件显示'),h('button',{onClick:()=>position.value=(position.value+1)%40},'移动一格'),h('button',{onClick:()=>ended.value=!ended.value},'切换结算'),h('span',`位置 ${position.value} · 选中 ${selected.value}`)]),
    h('div',{class:'monopoly-layout monopoly-view-3d',style:'flex:1;min-height:0'},[el('monopoly-table',[el('monopoly-stage',[
      h('aside',{class:'monopoly-player-side left'},[h('aside',{class:'monopoly-announcement-log'},[el('monopoly-announcement-log-heading','事件日志'),el('monopoly-announcement-log-list',Array.from({length:12},(_,i)=>h('article',[h('strong',`第 ${i+1} 回合 · 玩家经过起点获得奖励`)])))]),...cards(0)]),
      el('monopoly-board monopoly-board-3d',[
        h(Board,{tiles:tiles.value.map(t=>({...t,markers:t.index===5?[{kind:'double',label:'×2',title:'过路费 ×2'}]:t.index===6?[{kind:'double',label:'×3',title:'过路费 ×3'}]:t.index===12?[{kind:'god',god:'angel',label:'天使',title:'天使神明'}]:t.index===16?[{kind:'god',god:'devil',label:'恶魔',title:'恶魔神明'}]:t.markers,players:t.players.map(p=>({...p,god:'wealth'}))})),announcement:ended.value?undefined:'红色绅士购入晴川城，建成小屋',style:{display:visible.value?'block':'none'},onSelect:(index:number)=>selected.value=index},{announcement:()=>h('div',{class:'monopoly-announcement rent',style:{'--monopoly-announcement-color':'#e45a5a'}},[h('strong',[h('span',{class:'monopoly-virtual-avatar monopoly-avatar-tone-1 monopoly-announcement-inline-avatar'}),'红色绅士购入晴川城，建成小屋'])])}),
        el('monopoly-center-column',[
          h('section',{class:'monopoly-center-panel'},[el('monopoly-center-overview',[
            el('monopoly-turn-summary',[h('span','第 3 / 20 回合'),el('monopoly-current-action',[h('span','当前行动'),h('strong','红色绅士'),h('small','正在选择道具或投骰')]),h('span','28s')]),
            el('monopoly-center-player',[h('span',{class:'monopoly-virtual-avatar monopoly-avatar-tone-1'}),el('monopoly-center-player-copy',[h('strong','我 · 红色绅士'),h('small','第 1 顺位 · 当前位置 12 · 顺时针前进')]),el('monopoly-center-cash',[h('span','金币'),h('strong','5,000')])]),
            el('monopoly-center-inventory',[el('monopoly-center-assets',[h('div',[h('strong','3'),h('small','我的地块')]),h('div',[h('strong','无'),h('small','神明状态')]),h('div',[h('strong','3 / 3'),h('small','背包道具')])]),el('monopoly-center-cards',['指定骰','路障','查封'].map(n=>h('button',{class:'monopoly-card'},[h('b',n[0]),h('span',n)])))]),
            h('strong',{class:'monopoly-center-status'},'投骰前可使用道具'),el('monopoly-dice-result',[h('span','骰子结果'),h('span','⚄ ⚁'),h('strong','7'),h('small','红色绅士 · 5 + 2')])
          ]),el('monopoly-center-actions',[h('button','掷骰子'),h('button','购买小屋'),h('button','放弃')])])
          ,ended.value?h('section',{class:'monopoly-center-panel settlement monopoly-settlement-overlay-3d'},[
            el('monopoly-center-settlement',[h('small','现金结算'),h('h3','红色绅士 获胜'),el('monopoly-center-settlement-list',['红色绅士','蓝色飞行员','探险犬','紫色魔女'].map((n,i)=>h('div',{class:i===0?'winner':''},[h('span',i+1),h('span',{class:`monopoly-virtual-avatar monopoly-avatar-tone-${i+1}`}),h('strong',n),h('em',`${9000-i*1000} 金币`)]))),el('monopoly-center-actions',[h('button','再来一局'),h('button','退出房间')])])
          ]):null
        ])
      ]),
      h('aside',{class:'monopoly-player-side right'},[h('aside',{class:'game-room-panel monopoly-room-panel'},[h(RoomChat,{messages:chatMessages.value,draft:chatDraft.value,'onUpdate:draft':(v:string)=>chatDraft.value=v,onSend:sendChat,onEnter:sendChat})]),...cards(2)])
    ])])])
  ]);
}}).mount('#app');
