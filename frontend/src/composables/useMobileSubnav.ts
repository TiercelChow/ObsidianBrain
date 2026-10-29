import { computed, inject, onScopeDispose, provide, shallowRef, type Component, type ComputedRef, type InjectionKey } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import { useRoute } from 'vue-router'

export interface MobileSubnavItem {
  id: string
  label: string
  compactLabel?: string
  icon?: Component
  active: boolean
  to?: RouteLocationRaw
  select?: () => void
}

export interface MobileSubnav {
  label: string
  layout?: 'icons'
  items: MobileSubnavItem[]
}

const key: InjectionKey<(source: () => MobileSubnav) => void> = Symbol('mobile-subnav')

/** A leaving route cannot clear the navigation registered by its successor. */
export function provideMobileSubnav(): ComputedRef<MobileSubnav | null> {
  const route = useRoute()
  const current = shallowRef<{ owner: symbol; path: string; source: () => MobileSubnav } | null>(null)
  provide(key, (source) => {
    const owner = Symbol('page-subnav')
    current.value = { owner, path: route.path, source }
    onScopeDispose(() => {
      if (current.value?.owner === owner) current.value = null
    })
  })
  return computed(() => current.value?.path === route.path ? current.value.source() : null)
}

export function useMobileSubnav(source: () => MobileSubnav) {
  inject(key, undefined)?.(source)
}
