<template><!-- The shared Dock renders this page's action. --></template>

<script setup lang="ts">
import type { Component } from 'vue'
import { useMobilePageAction } from '@/composables/useMobilePageAction'

// Registration only: the Dock keeps the actual surface alive between pages.
const props = defineProps<{ icon: Component; label: string }>()
const emit = defineEmits<{ click: [event: MouseEvent] }>()
useMobilePageAction(() => ({ icon: props.icon, label: props.label, select: event => emit('click', event) }))
</script>
