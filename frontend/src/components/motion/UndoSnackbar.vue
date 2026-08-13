<template>
  <Transition name="undo-snackbar">
    <div v-if="show" class="undo-snackbar" role="status" aria-live="polite">
      <span>{{ message }}</span>
      <button type="button" @click="$emit('undo')">撤销</button>
    </div>
  </Transition>
</template>

<script setup lang="ts">
defineProps<{ show: boolean; message: string }>()
defineEmits<{ undo: [] }>()
</script>

<style scoped>
.undo-snackbar {
  position: fixed;
  left: 50%;
  bottom: max(20px, calc(var(--safe-bottom) + 12px));
  z-index: 3200;
  display: flex;
  align-items: center;
  gap: 16px;
  min-height: 52px;
  max-width: calc(100vw - 24px);
  padding: 4px 6px 4px 18px;
  border: 1px solid var(--border-glass);
  border-radius: 17px;
  background: var(--bg-glass-strong);
  backdrop-filter: blur(24px) saturate(180%);
  -webkit-backdrop-filter: blur(24px) saturate(180%);
  box-shadow: var(--shadow-lg), var(--inset-highlight);
  color: var(--text-secondary);
  font-size: 14px;
  transform: translateX(-50%);
}

.undo-snackbar span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.undo-snackbar button {
  min-width: 64px;
  min-height: var(--tap-target);
  padding: 0 14px;
  border: 0;
  border-radius: 13px;
  background: var(--accent-light);
  color: var(--accent);
  font-weight: 700;
}

.undo-snackbar-enter-active,
.undo-snackbar-leave-active {
  transition: opacity var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}
.undo-snackbar-enter-from,
.undo-snackbar-leave-to { opacity: 0; transform: translate(-50%, 12px) scale(0.97); }

@media (max-width: 768px) {
  .undo-snackbar { width: calc(100vw - 24px); justify-content: space-between; }
}
</style>
