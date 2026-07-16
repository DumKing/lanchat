<script setup lang="ts">
import { computed, onErrorCaptured, onMounted, onUnmounted, ref } from "vue";

type AppErrorPayload = {
  message: string;
  stack?: string;
  source?: string;
};

const errorState = ref<AppErrorPayload | null>(null);
const copied = ref(false);

const errorDetail = computed(() => {
  if (!errorState.value) return "";
  return [
    `来源：${errorState.value.source ?? "未知"}`,
    `错误：${errorState.value.message}`,
    errorState.value.stack ? `堆栈：\n${errorState.value.stack}` : "",
  ].filter(Boolean).join("\n\n");
});

function normalizeError(error: unknown, source: string): AppErrorPayload {
  if (error instanceof Error) {
    return {
      message: error.message || "应用运行时发生异常",
      stack: error.stack,
      source,
    };
  }
  if (typeof error === "string") {
    return { message: error, source };
  }
  return { message: "应用运行时发生异常", stack: JSON.stringify(error), source };
}

function showError(error: unknown, source: string) {
  errorState.value = normalizeError(error, source);
}

function onGlobalAppError(event: Event) {
  const detail = (event as CustomEvent<AppErrorPayload>).detail;
  errorState.value = detail?.message ? detail : normalizeError(detail, "全局异常");
}

function reloadApp() {
  window.location.reload();
}

async function copyErrorDetail() {
  copied.value = false;
  try {
    await navigator.clipboard.writeText(errorDetail.value);
    copied.value = true;
  } catch {
    copied.value = false;
  }
}

onErrorCaptured((error, _instance, info) => {
  showError(error, `Vue 组件异常：${info}`);
  return false;
});

onMounted(() => {
  window.addEventListener("lanchat-app-error", onGlobalAppError);
});

onUnmounted(() => {
  window.removeEventListener("lanchat-app-error", onGlobalAppError);
});
</script>

<template>
  <main v-if="errorState" class="app-error-boundary">
    <section class="app-error-card">
      <span class="app-error-mark">!</span>
      <div class="app-error-copy">
        <h1>LanChat 遇到了一点问题</h1>
        <p>界面渲染异常已被拦截，没有让整个软件白屏。可以先重新加载继续使用，错误详情可复制给开发者排查。</p>
      </div>
      <pre>{{ errorDetail }}</pre>
      <div class="app-error-actions">
        <button type="button" class="primary" @click="reloadApp">重新加载</button>
        <button type="button" @click="copyErrorDetail">{{ copied ? "已复制" : "复制错误详情" }}</button>
      </div>
    </section>
  </main>
  <slot v-else />
</template>
