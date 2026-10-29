import { computed, inject, onScopeDispose, provide, shallowRef, type Component, type InjectionKey } from 'vue'
import { useRoute } from 'vue-router'

export interface MobilePageAction {
  icon: Component
  label: string
  select: (event: MouseEvent) => void
}
const key: InjectionKey<(source: () => MobilePageAction) => void> = Symbol('mobile-page-action')

/** Pages supply actions; the shell owns one persistent glass surface. */
export function provideMobilePageAction() {
  const route = useRoute()
  const current = shallowRef<{ owner: symbol; path: string; source: () => MobilePageAction } | null>(null)
  provide(key, source => {
    const owner = Symbol('page-action')
    current.value = { owner, path: route.path, source }
    onScopeDispose(() => { if (current.value?.owner === owner) current.value = null })
  })
  return computed(() => current.value?.path === route.path ? current.value.source() : null)
}

export function useMobilePageAction(source: () => MobilePageAction) {
  inject(key, undefined)?.(source)
}
