import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const mainTs = readFileSync(resolve('src/main.ts'), 'utf8');
const styles = readFileSync(resolve('src/styles/global.css'), 'utf8');
const boundaryPath = resolve('src/components/AppErrorBoundary.vue');

assert.ok(existsSync(boundaryPath), '应提供根级 AppErrorBoundary 组件，避免 Vue 渲染异常导致整窗白屏');

const boundary = readFileSync(boundaryPath, 'utf8');

assert.match(mainTs, /import AppErrorBoundary from "\.\/components\/AppErrorBoundary\.vue";/, 'main.ts 应导入根级错误边界');
assert.match(mainTs, /h\(AppErrorBoundary,[\s\S]*?h\(App\)/, 'App 应被错误边界包裹，而不是直接挂载');
assert.match(mainTs, /app\.config\.errorHandler\s*=/, 'Vue 全局错误应进入统一兜底处理');
assert.match(mainTs, /window\.addEventListener\("error"/, '普通运行时错误应被捕获');
assert.match(mainTs, /event instanceof ErrorEvent/, '图片等资源加载失败不应被升级成整页致命错误');
assert.match(mainTs, /window\.addEventListener\("unhandledrejection"/, '未处理 Promise 错误应被捕获');
assert.match(mainTs, /window\.dispatchEvent\(new CustomEvent\("lanchat-app-error"/, '入口捕获到的错误应通知错误边界展示兜底页');

assert.match(boundary, /onErrorCaptured\(/, '错误边界应捕获子组件渲染和生命周期错误');
assert.match(boundary, /lanchat-app-error/, '错误边界应接收 main.ts 派发的全局错误事件');
assert.match(boundary, /function reloadApp\(\)/, '兜底页应提供重新加载能力');
assert.match(boundary, /function copyErrorDetail\(\)/, '兜底页应提供复制错误详情，便于排查');
assert.match(boundary, /v-if="errorState"/, '发生错误后应渲染兜底页面而不是继续白屏');
assert.match(boundary, /<slot v-else/, '无错误时应正常渲染应用内容');

assert.match(styles, /\.app-error-boundary\s*\{[\s\S]*?height:\s*100vh/s, '错误兜底页面应覆盖整个应用窗口');
assert.match(styles, /\.app-error-actions\s*\{[\s\S]*?display:\s*flex/s, '兜底页面应有可操作按钮区域');

console.log('app error boundary checks passed');
