import { defineComponent, h, type Component, type PropType } from 'vue'
import { Loading } from '@element-plus/icons-vue'

/** Familiar actions are icons; their names remain available to everyone. */
export default defineComponent({
  name: 'UiAction',
  props: {
    label: { type: String, required: true },
    icon: { type: [Object, Function] as PropType<Component>, required: true },
    loading: Boolean,
    disabled: Boolean,
    danger: Boolean,
  },
  emits: { click: (_event: MouseEvent) => true },
  setup(props, { emit }) {
    return () => h('button', {
      type: 'button', class: 'ui-icon-action ui-action',
      'aria-label': props.label, title: props.label,
      'aria-busy': props.loading || undefined,
      'data-glass-action': props.danger ? 'danger' : 'neutral',
      disabled: props.disabled || props.loading,
      onClick: (event: MouseEvent) => { if (!props.disabled && !props.loading) emit('click', event) },
    }, [h(props.loading ? Loading : props.icon, {
      class: ['ui-action__glyph', { 'is-loading': props.loading }], 'aria-hidden': 'true',
    })])
  },
})
