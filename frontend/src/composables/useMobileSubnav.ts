import { computed, inject, onScopeDispose, provide, shallowRef, type Component, type ComputedRef, type InjectionKey } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

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
  items: MobileSubnavItem[]
}

const key: InjectionKey<(source: () => MobileSubnav) => void> = Symbol('mobile-subnav')

/** A leaving route cannot clear the navigation registered by its successor. */
export function provideMobileSubnav(): ComputedRef<MobileSubnav | null> {
  const current = shallowRef<{ owner: symbol; source: () => MobileSubnav } | null>(null)
  provide(key, (source) => {
    const owner = Symbol('page-subnav')
    current.value = { owner, source }
    onScopeDispose(() => {
      if (current.value?.owner === owner) current.value = null
    })
  })
  return computed(() => current.value?.source() ?? null)
}

export function useMobileSubnav(source: () => MobileSubnav) {
  inject(key, undefined)?.(source)
}
