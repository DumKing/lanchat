import { createApp, h } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import AppErrorBoundary from "./components/AppErrorBoundary.vue";
import "./styles/global.css";

function errorPayload(error: unknown, source: string) {
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
  return {
    message: "应用运行时发生异常",
    stack: JSON.stringify(error),
    source,
  };
}

function notifyAppError(error: unknown, source: string) {
  window.dispatchEvent(new CustomEvent("lanchat-app-error", {
    detail: errorPayload(error, source),
  }));
}

function isIgnorableResizeObserverError(error: unknown) {
  const message = error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "";
  return message === "ResizeObserver loop limit exceeded"
    || message === "ResizeObserver loop completed with undelivered notifications.";
}

const app = createApp({
  render: () => h(AppErrorBoundary, null, { default: () => h(App) }),
});

app.config.errorHandler = (error, _instance, info) => {
  notifyAppError(error, `Vue 全局异常：${info}`);
};

window.addEventListener("error", (event) => {
  if (!(event instanceof ErrorEvent) || (!event.error && !event.message)) return;
  const error = event.error ?? event.message;
  if (isIgnorableResizeObserverError(error)) {
    event.preventDefault();
    return;
  }
  notifyAppError(error, "浏览器运行时异常");
});

window.addEventListener("unhandledrejection", (event) => {
  notifyAppError(event.reason, "未处理的异步异常");
});

app.use(createPinia()).mount("#app");
