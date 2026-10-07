import { computed, defineComponent, inject, provide, type InjectionKey, type Ref, type VNode } from 'vue'

const surfaceLiveKey: InjectionKey<Readonly<Ref<boolean>>> = Symbol('motion-surface-live')

/** Keep the last rendered contents while a sheet exits. Parents may clear
 * selection immediately; that must not empty/shrink the departing surface. */
export default defineComponent({
  name: 'MotionContent',
  props: { live: Boolean },
  setup(props, { slots }) {
    const enclosingSurface = inject(surfaceLiveKey, null)
    const live = computed(() => props.live && enclosingSurface?.value !== false)
    // A cached component VNode can still rerender its own reactive slots. Carry
    // the enclosing sheet's lifetime through nested switches as well.
    provide(surfaceLiveKey, live)
    let last: VNode[] | undefined
    return () => {
      if (live.value || !last) last = slots.default?.()
      return last
    }
  },
})
