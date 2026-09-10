<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { NButton, NInput } from 'naive-ui';
const props = defineProps<{ messages: Array<{ id: string; sender: string; content: string; mine?: boolean }>; draft: string }>();
const emit = defineEmits<{ 'update:draft': [value: string]; send: []; enter: [event: KeyboardEvent] }>();
const list = ref<HTMLElement>();
watch(() => props.messages.length, async () => { await nextTick(); if (list.value) list.value.scrollTop = list.value.scrollHeight; }, { immediate: true });
</script>
<template>
  <section class="monopoly-chat-module" aria-label="房间聊天">
    <header>房间聊天</header>
    <div ref="list" class="messages">
      <p v-if="!messages.length" class="empty">暂无消息，和房间里的玩家聊聊吧</p>
      <article v-for="message in messages" :key="message.id" :class="{ mine: message.mine }"><small>{{ message.sender }}</small><div>{{ message.content }}</div></article>
    </div>
    <footer><NInput :value="draft" size="small" placeholder="输入消息…" @update:value="emit('update:draft', $event)" @keydown.enter="emit('enter', $event)"/><NButton size="small" type="primary" @click="emit('send')">发送</NButton></footer>
  </section>
</template>
<style scoped>
.monopoly-chat-module{display:grid;grid-template-rows:30px minmax(0,1fr) auto;width:100%;height:100%;min-width:0;min-height:0;box-sizing:border-box;overflow:hidden;background:var(--panel-bg);color:var(--text);pointer-events:auto}
header{display:flex;align-items:center;padding:0 10px;font-size:11px;font-weight:700;border-bottom:1px solid var(--line);background:var(--top-bg)}
.messages{min-height:0;overflow:auto;padding:8px;scrollbar-width:thin}
article{margin:0 0 8px;font-size:11px;line-height:1.5}article small{display:block;color:var(--muted);font-size:10px;margin-bottom:3px}article>div{display:inline-block;max-width:100%;box-sizing:border-box;padding:5px 8px;border:1px solid var(--line);border-radius:7px;background:var(--app-bg);overflow-wrap:anywhere}.mine{text-align:right}.mine>div{background:var(--soft-accent)}
.empty{text-align:center;font-size:11px;color:var(--muted)}footer{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:6px;padding:8px;border-top:1px solid var(--line)}
</style>
