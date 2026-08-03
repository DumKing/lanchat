<script setup lang="ts">
import { computed, ref } from "vue";
import { NInput } from "naive-ui";
import { shouldSendComposerMessage } from "../utils/composerKeyboard";

const props = defineProps<{
  modelValue: string;
  disabled: boolean;
  placeholder: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  submit: [];
}>();

const isComposing = ref(false);
const value = computed({
  get: () => props.modelValue,
  set: (nextValue: string) => emit("update:modelValue", nextValue),
});

function handleCompositionStart() {
  isComposing.value = true;
}

function handleCompositionEnd() {
  isComposing.value = false;
}

function handleKeydown(event: KeyboardEvent) {
  if (!shouldSendComposerMessage({
    key: event.key,
    shiftKey: event.shiftKey,
    isComposing: event.isComposing || isComposing.value,
    keyCode: event.keyCode,
  })) return;
  event.preventDefault();
  emit("submit");
}
</script>

<template>
  <NInput
    v-model:value="value"
    class="composer-textarea"
    type="textarea"
    :autosize="{ minRows: 3, maxRows: 3 }"
    :disabled="disabled"
    :placeholder="placeholder"
    @compositionstart="handleCompositionStart"
    @compositionend="handleCompositionEnd"
    @keydown="handleKeydown"
  />
</template>
