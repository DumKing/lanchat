export type ComposerKeyEvent = Pick<KeyboardEvent, "key" | "shiftKey" | "isComposing" | "keyCode">;

export function shouldSendComposerMessage(event: ComposerKeyEvent): boolean {
  return event.key === "Enter"
    && !event.shiftKey
    && !event.isComposing
    && event.keyCode !== 229;
}
